//! Runs the Codex CLI (`codex exec`) as a subprocess for the Code Agent chat.
//!
//! This is intentionally separate from the MLX/Brain path: the Code Agent talks
//! to Codex + GPT-5.5 directly and streams its stdout back to the UI.

use std::path::Path;
use std::process::Stdio;

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

pub struct CodexOutput {
    pub text: String,
    pub exit_ok: bool,
}

/// Returns the resolved codex binary (from the `codex_path` setting or PATH).
pub fn codex_program(codex_path: Option<&str>) -> String {
    codex_path
        .filter(|p| !p.trim().is_empty())
        .map(|p| p.to_string())
        .unwrap_or_else(|| "codex".to_string())
}

/// Spawn `codex exec` in the given workspace, streaming stdout to the UI via
/// `codex-chunk` events. Returns the accumulated output.
pub async fn run_codex(
    app: &AppHandle,
    program: &str,
    workspace: &Path,
    model: &str,
    api_key: Option<&str>,
    prompt: &str,
) -> Result<CodexOutput, String> {
    let mut cmd = Command::new(program);
    cmd.arg("exec")
        .arg("-m")
        .arg(model)
        .arg("--skip-git-repo-check")
        .arg(prompt)
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(key) = api_key {
        cmd.env("OPENAI_API_KEY", key);
    }

    info!(program, model, workspace = %workspace.display(), "spawning codex");

    let mut child = cmd.spawn().map_err(|e| {
        format!("failed to start codex ({program}): {e}. Install with `npm i -g @openai/codex` or set a codex path in Settings.")
    })?;

    let stdout = child.stdout.take().ok_or("failed to capture codex stdout")?;
    let stderr = child.stderr.take().ok_or("failed to capture codex stderr")?;

    let mut collected = String::new();
    let mut reader = BufReader::new(stdout).lines();
    let mut err_reader = BufReader::new(stderr).lines();

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let piece = format!("{line}\n");
                        collected.push_str(&piece);
                        let _ = app.emit("codex-chunk", &piece);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        warn!(error = %e, "codex stdout read error");
                        break;
                    }
                }
            }
            err = err_reader.next_line() => {
                if let Ok(Some(line)) = err {
                    warn!(codex_stderr = %line);
                }
            }
        }
    }

    // Drain any remaining stderr.
    while let Ok(Some(line)) = err_reader.next_line().await {
        warn!(codex_stderr = %line);
    }

    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/liamgk/Desktop/BUDDY/.cursor/debug-4e7020.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"sessionId":"4e7020","id":"log_wait_start_codex","timestamp":{},"location":"codex_runner.rs:run_codex","message":"Waiting for codex to exit","data":{{}},"runId":"run1","hypothesisId":"Subprocess Hang"}}"#, buddy_database::chrono_now());
    }
    // #endregion

    let status = child.wait().await.map_err(|e| e.to_string())?;

    // #region agent log
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/Users/liamgk/Desktop/BUDDY/.cursor/debug-4e7020.log") {
        use std::io::Write;
        let _ = writeln!(file, r#"{{"sessionId":"4e7020","id":"log_wait_end_codex","timestamp":{},"location":"codex_runner.rs:run_codex","message":"codex exited","data":{{"success":{}}},"runId":"run1","hypothesisId":"Subprocess Hang"}}"#, buddy_database::chrono_now(), status.success());
    }
    // #endregion

    Ok(CodexOutput {
        text: collected,
        exit_ok: status.success(),
    })
}
