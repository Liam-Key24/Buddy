#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{Manager, RunEvent};

const BACKEND_HOST: &str = "127.0.0.1";
const BACKEND_PORT: u16 = 8787;

struct BackendState(Mutex<Option<Child>>);

fn application_support_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = std::path::PathBuf::from(home)
        .join("Library/Application Support/com.liamgk.buddy");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn read_groq_key_from_keychain() -> Option<String> {
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-a",
            "buddy",
            "-s",
            "com.liamgk.buddy.groq",
            "-w",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

fn wait_for_backend(timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(mut stream) = TcpStream::connect((BACKEND_HOST, BACKEND_PORT)) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
            let req = format!(
                "GET /health HTTP/1.1\r\nHost: {BACKEND_HOST}\r\nConnection: close\r\n\r\n"
            );
            if stream.write_all(req.as_bytes()).is_ok() {
                let mut buf = [0u8; 256];
                if let Ok(n) = stream.read(&mut buf) {
                    let body = String::from_utf8_lossy(&buf[..n]);
                    if body.contains("200") || body.contains("\"ok\"") {
                        return true;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}

fn spawn_backend() -> Result<Child, String> {
    let support = application_support_dir();
    let db_path = support.join("buddy.db");
    let log_path = support.join("backend.log");

    let mut cmd = if cfg!(debug_assertions) {
        let repo = env!("CARGO_MANIFEST_DIR").trim_end_matches("/src-tauri");
        let mut c = Command::new("bash");
        c.arg("-lc").arg(format!(
            "cd \"{repo}/backend\" && source .venv/bin/activate && \
             BUDDY_DB_PATH=\"{db}\" BUDDY_HOST={host} BUDDY_PORT={port} \
             PYTHONPATH=. uvicorn app.main:app --host {host} --port {port}",
            db = db_path.display(),
            host = BACKEND_HOST,
            port = BACKEND_PORT,
        ));
        c
    } else {
        // Packaged: expect buddy-backend sidecar beside the app resources / PATH override.
        let bin = std::env::var("BUDDY_BACKEND_BIN").unwrap_or_else(|_| {
            // Tauri externalBin name resolves next to the executable as buddy-backend-<triple>
            let exe = std::env::current_exe().ok();
            if let Some(exe) = exe {
                if let Some(dir) = exe.parent() {
                    let candidate = dir.join("buddy-backend");
                    if candidate.exists() {
                        return candidate.to_string_lossy().to_string();
                    }
                }
            }
            "buddy-backend".into()
        });
        let mut c = Command::new(bin);
        c.env("BUDDY_DB_PATH", &db_path)
            .env("BUDDY_HOST", BACKEND_HOST)
            .env("BUDDY_PORT", BACKEND_PORT.to_string());
        c
    };

    if let Some(key) = read_groq_key_from_keychain() {
        cmd.env("GROQ_API_KEY", key);
    }
    cmd.env("BUDDY_AI_ENABLED", "1");

    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));

    cmd.spawn()
        .map_err(|e| format!("Failed to start Buddy backend: {e}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(BackendState(Mutex::new(None)))
        .setup(|app| {
            let child = spawn_backend()?;
            *app.state::<BackendState>().0.lock().unwrap() = Some(child);
            if !wait_for_backend(Duration::from_secs(20)) {
                eprintln!("Buddy backend did not become ready on {BACKEND_HOST}:{BACKEND_PORT}");
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Buddy")
        .run(|app_handle, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<BackendState>() {
                    if let Ok(mut guard) = state.0.lock() {
                        if let Some(mut child) = guard.take() {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }
                }
            }
        });
}
