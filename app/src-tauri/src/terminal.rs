//! Interactive terminal sessions backed by a PTY.
//!
//! Each session spawns the user's shell in a workspace directory. Output is
//! streamed to the UI via `terminal-output` events; the frontend writes input
//! through the `terminal_write` command. This lets the user run their own
//! commands alongside the Code Agent in the same project folder.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;

use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};
use uuid::Uuid;

struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
}

#[derive(Default)]
pub struct TerminalManager {
    sessions: Mutex<HashMap<String, TerminalSession>>,
}

#[derive(Clone, Serialize)]
struct TerminalOutput {
    id: String,
    data: String,
}

#[derive(Clone, Serialize)]
struct TerminalExit {
    id: String,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(
        &self,
        app: AppHandle,
        cwd: &str,
        cols: u16,
        rows: u16,
    ) -> Result<String, String> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = CommandBuilder::new(shell);
        if !cwd.is_empty() && std::path::Path::new(cwd).is_dir() {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");

        let mut child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        drop(pair.slave);

        let id = Uuid::new_v4().to_string();
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        {
            let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;
            sessions.insert(
                id.clone(),
                TerminalSession {
                    master: pair.master,
                    writer,
                },
            );
        }

        // Reader thread: stream PTY output to the UI.
        let app_reader = app.clone();
        let reader_id = id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = app_reader.emit(
                            "terminal-output",
                            TerminalOutput {
                                id: reader_id.clone(),
                                data,
                            },
                        );
                    }
                    Err(_) => break,
                }
            }
            let _ = app_reader.emit("terminal-exit", TerminalExit { id: reader_id });
        });

        // Wait thread: keep the child handle until exit so the process reaps.
        std::thread::spawn(move || {
            let _ = child.wait();
        });

        info!(%id, cwd, "opened terminal session");
        Ok(id)
    }

    pub fn write(&self, id: &str, data: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get_mut(id)
            .ok_or_else(|| format!("terminal {id} not found"))?;
        session
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| e.to_string())?;
        session.writer.flush().map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get(id)
            .ok_or_else(|| format!("terminal {id} not found"))?;
        session
            .master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    pub fn close(&self, id: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if sessions.remove(id).is_some() {
                info!(%id, "closed terminal session");
            } else {
                warn!(%id, "close requested for unknown terminal");
            }
        }
    }
}
