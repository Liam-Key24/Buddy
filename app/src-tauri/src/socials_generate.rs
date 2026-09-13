use std::collections::HashSet;
use std::sync::Arc;

use buddy_database::{local_today, week_commencing_monday, SocialPost, SocialWeeklyPlan};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

use crate::inference_gateway::{self, CompleteKind};
use crate::run_control::RunScope;
use crate::runtime_policy::RuntimePolicy;
use crate::services::ProcessManager;
use crate::state::AppState;

const TEMPERATURE: f32 = 0.75;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GeneratedSlot {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub suggested_media: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub idea_id: Option<String>,
}

const SYSTEM_PROMPT: &str = r#"You write a week of social posts for one person.

Ground every post in the provided current story, story threads, active projects, unused content ideas, open drafts, and user notes. Do not invent shipped work, metrics, customers, or GitHub activity. Do not repeat recent published posts. Skip any idea marked used.

Voice: intelligent, curious, honest, technical when it helps, personal, practical, occasionally humorous. Not corporate or motivational-guru. The journey is the content.

LinkedIn: 120–220 words, first person, one specific beat, short paragraphs.
X: max 280 characters, one idea, no hashtag spam.

Return ONLY a JSON array. Each item:
{"id":"<slot id exactly>","body":"...","category":"building|money|security|study|personal","purpose":"why this post","suggested_media":"optional","thread_id":"building_apps|making_money|app_security|cloud_security|self_study","idea_id":"idea id if you used one, else empty"}

Use each unused idea at most once. Prefer turning unused ideas into posts before inventing new angles. If user notes ask for edits, apply them."#;

pub async fn generate_week(
    app: &AppHandle,
    state: &Arc<AppState>,
    week_start: Option<String>,
    notes: Option<String>,
    remake: bool,
) -> Result<SocialWeeklyPlan, String> {
    let pm = app
        .try_state::<Arc<ProcessManager>>()
        .ok_or_else(|| "process manager unavailable".to_string())?;
    pm.ensure_brain(state).await?;
    pm.ensure_mlx(state).await?;

    let week = week_start
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| week_commencing_monday(&local_today()));
    let notes = notes.unwrap_or_default();

    let mut context = state
        .db
        .gather_social_context()
        .map_err(|e| e.to_string())?;
    if let Some(t) = state.db.format_open_todos_context() {
        context.push_str(&format!("\n{t}\n"));
    }
    if let Some(s) = state.db.format_study_digest() {
        context.push_str(&format!("\n{s}\n"));
    }
    if let Some(r) = state.db.format_research_digest() {
        context.push_str(&format!("\n{r}\n"));
    }
    if !notes.trim().is_empty() {
        context.push_str(&format!("\n## User notes / edits\n{}\n", notes.trim()));
    }

    let plan = state
        .db
        .ensure_week_plan(&week, notes.trim(), &context)
        .map_err(|e| e.to_string())?;

    let writable: Vec<SocialPost> = plan
        .posts
        .iter()
        .filter(|p| p.platform != "github")
        .filter(|p| p.calendar_event_id.is_none() && p.status != "published")
        .filter(|p| remake || p.body.trim().is_empty())
        .cloned()
        .collect();

    if remake {
        state
            .db
            .clear_posts_for_remake(&plan.id)
            .map_err(|e| e.to_string())?;
        emit_socials(app);
    }

    if writable.is_empty() {
        return Ok(state.db.get_social_plan(&plan.id).map_err(|e| e.to_string())?);
    }

    let threads = state.db.list_social_threads().unwrap_or_default();
    let thread_ids: Vec<String> = threads.iter().map(|t| t.id.clone()).collect();
    let unused_idea_ids: HashSet<String> = state
        .db
        .list_unused_social_ideas()
        .unwrap_or_default()
        .into_iter()
        .map(|i| i.id)
        .collect();

    let policy = RuntimePolicy::cool();
    let scope = RunScope::try_start(&state.runs, &format!("socials:{week}"))
        .map_err(|e| format!("I'm still finishing the last request ({})", e.conversation_id))?;

    let mut batches: Vec<Vec<SocialPost>> = Vec::new();
    let linkedin: Vec<_> = writable
        .iter()
        .filter(|p| p.platform == "linkedin")
        .cloned()
        .collect();
    if !linkedin.is_empty() {
        batches.push(linkedin);
    }
    let mut x_posts: Vec<_> = writable
        .iter()
        .filter(|p| p.platform == "x")
        .cloned()
        .collect();
    x_posts.sort_by(|a, b| a.slot_date.cmp(&b.slot_date).then(a.slot_time.cmp(&b.slot_time)));
    for chunk in x_posts.chunks(5) {
        batches.push(chunk.to_vec());
    }

    let mut used_ideas: HashSet<String> = HashSet::new();
    let mut last_err: Option<String> = None;

    for batch in batches {
        let user = build_user_prompt(&context, &batch, remake, &notes);
        match complete_posts(app, state, &scope.guard, &policy, &user).await {
            Ok(generated) => {
                apply_generated(
                    state,
                    &batch,
                    &generated,
                    &thread_ids,
                    &unused_idea_ids,
                    &mut used_ideas,
                )?;
                emit_socials(app);
            }
            Err(e) => {
                warn!(error = %e, count = batch.len(), "social generate batch failed");
                last_err = Some(e);
            }
        }
    }

    for id in used_ideas {
        let _ = state.db.mark_social_idea_used(&id);
    }

    let out = state.db.get_social_plan(&plan.id).map_err(|e| e.to_string())?;
    if out.posts.iter().all(|p| p.body.trim().is_empty()) {
        return Err(last_err.unwrap_or_else(|| "model returned no posts".into()));
    }
    info!(
        week = %week,
        remake,
        filled = out.posts.iter().filter(|p| !p.body.trim().is_empty()).count(),
        "social week generated"
    );
    Ok(out)
}

