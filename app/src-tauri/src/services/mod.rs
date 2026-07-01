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

    pub async fn get_status(state: &AppState) -> ServiceStatus {
        let mlx = Self::check_mlx(state).await;
        let brain = Self::check_brain(state).await;
        ServiceStatus { mlx, brain }
    }

    pub fn start_brain(&self, state: &AppState) -> Result<(), String> {
        let mut guard = self.brain_child.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Ok(());
        }

        let venv_python = state
            .project_root
            .join("brain/venv/bin/python");
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
            ("python3", vec!["-m", "uvicorn", "main:app", "--host", "127.0.0.1", "--port", "8002"])
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
            }
        }
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.stop_brain();
    }
}
