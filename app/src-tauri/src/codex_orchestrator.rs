//! Orchestrates a Code Agent turn: builds the prompt from focus mode and file
//! attachments, runs the selected backend (Codex or Cursor), and persists the
//! exchange. Fully separate from the Brain/MLX path used by Buddy chat.

use std::path::PathBuf;

use buddy_core::PathGuard;
use tauri::{AppHandle, Emitter};
use tracing::warn;

use crate::codex_runner;
use crate::cursor_runner;
use crate::state::AppState;

const ATTACHMENT_MAX_LINES: usize = 400;
const DEFAULT_EXCLUSIONS: &[&str] = &[
    "Library", ".Trash", ".ssh", ".gnupg", ".cache", "Pictures",
];

fn focus_preamble(focus: &str) -> &'static str {
    match focus {
        "planning" => {
            "Focus: PLANNING. Brainstorm with the user, ask clarifying questions, and propose \
             structure or an approach. Do not write or change code unless explicitly asked."
        }
        "asking" => {
            "Focus: ASKING. Explain and answer questions about the code and project. Do not modify \
             any files."
        }
        "debugging" => {
            "Focus: DEBUGGING. Diagnose the reported error, inspect relevant files and logs, and \
             suggest the smallest correct fix."
        }
        "focused" => {
            "Focus: FOCUSED. Implement the requested change directly with minimal prose."
        }
        _ => "Focus: PLANNING. Brainstorm and clarify before making changes.",
    }
}

fn excluded_paths(state: &AppState) -> Vec<String> {
    state
        .db
        .get_setting("fs_excluded_paths")
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_else(|| DEFAULT_EXCLUSIONS.iter().map(|s| s.to_string()).collect())
}

fn read_attachment(guard: &PathGuard, path: &str) -> Option<String> {
    let resolved = guard.check(path).ok()?;
    let content = std::fs::read_to_string(&resolved).ok()?;
    let preview: String = content
        .lines()
        .take(ATTACHMENT_MAX_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!("--- File: {} ---\n{preview}\n", resolved.display()))
}

fn slugify(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    let collapsed: String = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        format!("project-{}", buddy_database::chrono_now())
    } else {
        collapsed.chars().take(48).collect()
    }
}

/// Resolve (and create) the workspace directory for a code conversation.
/// New projects land under the configured base (default `~/Desktop`).
fn resolve_workspace(
    state: &AppState,
    conversation: &buddy_database::Conversation,
) -> Result<PathBuf, String> {
    if let Some(existing) = &conversation.workspace_path {
        let path = PathBuf::from(existing);
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        return Ok(path);
    }

    let base = state
        .db
        .get_setting("codex_workspace")
        .ok()
        .flatten()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Desktop")
        });

    let dir = base.join(slugify(&conversation.title));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let _ = state
        .db
        .set_conversation_workspace(&conversation.id, &dir.display().to_string());
    Ok(dir)
}

fn build_prompt(
    state: &AppState,
    focus: &str,
    text: &str,
    attachments: &[String],
    include_preamble: bool,
) -> String {
    let mut prompt = String::new();
    if include_preamble {
        prompt.push_str(focus_preamble(focus));
        prompt.push_str("\n\n");
    }
    if !attachments.is_empty() {
        if let Ok(guard) = PathGuard::home(excluded_paths(state)) {
            prompt.push_str("Attached files for context:\n");
            for att in attachments {
                match read_attachment(&guard, att) {
                    Some(block) => prompt.push_str(&block),
                    None => prompt.push_str(&format!("--- File: {att} (unavailable) ---\n")),
                }
            }
            prompt.push('\n');
        }
    }
    prompt.push_str(text);
    prompt
}

pub async fn send_codex_message(
    app: AppHandle,
    state: &AppState,
    conversation_id: String,
    text: String,
    focus: Option<String>,
    attachments: Vec<String>,
) -> Result<(), String> {
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/liamgk/Desktop/BUDDY/.cursor/debug-4e7020.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"sessionId":"4e7020","id":"log_start","timestamp":{},"location":"codex_orchestrator.rs:send_codex_message","message":"Backend received message","data":{{}},"runId":"run1","hypothesisId":"Backend Error Early Exit"}}"#, buddy_database::chrono_now());
    }
    // #endregion

    let conversation = state
        .db
        .get_conversation(&conversation_id)
        .map_err(|e| e.to_string())?;

    let focus = focus
        .or(conversation.focus_mode.clone())
        .unwrap_or_else(|| "planning".to_string());
    let _ = state.db.set_conversation_focus(&conversation_id, &focus);

    let user_metadata = serde_json::json!({
        "attachments": attachments,
        "focus": focus,
    })
    .to_string();
    state
        .db
        .add_message_with_metadata(&conversation_id, "user", &text, Some(&user_metadata))
        .map_err(|e| e.to_string())?;

    if state
        .db
        .get_messages(&conversation_id)
        .map(|m| m.len())
        .unwrap_or(0)
        == 1
    {
        let title: String = text.chars().take(40).collect();
        let _ = state.db.update_conversation_title(&conversation_id, &title);
    }

    let backend = state
        .db
        .get_setting("code_agent_backend")
        .ok()
        .flatten()
        .unwrap_or_else(|| "cursor".to_string());

    let workspace = resolve_workspace(state, &conversation)?;

    let (assistant_content, backend_label) = if backend == "codex" {
        run_codex_backend(&app, state, &focus, &text, &attachments, &workspace).await
    } else {
        run_cursor_backend(&app, state, &conversation_id, &focus, &text, &attachments, &workspace)
            .await
    };

    let assistant_metadata = serde_json::json!({
        "backend": backend,
        "focus": focus,
        "attachments": attachments.len(),
    })
    .to_string();
    state
        .db
        .add_message_with_metadata(
            &conversation_id,
            "assistant",
            &assistant_content,
            Some(&assistant_metadata),
        )
        .map_err(|e| e.to_string())?;

    let _ = state.db.log_external_action(
        "code_agent_exec",
        &format!(
            "{backend_label} ({focus}) — {} attachment(s) in {}",
            attachments.len(),
            workspace.display()
        ),
        None,
        true,
    );

    let _ = app.emit("codex-done", ());

    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/liamgk/Desktop/BUDDY/.cursor/debug-4e7020.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"sessionId":"4e7020","id":"log_end","timestamp":{},"location":"codex_orchestrator.rs:send_codex_message","message":"Backend finished successfully","data":{{}},"runId":"run1","hypothesisId":"Backend Error Early Exit"}}"#, buddy_database::chrono_now());
    }
    // #endregion

    Ok(())
}

