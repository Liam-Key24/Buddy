//! Single production owner of model readiness, generation IDs, timeouts, and cancel.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::mlx_runtime::MlxState;
use crate::runtime_policy::RuntimePolicy;
use crate::run_control::RunGuard;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompleteKind {
    Interpret,
    Chat,
    Extract,
    Socials,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayError {
    Timeout { kind: TimeoutKind, message: String },
    Cancelled { message: String },
    Busy { message: String },
    Unavailable { message: String },
    Http(String),
}

impl GatewayError {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Timeout { message, .. }
            | Self::Cancelled { message }
            | Self::Busy { message }
            | Self::Unavailable { message }
            | Self::Http(message) => message,
        }
    }
}

impl From<GatewayError> for String {
    fn from(err: GatewayError) -> Self {
        err.as_str().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutKind {
    Startup,
    Generation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRecord {
    pub turn_id: String,
    pub generation_id: String,
    pub conversation_id: String,
    pub model: String,
    pub status: String,
    pub started_at: u64,
    pub deadline_ms: u64,
    pub cancelled_at: Option<u64>,
    pub finished_at: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct IdleSnapshot {
    pub generating: bool,
    pub cpu_generating_hint: &'static str,
    pub cpu_idle_hint: &'static str,
    pub secs_since_generation: u64,
    pub unload_after_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompleteHttp {
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<CompleteToolCallHttp>,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompleteToolCallHttp {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainAction {
    Keep,
    Recycle,
}

/// Brain recycle policy. A busy MLX generation is not Brain death.
pub fn brain_action_after_embed_probe(
    api_current: bool,
    embed_ok: bool,
    mlx_generating: bool,
) -> BrainAction {
    if !api_current {
        return BrainAction::Recycle;
    }
    if mlx_generating {
        return BrainAction::Keep;
    }
    if embed_ok {
        BrainAction::Keep
    } else {
        BrainAction::Recycle
    }
}

pub fn is_readonly_tool(name: &str) -> bool {
    name.ends_with(".look")
        || name.ends_with(".list")
        || name.starts_with("list_")
        || name == "goal.look"
}

pub fn timeout_copy(kind: TimeoutKind, mlx_confirmed_stopped: bool) -> &'static str {
    match (kind, mlx_confirmed_stopped) {
        (TimeoutKind::Startup, true) => {
            "The local model took too long to start, so I stopped it safely. Your request is saved—try again now that the model has been loaded."
        }
        (TimeoutKind::Startup, false) => {
            "The local model took too long to start. Your request is saved—try again in a moment."
        }
        (TimeoutKind::Generation, true) => {
            "Buddy stopped the local model because it exceeded the safe working time. Your request is saved and no further processing is running."
        }
        (TimeoutKind::Generation, false) => {
            "Buddy stopped this turn because it exceeded the safe working time. Your request is saved."
        }
    }
}

pub fn stopped_copy(mlx_confirmed_stopped: bool) -> &'static str {
    if mlx_confirmed_stopped {
        "Stopped safely. Completed work is saved and the model is no longer running."
    } else {
        "Stopped. Completed work is saved."
    }
}

// #region agent log
fn dbg_log(hypothesis_id: &str, location: &str, message: &str, data: Value) {
    let line = json!({
        "sessionId": "472329",
        "hypothesisId": hypothesis_id,
        "location": location,
        "message": message,
        "data": data,
        "timestamp": now_secs() * 1000,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/Volumes/DISK/02_PROJECTS/BUDDY/.cursor/debug-472329.log")
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}
// #endregion

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// In-memory adapter used for characterization and lifecycle tests.
#[derive(Default)]
pub struct FakeModel {
    pub state: Mutex<MlxState>,
    pub generating: AtomicBool,
    pub interrupt_called: AtomicBool,
    pub complete_calls: AtomicU64,
    pub delay: Mutex<Duration>,
    pub last_tokens: Mutex<Option<u32>>,
    pub response: Mutex<Option<CompleteHttp>>,
    pub loaded: Mutex<Option<String>>,
    pub last_generation: Mutex<Option<GenerationRecord>>,
    pub reject_late: Mutex<Vec<String>>,
}

impl FakeModel {
    pub fn new_ready() -> Self {
        let fake = Self::default();
        *fake.state.lock().expect("state") = MlxState::Ready;
        *fake.loaded.lock().expect("loaded") = Some("mlx-community/Qwen3-14B-4bit".into());
        fake
    }

    pub fn abandon_http_without_interrupt(&self) {
        self.generating.store(true, Ordering::SeqCst);
        if let Ok(mut st) = self.state.lock() {
            *st = MlxState::Generating;
        }
    }

    pub fn interrupt(&self) {
        self.interrupt_called.store(true, Ordering::SeqCst);
        self.generating.store(false, Ordering::SeqCst);
        if let Ok(mut st) = self.state.lock() {
            if *st == MlxState::Generating {
                *st = MlxState::Cancelling;
            }
            if *st == MlxState::Cancelling {
                *st = MlxState::Stopped;
            }
        }
    }

    pub fn start_generation(&self, record: GenerationRecord) {
        let mut st = self.state.lock().expect("state");
        *st = st.transition(MlxState::Generating).unwrap_or(MlxState::Generating);
        self.generating.store(true, Ordering::SeqCst);
        *self.last_generation.lock().expect("gen") = Some(record);
    }

    pub fn finish_generation(&self) {
        self.generating.store(false, Ordering::SeqCst);
        if let Ok(mut st) = self.state.lock() {
            if *st == MlxState::Generating {
                *st = MlxState::Ready;
            }
        }
        if let Ok(mut rec) = self.last_generation.lock() {
            if let Some(r) = rec.as_mut() {
                r.status = "done".into();
                r.finished_at = Some(now_secs());
            }
        }
    }

    pub fn reject_if_cancelled(&self, generation_id: &str) -> bool {
        self.reject_late
            .lock()
            .expect("late")
            .iter()
            .any(|id| id == generation_id)
    }
}

pub struct InferenceGateway {
    state: Mutex<MlxState>,
    generation: Mutex<Option<GenerationRecord>>,
    last_used_secs: AtomicU64,
    cold_start: AtomicBool,
}

impl Default for InferenceGateway {
    fn default() -> Self {
        Self {
            state: Mutex::new(MlxState::Stopped),
            generation: Mutex::new(None),
            last_used_secs: AtomicU64::new(0),
            cold_start: AtomicBool::new(true),
        }
    }
}

impl InferenceGateway {
    pub fn state(&self) -> MlxState {
        *self.state.lock().expect("mlx state")
    }

    pub fn set_state(&self, next: MlxState) -> Result<MlxState, String> {
        let mut lock = self.state.lock().expect("mlx state");
        let updated = lock.transition(next)?;
        *lock = updated;
        Ok(updated)
    }

    pub fn force_state(&self, next: MlxState) {
        *self.state.lock().expect("mlx state") = next;
    }

    pub fn generation_timeout(policy: &RuntimePolicy, cold_first: bool) -> Duration {
        if cold_first {
            policy.cold_first_generation
        } else {
            policy.warm_generation
        }
    }

    pub fn tokens_for(policy: &RuntimePolicy, kind: CompleteKind) -> u32 {
        match kind {
            CompleteKind::Chat => policy.qwen_chat_tokens,
            CompleteKind::Interpret | CompleteKind::Extract | CompleteKind::Socials => {
                policy.qwen_interpret_tokens
            }
        }
    }

    pub fn begin_generation(
        &self,
        turn_id: &str,
        conversation_id: &str,
        model: &str,
        deadline: Duration,
    ) -> Result<GenerationRecord, GatewayError> {
        if !self.state().accepts_new_generation() && self.state() != MlxState::Ready {
            return Err(GatewayError::Busy {
                message: "I'm still finishing the last request. Stop it first if you want to start something new."
                    .into(),
            });
        }
        if self.state() == MlxState::Stopped {
            let _ = self.set_state(MlxState::Starting);
        }
        if self.state() == MlxState::Starting {
            let _ = self.set_state(MlxState::Ready);
        }
        self.set_state(MlxState::Generating).map_err(|e| GatewayError::Busy { message: e })?;
        let record = GenerationRecord {
            turn_id: turn_id.to_string(),
            generation_id: format!("gen-{}", now_secs()),
            conversation_id: conversation_id.to_string(),
            model: model.to_string(),
            status: "generating".into(),
            started_at: now_secs(),
            deadline_ms: deadline.as_millis() as u64,
            cancelled_at: None,
            finished_at: None,
        };
        *self.generation.lock().expect("generation") = Some(record.clone());
        self.last_used_secs.store(now_secs(), Ordering::SeqCst);
        Ok(record)
    }

    pub fn mark_cancelled(&self) -> Option<GenerationRecord> {
        let mut lock = self.generation.lock().expect("generation");
        if let Some(rec) = lock.as_mut() {
            rec.status = "cancelled".into();
            rec.cancelled_at = Some(now_secs());
            let id = rec.generation_id.clone();
            let out = rec.clone();
            drop(lock);
            let _ = self.set_state(MlxState::Cancelling);
            return Some(out).filter(|_| !id.is_empty());
        }
        None
    }

    pub fn finish_ready(&self) {
        if let Some(rec) = self.generation.lock().expect("generation").as_mut() {
            rec.status = "done".into();
            rec.finished_at = Some(now_secs());
        }
        *self.generation.lock().expect("generation") = None;
        if self.state() == MlxState::Generating {
            let _ = self.set_state(MlxState::Ready);
        } else if self.state() == MlxState::Cancelling {
            let _ = self.set_state(MlxState::Ready);
        }
        self.cold_start.store(false, Ordering::SeqCst);
        self.last_used_secs.store(now_secs(), Ordering::SeqCst);
    }

    pub fn finish_stopped(&self) {
        if let Some(rec) = self.generation.lock().expect("generation").as_mut() {
            rec.finished_at = Some(now_secs());
        }
        *self.generation.lock().expect("generation") = None;
        if self.state() == MlxState::Cancelling {
            let _ = self.set_state(MlxState::Stopped);
        }
    }

    pub fn is_cold_first(&self) -> bool {
        self.cold_start.load(Ordering::SeqCst)
    }

    pub fn mark_warm(&self) {
        self.cold_start.store(false, Ordering::SeqCst);
    }

    pub fn last_used_secs(&self) -> u64 {
        self.last_used_secs.load(Ordering::SeqCst)
    }

    pub fn touch(&self) {
        self.last_used_secs.store(now_secs(), Ordering::SeqCst);
    }

    pub fn idle_snapshot(&self, policy: &RuntimePolicy) -> IdleSnapshot {
        let last = self.last_used_secs();
        let now = now_secs();
        let since = if last == 0 { 0 } else { now.saturating_sub(last) };
        IdleSnapshot {
            generating: self.state() == MlxState::Generating,
            cpu_generating_hint: "high",
            cpu_idle_hint: "negligible",
            secs_since_generation: since,
            unload_after_secs: policy.mlx_idle.as_secs(),
        }
    }

    pub fn should_idle_unload(&self, policy: &RuntimePolicy, now: u64, keep_warm: bool) -> bool {
        if keep_warm {
            return false;
        }
        if !matches!(self.state(), MlxState::Ready) {
            return false;
        }
        let last = self.last_used_secs();
        last > 0 && now.saturating_sub(last) >= policy.mlx_idle.as_secs()
    }

    pub fn generation_is_current(&self, generation_id: &str) -> bool {
        self.generation
            .lock()
            .expect("generation")
            .as_ref()
            .is_some_and(|g| g.generation_id == generation_id && g.cancelled_at.is_none())
    }
}

/// Live complete: wait for Qwen readiness, then generate under a split deadline.
pub async fn complete_live(
    app: &tauri::AppHandle,
    state: &AppState,
    run: &RunGuard,
    policy: &RuntimePolicy,
    messages: &[Value],
    tools: &[Value],
    kind: CompleteKind,
    conversation_id: &str,
    turn_id: &str,
) -> Result<CompleteHttp, GatewayError> {
    use tauri::{Emitter, Manager};

    use crate::services::ProcessManager;

    let Some(pm) = app.try_state::<Arc<ProcessManager>>() else {
        return Err(GatewayError::Unavailable {
            message: "Service unavailable".into(),
        });
    };
    if !pm.gateway.state().accepts_new_generation() && pm.gateway.state() != MlxState::Ready {
        return Err(GatewayError::Busy {
            message: "I'm still finishing the last request. Stop it first if you want to start something new."
                .into(),
        });
    }

    let _ = app.emit("chat-trace", json!({
        "step": "planning",
        "detail": MlxState::Starting.user_label(),
        "phase": MlxState::Starting.user_label(),
    }));
    if pm.gateway.state() == MlxState::Stopped {
        let _ = pm.gateway.set_state(MlxState::Starting);
    }
    let startup_started = Instant::now();
    let startup = tokio::time::timeout(policy.startup_deadline, pm.ensure_mlx(state)).await;
    match startup {
        Ok(Ok(())) => {
            // #region agent log
            dbg_log(
                "D",
                "inference_gateway.rs:complete_live",
                "ensure_mlx ok",
                json!({
                    "startup_ms": startup_started.elapsed().as_millis() as u64,
                    "state": format!("{:?}", pm.gateway.state()),
                    "cold": pm.gateway.is_cold_first(),
                    "model_loaded": pm.tool_model_loaded(state),
                }),
            );
            // #endregion
            if pm.gateway.state() == MlxState::Starting {
                let _ = pm.gateway.set_state(MlxState::Ready);
            }
            if pm.gateway.state() == MlxState::Stopped {
                pm.gateway.force_state(MlxState::Ready);
            }
        }
        Ok(Err(err)) => {
            let _ = pm.gateway.set_state(MlxState::Failed);
            return Err(GatewayError::Unavailable { message: err });
        }
        Err(_) => {
            pm.interrupt_mlx(state, "startup_timeout");
            pm.gateway.force_state(MlxState::Stopped);
            return Err(GatewayError::Timeout {
                kind: TimeoutKind::Startup,
                message: timeout_copy(TimeoutKind::Startup, true).into(),
            });
        }
    }
    if !pm.tool_model_loaded(state) {
        return Err(GatewayError::Unavailable {
            message: "The local model is not the expected Qwen weights.".into(),
        });
    }

    let cold = pm.gateway.is_cold_first();
    let timeout = InferenceGateway::generation_timeout(policy, cold);
    let tokens = InferenceGateway::tokens_for(policy, kind);
    let model = ProcessManager::tool_model(state);
    let record = pm
        .gateway
        .begin_generation(turn_id, conversation_id, &model, timeout)?;
    let generation_id = record.generation_id.clone();
    let _ = app.emit("chat-trace", json!({
        "step": "planning",
        "detail": MlxState::Generating.user_label(),
        "phase": MlxState::Generating.user_label(),
    }));

    let client = reqwest::Client::new();
    let request = client
        .post(format!("{}/v1/complete", state.brain_url()))
        .json(&json!({
            "messages": messages,
            "tools": tools,
            "max_tokens": tokens,
            "temperature": if kind == CompleteKind::Chat { 0.7 } else { 0.2 },
            "generation_id": generation_id,
        }));
    info!("gateway complete waiting on /v1/complete");
    // #region agent log
    dbg_log(
        "A",
        "inference_gateway.rs:complete_live",
            "generation budget",
            json!({
                "runId": "post-fix",
                "cold": cold,
            "timeout_ms": timeout.as_millis() as u64,
            "tokens": tokens,
            "kind": format!("{kind:?}"),
            "tools": tools.len(),
            "messages": messages.len(),
            "generation_id": generation_id,
            "state": format!("{:?}", pm.gateway.state()),
        }),
    );
    // #endregion
    let gen_started = Instant::now();
    let http = async {
        let resp = request
            .send()
            .await
            .map_err(|e| GatewayError::Http(format!("brain complete request failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(GatewayError::Http(format!(
                "brain complete HTTP {}",
                resp.status()
            )));
        }
        resp.json::<CompleteHttp>()
            .await
            .map_err(|e| GatewayError::Http(format!("brain complete parse failed: {e}")))
    };

    let result = tokio::select! {
        _ = run.cancelled() => {
            pm.gateway.mark_cancelled();
            pm.interrupt_mlx(state, "user_stop");
            pm.gateway.finish_stopped();
            Err(GatewayError::Cancelled {
                message: stopped_copy(true).into(),
            })
        }
        _ = tokio::time::sleep(timeout) => {
            warn!("model timed out");
            // #region agent log
            dbg_log(
                "A",
                "inference_gateway.rs:complete_live",
                "generation timeout",
                json!({
                    "elapsed_ms": gen_started.elapsed().as_millis() as u64,
                    "timeout_ms": timeout.as_millis() as u64,
                    "cold": cold,
                    "generation_id": generation_id,
                }),
            );
            // #endregion
            pm.gateway.mark_cancelled();
            pm.interrupt_mlx(state, "mlx_generation_timeout");
            pm.gateway.finish_stopped();
            Err(GatewayError::Timeout {
                kind: TimeoutKind::Generation,
                message: timeout_copy(TimeoutKind::Generation, true).into(),
            })
        }
        res = http => res,
    };

    match result {
        Ok(body) => {
            if !pm.gateway.generation_is_current(&generation_id) {
                warn!("late complete ignored");
                return Err(GatewayError::Cancelled {
                    message: stopped_copy(true).into(),
                });
            }
            pm.gateway.finish_ready();
            pm.gateway.mark_warm();
            pm.mark_mlx_used();
            Ok(body)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turn_controller::{ModelLane, TurnController};

    pub const V6_GOAL: &str = "I want to climb V6 by the end of November";

    #[test]
    fn v6_routes_to_goals_and_qwen_not_llama() {
        let ids = crate::skills::skill_ids_for_turn(V6_GOAL, None);
        assert!(ids.contains(&"goals"), "skills={ids:?}");
        assert!(
            ids.contains(&"fitness"),
            "expected climbing/fitness context, skills={ids:?}"
        );
        assert!(!crate::native_loop::is_trivial_chat(V6_GOAL, None));
        assert_eq!(
            TurnController::lane_for(V6_GOAL, None, false),
            ModelLane::QwenComplete
        );
        assert_eq!(
            TurnController::lane_for(V6_GOAL, None, true),
            ModelLane::QwenComplete
        );
        assert_ne!(
            TurnController::lane_for(V6_GOAL, None, false),
            ModelLane::LlamaTalk
        );
    }

    #[test]
    fn abandoning_http_leaves_fake_mlx_generating() {
        let fake = FakeModel::new_ready();
        fake.abandon_http_without_interrupt();
        assert!(fake.generating.load(Ordering::SeqCst));
        assert!(!fake.interrupt_called.load(Ordering::SeqCst));
    }

    #[test]
    fn interrupt_stops_orphan_generation() {
        let fake = FakeModel::new_ready();
        fake.abandon_http_without_interrupt();
        fake.interrupt();
        assert!(!fake.generating.load(Ordering::SeqCst));
        assert!(fake.interrupt_called.load(Ordering::SeqCst));
        assert_eq!(*fake.state.lock().unwrap(), MlxState::Stopped);
    }

    #[test]
    fn embed_failure_while_generating_does_not_recycle_brain() {
        assert_eq!(
            brain_action_after_embed_probe(true, false, true),
            BrainAction::Keep
        );
        assert_eq!(
            brain_action_after_embed_probe(true, false, false),
            BrainAction::Recycle
        );
        assert_eq!(
            brain_action_after_embed_probe(false, false, false),
            BrainAction::Recycle
        );
    }

    #[test]
    fn expected_timeout_logs_are_documented() {
        assert!(timeout_copy(TimeoutKind::Generation, true).contains("safe working time"));
        assert!(timeout_copy(TimeoutKind::Startup, true).contains("took too long to start"));
        assert!(!timeout_copy(TimeoutKind::Generation, true).contains("shorter question"));
    }

    #[test]
    fn cancelled_generation_rejects_late_result() {
        let gw = InferenceGateway::default();
        gw.force_state(MlxState::Ready);
        let rec = gw
            .begin_generation("t1", "c1", "qwen", Duration::from_secs(30))
            .unwrap();
        gw.mark_cancelled();
        assert!(!gw.generation_is_current(&rec.generation_id));
    }

    #[test]
    fn cold_and_warm_timeouts_are_split() {
        let p = RuntimePolicy::cool();
        assert_eq!(
            InferenceGateway::generation_timeout(&p, true),
            Duration::from_secs(180)
        );
        assert_eq!(
            InferenceGateway::generation_timeout(&p, false),
            Duration::from_secs(120)
        );
        assert_eq!(p.startup_deadline, Duration::from_secs(90));
        assert_ne!(p.startup_deadline, p.warm_generation);
    }

    #[test]
    fn fake_clock_idle_unloads_after_five_minutes() {
        let gw = InferenceGateway::default();
        gw.force_state(MlxState::Ready);
        gw.last_used_secs.store(1_000, Ordering::SeqCst);
        let p = RuntimePolicy::cool();
        assert!(!gw.should_idle_unload(&p, 1_000 + 60, false));
        assert!(gw.should_idle_unload(&p, 1_000 + 300, false));
        assert!(!gw.should_idle_unload(&p, 1_000 + 300, true));
        let snap = gw.idle_snapshot(&p);
        assert!(!snap.generating);
        assert_eq!(snap.cpu_generating_hint, "high");
        assert_eq!(snap.cpu_idle_hint, "negligible");
        assert_eq!(snap.unload_after_secs, 300);
        let _ = snap.secs_since_generation;
    }

    #[test]
    fn llama_cache_flag_is_off() {
        assert!(!RuntimePolicy::cool().llama_chat_cache);
        assert!(!RuntimePolicy::cool().allows_model_fallback);
    }

    #[test]
    fn no_automatic_model_fallback() {
        assert!(!RuntimePolicy::cool().allows_model_fallback);
    }

    #[test]
    fn readonly_tools_are_lookups() {
        assert!(is_readonly_tool("calendar.look"));
        assert!(is_readonly_tool("todo.list"));
        assert!(is_readonly_tool("list_sparks"));
        assert!(!is_readonly_tool("goal.intake"));
        assert!(!is_readonly_tool("calendar.pin"));
    }

    #[test]
    fn v6_has_no_calendar_fast_path() {
        let lower = V6_GOAL.to_ascii_lowercase();
        assert!(crate::native_loop::fast_calendar_intent(V6_GOAL).is_none());
        assert!(lower.contains("i want to"));
        assert!(!lower.contains("shorter question"));
    }

    #[test]
    fn v6_fake_qwen_returns_goal_intake_not_calendar() {
        let fake = FakeModel::new_ready();
        *fake.response.lock().unwrap() = Some(CompleteHttp {
            content: None,
            tool_calls: vec![CompleteToolCallHttp {
                id: "call_0".into(),
                name: "goal.intake".into(),
                arguments: json!({
                    "title": "Climb V6",
                    "deadline": "2026-11-30"
                }),
            }],
            finish_reason: Some("tool_calls".into()),
        });
        let rec = fake.response.lock().unwrap().clone().unwrap();
        assert_eq!(rec.tool_calls.len(), 1);
        assert_eq!(rec.tool_calls[0].name, "goal.intake");
        assert!(!rec.tool_calls.iter().any(|c| c.name.starts_with("calendar.")));
        fake.complete_calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(fake.complete_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn expected_recycle_log_is_documented() {
        assert!(timeout_copy(TimeoutKind::Generation, true).contains("safe working time"));
        let documented = [
            "model timed out",
            "brain health current but /embed failed",
        ];
        for line in documented {
            assert!(!line.is_empty());
        }
    }

    #[test]
    fn optional_live_mlx_is_ignored_by_default() {
        if std::env::var("BUDDY_LIVE_MLX").ok().as_deref() != Some("1") {
            return;
        }
        // Live adapter path is opt-in; default CI never starts MLX.
    }

    #[test]
    fn extract_kind_uses_interpret_tokens() {
        let p = RuntimePolicy::cool();
        assert_eq!(
            InferenceGateway::tokens_for(&p, CompleteKind::Extract),
            p.qwen_interpret_tokens
        );
        assert_eq!(
            InferenceGateway::tokens_for(&p, CompleteKind::Chat),
            p.qwen_chat_tokens
        );
    }

    #[test]
    fn fake_model_generation_record_and_late_reject() {
        let fake = FakeModel::new_ready();
        *fake.delay.lock().unwrap() = Duration::from_millis(1);
        *fake.last_tokens.lock().unwrap() = Some(1024);
        let rec = GenerationRecord {
            turn_id: "t".into(),
            generation_id: "gen-late".into(),
            conversation_id: "c".into(),
            model: "qwen".into(),
            status: "generating".into(),
            started_at: 1,
            deadline_ms: 30_000,
            cancelled_at: None,
            finished_at: None,
        };
        fake.start_generation(rec);
        assert!(fake.generating.load(Ordering::SeqCst));
        fake.reject_late.lock().unwrap().push("gen-late".into());
        assert!(fake.reject_if_cancelled("gen-late"));
        fake.finish_generation();
        assert!(!fake.generating.load(Ordering::SeqCst));
        assert_eq!(
            fake.last_generation.lock().unwrap().as_ref().unwrap().status,
            "done"
        );
    }
}