fn emit_socials(app: &AppHandle) {
    let _ = app.emit("socials-updated", ());
}

fn build_user_prompt(context: &str, batch: &[SocialPost], remake: bool, notes: &str) -> String {
    let mut slots = String::new();
    for p in batch {
        slots.push_str(&format!(
            "- id={} platform={} date={} time={} current_body={}\n",
            p.id,
            p.platform,
            p.slot_date,
            p.slot_time,
            if remake && !p.body.trim().is_empty() {
                json!(p.body).to_string()
            } else {
                "\"\"".into()
            }
        ));
    }
    let mode = if remake {
        "Remake these slots as fresh posts. Apply user notes/edits to the current bodies when provided. Do not copy them verbatim."
    } else {
        "Write new posts for these empty slots."
    };
    format!(
        "{context}\n\n## Mode\n{mode}\nNotes: {}\n\n## Slots to fill (return one JSON object per id)\n{slots}",
        notes.trim()
    )
}

async fn complete_posts(
    app: &AppHandle,
    state: &AppState,
    run: &crate::run_control::RunGuard,
    policy: &RuntimePolicy,
    user: &str,
) -> Result<Vec<GeneratedSlot>, String> {
    let parsed = inference_gateway::complete_live(
        app,
        state,
        run,
        policy,
        &[
            json!({"role": "system", "content": SYSTEM_PROMPT}),
            json!({"role": "user", "content": user}),
        ],
        &[],
        CompleteKind::Socials,
        "socials",
        "socials-week",
    )
    .await
    .map_err(|e| e.as_str().to_string())?;
    parse_generated_posts(parsed.content.as_deref().unwrap_or(""))
}

fn apply_generated(
    state: &AppState,
    batch: &[SocialPost],
    generated: &[GeneratedSlot],
    thread_ids: &[String],
    unused_idea_ids: &HashSet<String>,
    used_ideas: &mut HashSet<String>,
) -> Result<(), String> {
    for (i, post) in batch.iter().enumerate() {
        let slot = generated
            .iter()
            .find(|g| g.id == post.id)
            .or_else(|| generated.get(i));
        let Some(slot) = slot else { continue };
        let body = slot.body.trim();
        if body.is_empty() {
            continue;
        }
        let mut next = post.clone();
        next.body = body.to_string();
        if !slot.category.trim().is_empty() {
            next.category = slot.category.trim().to_string();
        }
        if !slot.purpose.trim().is_empty() {
            next.purpose = slot.purpose.trim().to_string();
        }
        next.suggested_media = slot.suggested_media.trim().to_string();
        if let Some(tid) = slot
            .thread_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && thread_ids.iter().any(|id| id == s))
        {
            next.thread_id = Some(tid.to_string());
        }
        next.status = "proposed".into();
        state.db.update_social_post(next).map_err(|e| e.to_string())?;

        if let Some(idea) = slot
            .idea_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && unused_idea_ids.contains(*s))
        {
            used_ideas.insert(idea.to_string());
        }
    }
    Ok(())
}

pub fn parse_generated_posts(raw: &str) -> Result<Vec<GeneratedSlot>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty model response".into());
    }
    let unfenced = strip_fence(trimmed);
    let value = extract_json_value(&unfenced).ok_or_else(|| "model did not return JSON".to_string())?;
    let arr = match value {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("posts")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| "JSON object missing posts array".to_string())?,
        _ => return Err("JSON was not an array".into()),
    };
    let mut out = Vec::new();
    for item in arr {
        match serde_json::from_value::<GeneratedSlot>(item) {
            Ok(slot) if !slot.body.trim().is_empty() => out.push(slot),
            _ => {}
        }
    }
    if out.is_empty() {
        return Err("no post bodies in model JSON".into());
    }
    Ok(out)
}

fn strip_fence(text: &str) -> String {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("```json") {
        return rest
            .strip_suffix("```")
            .unwrap_or(rest)
            .trim()
            .to_string();
    }
    if let Some(rest) = t.strip_prefix("```") {
        return rest
            .strip_suffix("```")
            .unwrap_or(rest)
            .trim()
            .to_string();
    }
    t.to_string()
}

fn extract_json_value(text: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return Some(v);
    }
    let bytes = text.as_bytes();
    let start = bytes.iter().position(|c| *c == b'[' || *c == b'{')?;
    let open = bytes[start];
    let close = if open == b'[' { b']' } else { b'}' };
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, ch) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escape {
                escape = false;
            } else if *ch == b'\\' {
                escape = true;
            } else if *ch == b'"' {
                in_str = false;
            }
            continue;
        }
        match *ch {
            b'"' => in_str = true,
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return serde_json::from_str(&text[start..=i]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_array() {
        let raw = r#"```json
[{"id":"a","body":"Shipped auth on the laptop.","category":"building","idea_id":"i1"}]
```"#;
        let posts = parse_generated_posts(raw).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, "a");
        assert!(posts[0].body.contains("auth"));
        assert_eq!(posts[0].idea_id.as_deref(), Some("i1"));
    }

    #[test]
    fn parses_wrapped_posts_object() {
        let raw = r#"Here you go: {"posts":[{"id":"x","body":"Local models only."}]}"#;
        let posts = parse_generated_posts(raw).unwrap();
        assert_eq!(posts[0].body, "Local models only.");
    }
}