async fn run_codex_backend(
    app: &AppHandle,
    state: &AppState,
    focus: &str,
    text: &str,
    attachments: &[String],
    workspace: &std::path::Path,
) -> (String, String) {
    let prompt = build_prompt(state, focus, text, attachments, true);
    let model = state
        .db
        .get_setting("codex_model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "gpt-5.5".to_string());
    let codex_path = state.db.get_setting("codex_path").ok().flatten();
    let program = codex_runner::codex_program(codex_path.as_deref());
    let api_key = crate::secrets::get_secret("openai_api_key").ok().flatten();

    let result =
        codex_runner::run_codex(app, &program, workspace, &model, api_key.as_deref(), &prompt).await;
    (finalize(app, result.map(|o| (o.text, o.exit_ok))), format!("Codex ({model})"))
}

#[allow(clippy::too_many_arguments)]
async fn run_cursor_backend(
    app: &AppHandle,
    state: &AppState,
    conversation_id: &str,
    focus: &str,
    text: &str,
    attachments: &[String],
    workspace: &std::path::Path,
) -> (String, String) {
    let mode = cursor_runner::focus_to_mode(focus);
    // Native plan/ask modes handle behavior; for agent mode add a light preamble.
    let prompt = build_prompt(state, focus, text, attachments, mode.is_none());

    let model = state
        .db
        .get_setting("code_model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "auto".to_string());
    let cursor_path = state.db.get_setting("cursor_path").ok().flatten();
    let program = cursor_runner::cursor_program(cursor_path.as_deref());
    let api_key = crate::secrets::get_secret("cursor_api_key").ok().flatten();

    let chat_key = format!("cursor_chat:{conversation_id}");
    let mut chat_id = state.db.get_setting(&chat_key).ok().flatten();
    if chat_id.is_none() {
        chat_id = cursor_runner::create_chat(&program, api_key.as_deref(), workspace).await;
        if let Some(id) = &chat_id {
            let _ = state.db.set_setting(&chat_key, id);
        }
    }

    let result = cursor_runner::run_cursor(
        app,
        &program,
        workspace,
        &model,
        mode,
        api_key.as_deref(),
        chat_id.as_deref(),
        &prompt,
    )
    .await;

    let content = match result {
        Ok(output) => {
            // #region agent log
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/liamgk/Desktop/BUDDY/.cursor/debug-4e7020.log") {
                use std::io::Write;
                let _ = writeln!(file, r#"{{"sessionId":"4e7020","id":"log_cursor_result","timestamp":{},"location":"codex_orchestrator.rs:run_cursor_backend","message":"Cursor backend completed","data":{{"chat_id":{:?},"text_len":{}}},"runId":"run1","hypothesisId":"Backend Error Early Exit"}}"#, buddy_database::chrono_now(), output.chat_id, output.text.len());
            }
            // #endregion
            if let Some(id) = output.chat_id {
                let _ = state.db.set_setting(&chat_key, &id);
            }
            finalize_text(app, output.text, output.exit_ok)
        }
        Err(e) => finalize_err(app, e),
    };
    (content, format!("Cursor ({model})"))
}

fn finalize(app: &AppHandle, result: Result<(String, bool), String>) -> String {
    match result {
        Ok((text, exit_ok)) => finalize_text(app, text, exit_ok),
        Err(e) => finalize_err(app, e),
    }
}

fn finalize_text(app: &AppHandle, text: String, _exit_ok: bool) -> String {
    if text.trim().is_empty() {
        let msg = "Agent produced no output.".to_string();
        let _ = app.emit("codex-chunk", &msg);
        msg
    } else {
        text
    }
}

fn finalize_err(app: &AppHandle, e: String) -> String {
    warn!(error = %e, "code agent run failed");
    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/liamgk/Desktop/BUDDY/.cursor/debug-4e7020.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"sessionId":"4e7020","id":"log_finalize_err","timestamp":{},"location":"codex_orchestrator.rs:finalize_err","message":"Backend error","data":{{"error":{:?}}},"runId":"run1","hypothesisId":"Backend Error Early Exit"}}"#, buddy_database::chrono_now(), e);
    }
    // #endregion
    let msg = format!("Code Agent error: {e}");
    let _ = app.emit("codex-error", &msg);
    let _ = app.emit("codex-chunk", &msg);
    msg
}
