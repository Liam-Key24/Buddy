//! Executes a Code Agent turn: resolves workspace, maps focus to backend
//! flags, runs Codex or Cursor, and returns streamed output.
//!
//! This is Core execution — not a second planner. Chat planning comes from
//! the Brain; the Code Agent page calls [`send_codex_message`] with an
//! explicit focus from the UI.

use std::path::PathBuf;

use buddy_core::{excluded_paths_from_setting, PathGuard};
use buddy_database::Database;
use tracing::warn;

use crate::codex_runner;
use crate::cursor_runner;
use crate::{CodeEmit, SecretLookup};

const ATTACHMENT_MAX_LINES: usize = 400;

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

fn excluded_paths(db: &Database) -> Vec<String> {
    excluded_paths_from_setting(db.get_setting("fs_excluded_paths").ok().flatten())
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
    db: &Database,
    conversation: &buddy_database::Conversation,
) -> Result<PathBuf, String> {
    if let Some(existing) = &conversation.workspace_path {
        let path = PathBuf::from(existing);
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        return Ok(path);
    }

    let base = db
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
    let _ = db.set_conversation_workspace(&conversation.id, &dir.display().to_string());
    Ok(dir)
}

fn build_prompt(
    db: &Database,
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
        if let Ok(guard) = PathGuard::home(excluded_paths(db)) {
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

/// Result of running one Code Agent backend turn, without any message
/// persistence — callers decide how (and whether) to store the exchange.
pub struct CodeTurnOutcome {
    pub assistant_content: String,
    pub backend: String,
    pub backend_label: String,
    pub focus: String,
    pub workspace: PathBuf,
}

/// Resolve workspace/backend and run Codex or Cursor. Does not plan intent —
/// callers (Buddy chat via Brain `code` intent, or the Code Agent page) decide
/// that a coding turn should run.
pub async fn run_code_turn(
    emit: &dyn CodeEmit,
    db: &Database,
    secrets: &dyn SecretLookup,
    conversation_id: &str,
    text: &str,
    focus_hint: Option<&str>,
    attachments: &[String],
) -> Result<CodeTurnOutcome, String> {
    let conversation = db.get_conversation(conversation_id).map_err(|e| e.to_string())?;

    let focus = focus_hint
        .map(str::to_string)
        .or(conversation.focus_mode.clone())
        .unwrap_or_else(|| "planning".to_string());
    let _ = db.set_conversation_focus(conversation_id, &focus);

    let workspace = resolve_workspace(db, &conversation)?;
    let backend = db.get_setting_or("code_agent_backend", "cursor");

    let (assistant_content, backend_label) = if backend == "codex" {
        run_codex_backend(emit, db, secrets, &focus, text, attachments, &workspace).await
    } else {
        run_cursor_backend(
            emit,
            db,
            secrets,
            conversation_id,
            &focus,
            text,
            attachments,
            &workspace,
        )
        .await
    };

    let _ = db.log_external_action(
        "code_agent_exec",
        &format!(
            "{backend_label} ({focus}) — {} attachment(s) in {}",
            attachments.len(),
            workspace.display()
        ),
        None,
        true,
    );

    Ok(CodeTurnOutcome {
        assistant_content,
        backend,
        backend_label,
        focus,
        workspace,
    })
}

/// Code Agent page entry: persist user turn, execute backend, persist reply.
#[allow(clippy::too_many_arguments)]
pub async fn send_codex_message(
    emit: &dyn CodeEmit,
    db: &Database,
    secrets: &dyn SecretLookup,
    conversation_id: String,
    text: String,
    focus: Option<String>,
    attachments: Vec<String>,
) -> Result<(), String> {
    let conversation = db.get_conversation(&conversation_id).map_err(|e| e.to_string())?;
    let resolved_focus = focus
        .clone()
        .or(conversation.focus_mode.clone())
        .unwrap_or_else(|| "planning".to_string());

    let user_metadata = serde_json::json!({
        "attachments": attachments,
        "focus": resolved_focus,
    })
    .to_string();
    db.add_message_with_metadata(&conversation_id, "user", &text, Some(&user_metadata))
        .map_err(|e| e.to_string())?;

    if db
        .get_messages(&conversation_id)
        .map(|m| m.len())
        .unwrap_or(0)
        == 1
    {
        let title: String = text.chars().take(40).collect();
        let _ = db.update_conversation_title(&conversation_id, &title);
    }

    let outcome = run_code_turn(
        emit,
        db,
        secrets,
        &conversation_id,
        &text,
        focus.as_deref(),
        &attachments,
    )
    .await?;

    let assistant_metadata = serde_json::json!({
        "backend": outcome.backend,
        "focus": outcome.focus,
        "attachments": attachments.len(),
    })
    .to_string();
    db.add_message_with_metadata(
        &conversation_id,
        "assistant",
        &outcome.assistant_content,
        Some(&assistant_metadata),
    )
    .map_err(|e| e.to_string())?;

    emit.done();

    Ok(())
}

async fn run_codex_backend(
    emit: &dyn CodeEmit,
    db: &Database,
    secrets: &dyn SecretLookup,
    focus: &str,
    text: &str,
    attachments: &[String],
    workspace: &std::path::Path,
) -> (String, String) {
    let prompt = build_prompt(db, focus, text, attachments, true);
    let model = db.get_setting_or("codex_model", "gpt-5.5");
    let codex_path = db.get_setting("codex_path").ok().flatten();
    let program = codex_runner::codex_program(codex_path.as_deref());
    let api_key = secrets.get("openai_api_key");

    let result =
        codex_runner::run_codex(emit, &program, workspace, &model, api_key.as_deref(), &prompt)
            .await;
    (finalize(emit, result.map(|o| (o.text, o.exit_ok))), format!("Codex ({model})"))
}

#[allow(clippy::too_many_arguments)]
async fn run_cursor_backend(
    emit: &dyn CodeEmit,
    db: &Database,
    secrets: &dyn SecretLookup,
    conversation_id: &str,
    focus: &str,
    text: &str,
    attachments: &[String],
    workspace: &std::path::Path,
) -> (String, String) {
    let mode = cursor_runner::focus_to_mode(focus);
    // Native plan/ask modes handle behavior; for agent mode add a light preamble.
    let prompt = build_prompt(db, focus, text, attachments, mode.is_none());

    let model = db.get_setting_or("code_model", "auto");
    let cursor_path = db.get_setting("cursor_path").ok().flatten();
    let program = cursor_runner::cursor_program(cursor_path.as_deref());
    let api_key = secrets.get("cursor_api_key");

    let chat_key = format!("cursor_chat:{conversation_id}");
    let mut chat_id = db.get_setting(&chat_key).ok().flatten();
    if chat_id.is_none() {
        chat_id = cursor_runner::create_chat(&program, api_key.as_deref(), workspace).await;
        if let Some(id) = &chat_id {
            let _ = db.set_setting(&chat_key, id);
        }
    }

    let result = cursor_runner::run_cursor(
        emit,
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
            if let Some(id) = output.chat_id {
                let _ = db.set_setting(&chat_key, &id);
            }
            finalize_text(emit, output.text, output.exit_ok)
        }
        Err(e) => finalize_err(emit, e),
    };
    (content, format!("Cursor ({model})"))
}

fn finalize(emit: &dyn CodeEmit, result: Result<(String, bool), String>) -> String {
    match result {
        Ok((text, exit_ok)) => finalize_text(emit, text, exit_ok),
        Err(e) => finalize_err(emit, e),
    }
}

fn finalize_text(emit: &dyn CodeEmit, text: String, _exit_ok: bool) -> String {
    if text.trim().is_empty() {
        let msg = "Agent produced no output.".to_string();
        emit.chunk(&msg);
        msg
    } else {
        text
    }
}

fn finalize_err(emit: &dyn CodeEmit, e: String) -> String {
    warn!(error = %e, "code agent run failed");
    let msg = format!("Code Agent error: {e}");
    emit.error(&msg);
    msg
}
