//! Runs the Codex CLI (`codex exec`) as a subprocess for the Code Agent chat.
//!
//! This is intentionally separate from the MLX/Brain path: the Code Agent talks
//! to Codex + GPT-5.5 directly and streams its stdout back to the UI.

use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

use crate::CodeEmit;

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
/// [`CodeEmit::chunk`]. Returns the accumulated output.
pub async fn run_codex(
    emit: &dyn CodeEmit,
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
                        emit.chunk(&piece);
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

    let status = child.wait().await.map_err(|e| e.to_string())?;
    Ok(CodexOutput {
        text: collected,
        exit_ok: status.success(),
    })
}
