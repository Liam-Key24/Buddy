use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::state::AppState;

/// Stop the 8B server shortly after chat so it cannot sit at 100% CPU.
const MLX_IDLE_SECS: u64 = 90;

/// Brain `/health.api` required for Talk + native complete.
pub const BRAIN_API_MIN: u32 = 2;

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub mlx: bool,
    pub brain: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStepResult {
    pub ok: bool,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStartResult {
    pub brain: ServiceStepResult,
    pub mlx: ServiceStepResult,
}

fn step_ok(code: &str, message: impl Into<String>) -> ServiceStepResult {
    ServiceStepResult {
        ok: true,
        code: code.into(),
        message: message.into(),
    }
}

fn step_err(kind: &str, err: impl Into<String>) -> ServiceStepResult {
    let message = err.into();
    ServiceStepResult {
        ok: false,
        code: classify_service_error(kind, &message),
        message,
    }
}

fn classify_service_error(kind: &str, err: &str) -> String {
    let e = err.to_ascii_lowercase();
    let suffix = if e.contains("directory missing") || e.contains("project root") {
        "DIR"
    } else if e.contains("still in use") {
        "PORT"
    } else if e.contains("not found") || e.contains("venv") || e.contains("pip install") {
        "BIN"
    } else if e.contains("/embed") {
        "EMBED"
    } else if e.contains("/health") || e.contains("/v1/models") || e.contains("never became ready") {
        "HEALTH"
    } else if e.contains("failed to start") || e.contains("os error") {
        "START"
    } else {
        "FAIL"
    };
    format!("{kind}_{suffix}")
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct BrainHealth {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub api: u32,
    #[serde(default)]
    pub routes: Vec<String>,
}

impl BrainHealth {
    pub fn from_json(raw: &str) -> Self {
        serde_json::from_str(raw).unwrap_or_default()
    }

    pub fn is_current(&self) -> bool {
        self.api >= BRAIN_API_MIN
            && self.has_route("/chat/talk")
            && self.has_route("/v1/complete")
            && self.has_route("/embed")
    }

    pub fn supports_talk(&self) -> bool {
        self.api >= BRAIN_API_MIN && self.has_route("/chat/talk")
    }

    fn has_route(&self, name: &str) -> bool {
        self.routes.iter().any(|r| r == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TalkRecovery {
    RecycleBrain,
    FallbackRespond,
    GiveUp,
}

/// 404 / connection → recycle once, then `/chat/respond`. Other errors fail.
pub fn talk_recovery(err: &str, already_recycled: bool) -> TalkRecovery {
    if !talk_error_is_stale(err) {
        return TalkRecovery::GiveUp;
    }
    if already_recycled {
        TalkRecovery::FallbackRespond
    } else {
        TalkRecovery::RecycleBrain
    }
}

pub fn talk_error_is_stale(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    e.contains("404")
        || e.contains("connection refused")
        || e.contains("connection reset")
        || e.contains("connect error")
        || e.contains("error sending request")
        || e.contains("request failed")
}

pub struct ProcessManager {
    brain_child: Mutex<Option<Child>>,
    mlx_child: Mutex<Option<Child>>,
    mlx_owned: AtomicBool,
    brain_owned: AtomicBool,
    brain_ensure_lock: tokio::sync::Mutex<()>,
    mlx_ensure_lock: tokio::sync::Mutex<()>,
    last_mlx_used_secs: AtomicU64,
    mlx_loaded_model: Mutex<Option<String>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            brain_child: Mutex::new(None),
            mlx_child: Mutex::new(None),
            mlx_owned: AtomicBool::new(false),
            brain_owned: AtomicBool::new(false),
            brain_ensure_lock: tokio::sync::Mutex::new(()),
            mlx_ensure_lock: tokio::sync::Mutex::new(()),
            last_mlx_used_secs: AtomicU64::new(0),
            mlx_loaded_model: Mutex::new(None),
        }
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub fn mark_mlx_used(&self) {
        self.last_mlx_used_secs
            .store(Self::now_secs(), Ordering::SeqCst);
    }

    /// Kill leftover MLX on our port (previous run / manual server) so launch
    /// does not inherit a 100% CPU Python process.
    pub fn reclaim_mlx_port(&self, state: &AppState) {
        let port = Self::mlx_port(state);
        if Self::pids_on_port(port).is_empty() {
            return;
        }
        info!(port, "stopping leftover mlx (chat-only)");
        self.stop_mlx();
        Self::clear_port(port);
    }

    /// Stop an owned MLX process after chat goes idle (unless keep-warm is on).
    pub fn spawn_idle_watcher(self: &Arc<Self>, state: Arc<AppState>) {
        let pm = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            loop {
                interval.tick().await;
                if state.runs.active().is_some() {
                    pm.mark_mlx_used();
                    continue;
                }
                if ProcessManager::auto_start_mlx(&state) {
                    continue;
                }
                if !pm.mlx_owned.load(Ordering::SeqCst) {
                    continue;
                }
                let last = pm.last_mlx_used_secs.load(Ordering::SeqCst);
                if last == 0 {
                    continue;
                }
                let idle = ProcessManager::now_secs().saturating_sub(last);
                if idle >= MLX_IDLE_SECS {
                    info!(idle_secs = idle, "stopping idle mlx");
                    pm.stop_mlx();
                }
            }
        });
    }

    fn brain_port(state: &AppState) -> u16 {
        Self::port_from_url(&state.brain_url(), 8002)
    }

    fn mlx_port(state: &AppState) -> u16 {
        Self::port_from_url(&state.mlx_url(), 8001)
    }

    fn port_from_url(url: &str, default: u16) -> u16 {
        url.rsplit(':')
            .next()
            .and_then(|p| p.trim_end_matches('/').parse().ok())
            .unwrap_or(default)
    }

    fn pids_on_port(port: u16) -> Vec<String> {
        let Ok(output) = Command::new("lsof")
            .args(["-ti", &format!("tcp:{port}")])
            .output()
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    fn kill_processes_on_port(port: u16, signal: &str) {
        for pid in Self::pids_on_port(port) {
            info!(%pid, port, signal, "killing process on service port");
            let _ = Command::new("kill").args([signal, &pid]).status();
        }
    }

    fn clear_port(port: u16) {
        Self::kill_processes_on_port(port, "-TERM");
        std::thread::sleep(std::time::Duration::from_millis(400));
        if !Self::pids_on_port(port).is_empty() {
            Self::kill_processes_on_port(port, "-KILL");
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    pub async fn check_mlx(state: &AppState) -> bool {
        // /health is cheap; /v1/models walks the Hugging Face cache.
        let url = format!("{}/health", state.mlx_url());
        match reqwest::Client::new()
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                tracing::debug!(error = %e, "mlx health check failed");
                false
            }
        }
    }

    pub async fn fetch_brain_health(state: &AppState) -> Option<BrainHealth> {
        let url = format!("{}/health", state.brain_url());
        match reqwest::Client::new()
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp.json::<BrainHealth>().await.ok(),
            Ok(_) => None,
            Err(e) => {
                tracing::debug!(error = %e, "brain health check failed");
                None
            }
        }
    }

    pub async fn check_brain(state: &AppState) -> bool {
        Self::fetch_brain_health(state).await.is_some()
    }

    pub async fn check_brain_current(state: &AppState) -> bool {
        Self::fetch_brain_health(state)
            .await
            .is_some_and(|h| h.is_current())
    }

    pub async fn check_brain_embed(state: &AppState) -> bool {
        Self::check_brain_embed_with_timeout(state, 2).await
    }

    pub async fn check_brain_embed_with_timeout(state: &AppState, timeout_secs: u64) -> bool {
        let url = format!("{}/embed", state.brain_url());
        match reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({ "text": "ping" }))
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                tracing::debug!(error = %e, "brain embed check failed");
                false
            }
        }
    }

    pub async fn check_brain_ready(state: &AppState) -> bool {
        Self::check_brain_current(state).await && Self::check_brain_embed(state).await
    }

    pub async fn get_status(state: &AppState) -> ServiceStatus {
        let mlx = Self::check_mlx(state).await;
        let brain = Self::check_brain_current(state).await;
        ServiceStatus { mlx, brain }
    }

    pub fn auto_start_mlx(state: &AppState) -> bool {
        state
            .db
            .get_setting_or("auto_start_mlx", "false")
            .eq_ignore_ascii_case("true")
    }

    const QWEN14_HF: &str = "mlx-community/Qwen3-14B-4bit";
    const QWEN14_S1_BYTES: u64 = 5_354_381_380;
    const QWEN14_S2_BYTES: u64 = 2_953_517_134;

    fn model_name(state: &AppState) -> String {
        state
            .db
            .get_setting_or("model_name", Self::QWEN14_HF)
    }

    /// Prefer the on-disk 14B snapshot so MLX does not hit Hugging Face again.
    fn resolve_mlx_model(state: &AppState, model: &str) -> String {
        let trimmed = model.trim();
        if trimmed == Self::QWEN14_HF || trimmed.ends_with("Qwen3-14B-4bit") {
            let local = state.project_root.join("brain/models/Qwen3-14B-4bit");
            if Self::qwen14_local_ready(&local) {
                return local.to_string_lossy().into_owned();
            }
        }
        trimmed.to_string()
    }

    fn qwen14_local_ready(dir: &std::path::Path) -> bool {
        if !dir.join("config.json").is_file() {
            return false;
        }
        let shards = [
            ("model-00001-of-00002.safetensors", Self::QWEN14_S1_BYTES),
            ("model-00002-of-00002.safetensors", Self::QWEN14_S2_BYTES),
        ];
        for (name, size) in shards {
            match std::fs::metadata(dir.join(name)) {
                Ok(meta) if meta.len() == size => {}
                _ => return false,
            }
        }
        true
    }

    pub fn tool_model(state: &AppState) -> String {
        Self::model_name(state)
    }

    pub fn chat_model(state: &AppState) -> String {
        state
            .db
            .get_setting_or("model_name_chat", crate::state::LLAMA_CHAT_MODEL)
    }

    pub fn tool_model_loaded(&self, state: &AppState) -> bool {
        let Some(loaded) = self.loaded_mlx_model() else {
            return false;
        };
        let tool = Self::tool_model(state);
        loaded == tool || loaded.contains("Qwen3-14B")
    }

    fn loaded_mlx_model(&self) -> Option<String> {
        self.mlx_loaded_model.lock().ok().and_then(|g| g.clone())
    }

    fn set_loaded_mlx_model(&self, model: Option<String>) {
        if let Ok(mut guard) = self.mlx_loaded_model.lock() {
            *guard = model;
        }
    }

    fn venv_bin(state: &AppState, name: &str) -> PathBuf {
        state.project_root.join("brain/venv/bin").join(name)
    }

    pub async fn ensure_brain(&self, state: &AppState) -> Result<(), String> {
        let _guard = self.brain_ensure_lock.lock().await;
        if Self::check_brain_ready(state).await {
            return Ok(());
        }

        if let Some(health) = Self::fetch_brain_health(state).await {
            if health.is_current() {
                if Self::check_brain_embed_with_timeout(state, 60).await {
                    return Ok(());
                }
                warn!("brain health current but /embed failed — recycling");
            } else {
                info!(
                    api = health.api,
                    routes = health.routes.len(),
                    "stale brain — recycling for /chat/talk"
                );
            }
        }

        self.restart_brain_process(state)?;
        self.wait_brain_current(state).await?;

        if Self::check_brain_embed_with_timeout(state, 60).await {
            return Ok(());
        }

        Err("brain started but /embed endpoint not available".into())
    }

    /// Kill and restart Brain; wait until `/health` is current (Talk retry path).
    pub async fn recycle_brain(&self, state: &AppState) -> Result<(), String> {
        let _guard = self.brain_ensure_lock.lock().await;
        info!("recycling brain");
        self.restart_brain_process(state)?;
        self.wait_brain_current(state).await
    }

    /// Settings / sidebar: always recycle, then wait for `/embed`.
    pub async fn restart_brain(&self, state: &AppState) -> Result<(), String> {
        self.recycle_brain(state).await?;
        if Self::check_brain_embed_with_timeout(state, 60).await {
            return Ok(());
        }
        Err("brain restarted but /embed endpoint not available".into())
    }

    /// Settings: stop MLX and load the Tool model again.
    pub async fn restart_mlx(&self, state: &AppState) -> Result<(), String> {
        let _guard = self.mlx_ensure_lock.lock().await;
        let model = Self::tool_model(state);
        info!(%model, "user restart mlx");
        self.stop_mlx();
        Self::clear_port(Self::mlx_port(state));
        self.start_mlx_model(state, &model)?;
        for i in 0..180 {
            if Self::check_mlx(state).await {
                info!(attempt = i + 1, "mlx is ready after restart");
                self.mark_mlx_used();
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Err("mlx restarted but /v1/models never became ready".into())
    }

    /// Start Brain and Qwen together. One failure does not skip the other.
    pub async fn start_runtime(&self, state: &AppState) -> RuntimeStartResult {
        let root = state.project_root.display().to_string();
        let brain = match self.ensure_brain(state).await {
            Ok(()) => step_ok("BRAIN_OK", format!("Brain ready ({root})")),
            Err(err) => step_err("BRAIN", format!("{err} (root: {root})")),
        };
        let mlx = match self.ensure_mlx(state).await {
            Ok(()) => step_ok("MLX_OK", format!("MLX ready ({root})")),
            Err(err) => step_err("MLX", format!("{err} (root: {root})")),
        };
        RuntimeStartResult { brain, mlx }
    }

    fn restart_brain_process(&self, state: &AppState) -> Result<(), String> {
        let port = Self::brain_port(state);
        self.stop_brain();
        Self::clear_port(port);
        self.start_brain(state)
    }

    async fn wait_brain_current(&self, state: &AppState) -> Result<(), String> {
        for _ in 0..120 {
            if Self::check_brain_current(state).await {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        Err("brain recycled but /health is not current (missing /chat/talk?)".into())
    }

    pub async fn ensure_mlx(&self, state: &AppState) -> Result<(), String> {
        self.ensure_mlx_model(state, &Self::tool_model(state)).await
    }

    pub async fn ensure_chat_mlx(&self, state: &AppState) -> Result<(), String> {
        self.ensure_mlx_model(state, &Self::chat_model(state)).await
    }

    pub async fn ensure_mlx_model(&self, state: &AppState, model: &str) -> Result<(), String> {
        let _guard = self.mlx_ensure_lock.lock().await;
        self.mark_mlx_used();
        let up = Self::check_mlx(state).await;
        let loaded = self.loaded_mlx_model();
        if up && loaded.as_deref() == Some(model) {
            return Ok(());
        }
        if up {
            info!(
                from = loaded.as_deref().unwrap_or("unknown"),
                to = model,
                "switching mlx model"
            );
        }

        let port = Self::mlx_port(state);
        self.stop_mlx();
        Self::clear_port(port);
        self.start_mlx_model(state, model)?;

        // Model load / first download can take a while.
        for i in 0..180 {
            if Self::check_mlx(state).await {
                info!(attempt = i + 1, "mlx is ready");
                self.mark_mlx_used();
                return Ok(());
            }
            if i % 10 == 9 {
                info!(attempt = i + 1, "waiting for mlx to become ready…");
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        Err("mlx started but /v1/models never became ready (model may still be downloading)".into())
    }

    pub fn start_brain(&self, state: &AppState) -> Result<(), String> {
        let port = Self::brain_port(state);
        let mut guard = self.brain_child.lock().map_err(|e| e.to_string())?;

        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Self::clear_port(port);

        if !Self::pids_on_port(port).is_empty() {
            return Err(format!("brain port {port} still in use after cleanup"));
        }

        let brain_dir = state.project_root.join("brain");
        if !brain_dir.is_dir() {
            return Err(format!(
                "brain directory missing at {}. Set project root or run Buddy from the repo once.",
                brain_dir.display()
            ));
        }

        let venv_python = Self::venv_bin(state, "python");
        let uvicorn = Self::venv_bin(state, "uvicorn");
        let reload = cfg!(debug_assertions);

        let (program, args): (String, Vec<String>) = if uvicorn.exists() {
            (
                uvicorn.to_string_lossy().into_owned(),
                Self::uvicorn_args(port, reload, false),
            )
        } else if venv_python.exists() {
            (
                venv_python.to_string_lossy().into_owned(),
                Self::uvicorn_args(port, reload, true),
            )
        } else {
            (
                "python3".into(),
                Self::uvicorn_args(port, reload, true),
            )
        };

        info!(program = %program, port, "starting brain process");
        let child = Command::new(&program)
            .args(&args)
            .current_dir(&brain_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start brain: {e}"))?;

        *guard = Some(child);
        self.brain_owned.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn uvicorn_args(port: u16, reload: bool, module: bool) -> Vec<String> {
        let mut args = Vec::new();
        if module {
            args.extend(["-m".into(), "uvicorn".into()]);
        }
        args.extend([
            "main:app".into(),
            "--host".into(),
            "127.0.0.1".into(),
            "--port".into(),
            port.to_string(),
        ]);
        if reload {
            args.push("--reload".into());
        }
        args
    }

    pub fn start_mlx(&self, state: &AppState) -> Result<(), String> {
        self.start_mlx_model(state, &Self::tool_model(state))
    }

    pub fn start_mlx_model(&self, state: &AppState, model: &str) -> Result<(), String> {
        let port = Self::mlx_port(state);
        let requested = model.to_string();
        let model = Self::resolve_mlx_model(state, model);
        let mut guard = self.mlx_child.lock().map_err(|e| e.to_string())?;

        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Self::clear_port(port);

        if !Self::pids_on_port(port).is_empty() {
            return Err(format!("mlx port {port} still in use after cleanup"));
        }

        let brain_dir = state.project_root.join("brain");
        if !brain_dir.is_dir() {
            return Err(format!(
                "brain directory missing at {}. Buddy needs the repo (with brain/venv) to start MLX.",
                brain_dir.display()
            ));
        }

        let server_bin = Self::venv_bin(state, "mlx_lm.server");
        let venv_python = Self::venv_bin(state, "python");
        let start_script = brain_dir.join("scripts/start_mlx.sh");

        let (program, args, cwd): (String, Vec<String>, PathBuf) = if server_bin.exists() {
            (
                server_bin.to_string_lossy().into_owned(),
                vec![
                    "--model".into(),
                    model.clone(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
                brain_dir.clone(),
            )
        } else if venv_python.exists() {
            (
                venv_python.to_string_lossy().into_owned(),
                vec![
                    "-m".into(),
                    "mlx_lm.server".into(),
                    "--model".into(),
                    model.clone(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
                brain_dir.clone(),
            )
        } else if start_script.exists() {
            (
                "/bin/bash".into(),
                vec![start_script.to_string_lossy().into_owned()],
                brain_dir.clone(),
            )
        } else {
            return Err(format!(
                "MLX server not found under {}. Create brain/venv and pip install -r brain/requirements.txt",
                Self::venv_bin(state, "").display()
            ));
        };

        info!(program = %program, requested = %requested, %model, port, "starting mlx process");
        let child = Command::new(&program)
            .args(&args)
            .current_dir(&cwd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start mlx: {e}"))?;

        *guard = Some(child);
        self.mlx_owned.store(true, Ordering::SeqCst);
        self.set_loaded_mlx_model(Some(requested));
        Ok(())
    }

    pub fn stop_brain(&self) {
        if let Ok(mut guard) = self.brain_child.lock() {
            if let Some(mut child) = guard.take() {
                info!("stopping brain process");
                if let Err(e) = child.kill() {
                    warn!(error = %e, "failed to kill brain process");
                }
                let _ = child.wait();
            }
        }
        self.brain_owned.store(false, Ordering::SeqCst);
    }

    pub fn stop_mlx(&self) {
        if let Ok(mut guard) = self.mlx_child.lock() {
            if let Some(mut child) = guard.take() {
                info!("stopping mlx process");
                if let Err(e) = child.kill() {
                    warn!(error = %e, "failed to kill mlx process");
                }
                let _ = child.wait();
            }
        }
        self.mlx_owned.store(false, Ordering::SeqCst);
        self.set_loaded_mlx_model(None);
    }

    /// Abort an in-flight generation (Stop). mlx_lm.server cannot cancel a request otherwise.
    pub fn interrupt_mlx(&self, state: &AppState) {
        info!("interrupting mlx generation");
        self.stop_mlx();
        Self::clear_port(Self::mlx_port(state));
    }

    pub fn stop_owned_services(&self) {
        if self.brain_owned.load(Ordering::SeqCst) {
            self.stop_brain();
        }
        if self.mlx_owned.load(Ordering::SeqCst) {
            self.stop_mlx();
        }
    }

    pub fn project_root_looks_valid(root: &Path) -> bool {
        root.join("brain").is_dir()
            && (root.join("app").is_dir() || root.join("Cargo.toml").is_file())
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.stop_owned_services();
    }
}

#[cfg(test)]
mod brain_health_tests {
    use super::{
        talk_error_is_stale, talk_recovery, BrainHealth, TalkRecovery, BRAIN_API_MIN,
    };

    #[test]
    fn legacy_health_is_stale() {
        let h = BrainHealth::from_json(r#"{"status":"ok"}"#);
        assert_eq!(h.api, 0);
        assert!(!h.is_current());
        assert!(!h.supports_talk());
    }

    #[test]
    fn current_health_lists_talk() {
        let h = BrainHealth::from_json(
            r#"{"status":"ok","api":2,"routes":["/chat/talk","/v1/complete","/embed"]}"#,
        );
        assert!(h.api >= BRAIN_API_MIN);
        assert!(h.supports_talk());
        assert!(h.is_current());
    }

    #[test]
    fn api2_without_talk_is_stale() {
        let h = BrainHealth::from_json(
            r#"{"status":"ok","api":2,"routes":["/v1/complete","/embed"]}"#,
        );
        assert!(!h.supports_talk());
        assert!(!h.is_current());
    }

    #[test]
    fn talk_404_recycles_then_falls_back() {
        let err = "brain talk HTTP 404 Not Found";
        assert!(talk_error_is_stale(err));
        assert_eq!(talk_recovery(err, false), TalkRecovery::RecycleBrain);
        assert_eq!(talk_recovery(err, true), TalkRecovery::FallbackRespond);
    }

    #[test]
    fn talk_connection_recycles() {
        let err = "brain talk request failed: error sending request for url (http://127.0.0.1:8002/chat/talk)";
        assert_eq!(talk_recovery(err, false), TalkRecovery::RecycleBrain);
        assert_eq!(talk_recovery(err, true), TalkRecovery::FallbackRespond);
    }

    #[test]
    fn talk_500_gives_up() {
        assert_eq!(
            talk_recovery("brain talk HTTP 500 Internal Server Error", false),
            TalkRecovery::GiveUp
        );
    }
}


