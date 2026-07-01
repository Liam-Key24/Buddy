use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use serde::Serialize;
use tracing::{info, warn};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub mlx: bool,
    pub brain: bool,
}

pub struct ProcessManager {
    brain_child: Mutex<Option<Child>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            brain_child: Mutex::new(None),
        }
    }

    fn brain_port(state: &AppState) -> u16 {
        state
            .brain_url()
            .rsplit(':')
            .next()
            .and_then(|p| p.trim_end_matches('/').parse().ok())
            .unwrap_or(8002)
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
            info!(%pid, port, signal, "killing process on brain port");
            let _ = Command::new("kill").args([signal, &pid]).status();
        }
    }

    fn clear_brain_port(port: u16) {
        Self::kill_processes_on_port(port, "-TERM");
        std::thread::sleep(std::time::Duration::from_millis(400));
        if !Self::pids_on_port(port).is_empty() {
            Self::kill_processes_on_port(port, "-KILL");
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    pub async fn check_mlx(state: &AppState) -> bool {
        let url = format!("{}/v1/models", state.mlx_url());
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

    pub async fn check_brain(state: &AppState) -> bool {
        let url = format!("{}/health", state.brain_url());
        match reqwest::Client::new()
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                tracing::debug!(error = %e, "brain health check failed");
                false
            }
        }
    }

    pub async fn check_brain_embed(state: &AppState) -> bool {
        let url = format!("{}/embed", state.brain_url());
        match reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({ "text": "ping" }))
            .timeout(std::time::Duration::from_secs(2))
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
        Self::check_brain(state).await && Self::check_brain_embed(state).await
    }

    pub async fn get_status(state: &AppState) -> ServiceStatus {
        let mlx = Self::check_mlx(state).await;
        let brain = Self::check_brain_ready(state).await;
        ServiceStatus { mlx, brain }
    }

    pub async fn ensure_brain(&self, state: &AppState) -> Result<(), String> {
        if Self::check_brain_ready(state).await {
            return Ok(());
        }

        let port = Self::brain_port(state);
        self.stop_brain();
        Self::clear_brain_port(port);
        self.start_brain(state)?;

        for _ in 0..10 {
            if Self::check_brain_embed(state).await {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        Err("brain started but /embed endpoint not available".into())
    }

    pub fn start_brain(&self, state: &AppState) -> Result<(), String> {
        let port = Self::brain_port(state);
        let mut guard = self.brain_child.lock().map_err(|e| e.to_string())?;

        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Self::clear_brain_port(port);

        if !Self::pids_on_port(port).is_empty() {
            return Err(format!("brain port {port} still in use after cleanup"));
        }

        let venv_python = state.project_root.join("brain/venv/bin/python");
        let uvicorn = state.project_root.join("brain/venv/bin/uvicorn");

        let (program, args): (&str, Vec<&str>) = if uvicorn.exists() {
            (
                uvicorn.to_str().unwrap(),
                vec!["main:app", "--host", "127.0.0.1", "--port", "8002"],
            )
        } else if venv_python.exists() {
            (
                venv_python.to_str().unwrap(),
                vec![
                    "-m",
                    "uvicorn",
                    "main:app",
                    "--host",
                    "127.0.0.1",
                    "--port",
                    "8002",
                ],
            )
        } else {
            (
                "python3",
                vec![
                    "-m",
                    "uvicorn",
                    "main:app",
                    "--host",
                    "127.0.0.1",
                    "--port",
                    "8002",
                ],
            )
        };

        info!(program, "starting brain process");
        let child = Command::new(program)
            .args(&args)
            .current_dir(state.project_root.join("brain"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start brain: {e}"))?;

        *guard = Some(child);
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
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.stop_brain();
    }
}
