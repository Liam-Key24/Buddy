//! Runs the Cursor CLI (`cursor-agent`) headless for the Code Agent chat.
//!
//! Separate from the MLX/Brain path. The agent runs autonomously in the chosen
//! workspace with write + shell access, streaming output back to the UI via
//! [`CodeEmit`], including a preview URL when a local dev-server URL is
//! detected.

use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

use crate::CodeEmit;

fn localhost_url() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"https?://(?:localhost|127\.0\.0\.1|0\.0\.0\.0)(?::\d+)?(?:/\S*)?")
            .expect("valid regex")
    })
}

pub struct CursorOutput {
    pub text: String,
    pub exit_ok: bool,
    pub chat_id: Option<String>,
}

/// Resolve the cursor-agent binary (from the `cursor_path` setting or PATH).
pub fn cursor_program(cursor_path: Option<&str>) -> String {
    cursor_path
        .filter(|p| !p.trim().is_empty())
        .map(|p| p.to_string())
        .unwrap_or_else(|| "cursor-agent".to_string())
}

/// Map a Buddy focus mode to a cursor-agent `--mode`. Planning and asking use
/// the native restricted modes; focused/debugging use the default agent mode
/// (returns None, meaning no `--mode` flag).
pub fn focus_to_mode(focus: &str) -> Option<&'static str> {
    match focus {
        "planning" => Some("plan"),
        "asking" => Some("ask"),
        _ => None,
    }
}

/// Create a new cursor-agent chat and return its id, for multi-turn resume.
pub async fn create_chat(
    program: &str,
    api_key: Option<&str>,
    workspace: &Path,
) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.arg("create-chat")
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(key) = api_key {
        cmd.arg("--api-key").arg(key);
    }
    let output = cmd.output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Extract assistant text and any chat/session id from a stream-json line.
fn extract(value: &Value) -> (Option<String>, Option<String>) {
    let chat_id = value
        .get("chatId")
        .or_else(|| value.get("chat_id"))
        .or_else(|| value.get("session_id"))
        .or_else(|| value.get("sessionId"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Assistant message with structured content blocks.
    if kind == "assistant" {
        if let Some(content) = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            let mut text = String::new();
            for block in content {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    text.push_str(t);
                }
            }
            if !text.is_empty() {
                return (Some(text), chat_id);
            }
        }
    }

    // Partial text delta or simple text event.
    if let Some(t) = value.get("text").and_then(|v| v.as_str()) {
        return (Some(t.to_string()), chat_id);
    }
    if let Some(t) = value.get("delta").and_then(|v| v.as_str()) {
        return (Some(t.to_string()), chat_id);
    }
    // Final result payload.
    if kind == "result" {
        if let Some(t) = value.get("result").and_then(|v| v.as_str()) {
            return (Some(t.to_string()), chat_id);
        }
    }

    (None, chat_id)
}

fn maybe_emit_preview(emit: &dyn CodeEmit, text: &str) {
    if let Some(m) = localhost_url().find(text) {
        emit.preview_url(m.as_str());
    }
}

/// Spawn `cursor-agent` headless in the workspace, streaming output to the UI.
#[allow(clippy::too_many_arguments)]
pub async fn run_cursor(
    emit: &dyn CodeEmit,
    program: &str,
    workspace: &Path,
    model: &str,
    mode: Option<&str>,
    api_key: Option<&str>,
    chat_id: Option<&str>,
    prompt: &str,
) -> Result<CursorOutput, String> {
    let mut cmd = Command::new(program);
    cmd.arg("--print")
        .arg("--force")
        .arg("--sandbox")
        .arg("disabled")
        .arg("--trust")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--stream-partial-output")
        .arg("--workspace")
        .arg(workspace)
        .arg("--model")
        .arg(model);

    if let Some(mode) = mode {
        cmd.arg("--mode").arg(mode);
    }
    if let Some(id) = chat_id {
        cmd.arg("--resume").arg(id);
    }
    if let Some(key) = api_key {
        cmd.arg("--api-key").arg(key);
    }

    cmd.arg(prompt)
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    info!(program, model, workspace = %workspace.display(), ?mode, "spawning cursor-agent");

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "failed to start cursor-agent ({program}): {e}. Install with `curl https://cursor.com/install -fsS | bash` then run `cursor-agent login`."
        )
    })?;

    let stdout = child.stdout.take().ok_or("failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("failed to capture stderr")?;

    let mut collected = String::new();
    let mut discovered_chat_id: Option<String> = chat_id.map(String::from);
    let mut reader = BufReader::new(stdout).lines();
    let mut err_reader = BufReader::new(stderr).lines();

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Value>(&line) {
                            Ok(value) => {
                                let (text, chat) = extract(&value);
                                if let Some(chat) = chat {
                                    discovered_chat_id = Some(chat);
                                }
                                if let Some(text) = text {
                                    collected.push_str(&text);
                                    maybe_emit_preview(emit, &text);
                                    emit.chunk(&text);
                                }
                            }
                            Err(_) => {
                                // Not JSON (plain text fallback) — pass through.
                                let piece = format!("{line}\n");
                                collected.push_str(&piece);
                                maybe_emit_preview(emit, &line);
                                emit.chunk(&piece);
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        warn!(error = %e, "cursor-agent stdout read error");
                        break;
                    }
                }
            }
            err = err_reader.next_line() => {
                if let Ok(Some(line)) = err {
                    warn!(cursor_stderr = %line);
                }
            }
        }
    }

    while let Ok(Some(line)) = err_reader.next_line().await {
        warn!(cursor_stderr = %line);
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    Ok(CursorOutput {
        text: collected,
        exit_ok: status.success(),
        chat_id: discovered_chat_id,
    })
}
