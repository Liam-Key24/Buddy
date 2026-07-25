use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
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
    mlx_child: Mutex<Option<Child>>,
    mlx_owned: AtomicBool,
    brain_owned: AtomicBool,
    brain_ensure_lock: tokio::sync::Mutex<()>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            brain_child: Mutex::new(None),
            mlx_child: Mutex::new(None),
            mlx_owned: AtomicBool::new(false),
            brain_owned: AtomicBool::new(false),
            brain_ensure_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn brain_port(state: &AppState) -> u16 {
        Self::port_from_url(&state.brain_url(), 8002)
    }

    fn mlx_port(state: &AppState) -> u16 {
        Self::port_from_url(&state.mlx_url(), 8001)
    }

    fn port_from_url(url: &str, default: u16) -> u16 {
        url.rsplit(':')
            .next()
            .and_then(|p| p.trim_end_matches('/').parse().ok())
            .unwrap_or(default)
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
            info!(%pid, port, signal, "killing process on service port");
            let _ = Command::new("kill").args([signal, &pid]).status();
        }
    }

    fn clear_port(port: u16) {
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
        Self::check_brain_embed_with_timeout(state, 2).await
    }

    pub async fn check_brain_embed_with_timeout(state: &AppState, timeout_secs: u64) -> bool {
        let url = format!("{}/embed", state.brain_url());
        match reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({ "text": "ping" }))
            .timeout(std::time::Duration::from_secs(timeout_secs))
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

    pub fn auto_start_mlx(state: &AppState) -> bool {
        state
            .db
            .get_setting_or("auto_start_mlx", "true")
            .eq_ignore_ascii_case("true")
    }

    fn model_name(state: &AppState) -> String {
        state
            .db
            .get_setting_or("model_name", "mlx-community/Llama-3.2-3B-Instruct-4bit")
    }

    fn venv_bin(state: &AppState, name: &str) -> PathBuf {
        state.project_root.join("brain/venv/bin").join(name)
    }

    pub async fn ensure_brain(&self, state: &AppState) -> Result<(), String> {
        let _guard = self.brain_ensure_lock.lock().await;
        if Self::check_brain_ready(state).await {
            return Ok(());
        }

        // If health is already up, just wait for embed (don't kill a warming process).
        if Self::check_brain(state).await
            && Self::check_brain_embed_with_timeout(state, 60).await
        {
            return Ok(());
        }

        let port = Self::brain_port(state);
        self.stop_brain();
        Self::clear_port(port);
        self.start_brain(state)?;

        // Wait for /health (startup may preload the embedding model).
        for _ in 0..120 {
            if Self::check_brain(state).await {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        if Self::check_brain_embed_with_timeout(state, 60).await {
            return Ok(());
        }

        Err("brain started but /embed endpoint not available".into())
    }

    pub async fn ensure_mlx(&self, state: &AppState) -> Result<(), String> {
        if Self::check_mlx(state).await {
            return Ok(());
        }

        let port = Self::mlx_port(state);
        self.stop_mlx();
        Self::clear_port(port);
        self.start_mlx(state)?;

        // Model load / first download can take a while.
        for i in 0..180 {
            if Self::check_mlx(state).await {
                info!(attempt = i + 1, "mlx is ready");
                return Ok(());
            }
            if i % 10 == 9 {
                info!(attempt = i + 1, "waiting for mlx to become ready…");
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }

        Err("mlx started but /v1/models never became ready (model may still be downloading)".into())
    }

    pub fn start_brain(&self, state: &AppState) -> Result<(), String> {
        let port = Self::brain_port(state);
        let mut guard = self.brain_child.lock().map_err(|e| e.to_string())?;

        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Self::clear_port(port);

        if !Self::pids_on_port(port).is_empty() {
            return Err(format!("brain port {port} still in use after cleanup"));
        }

        let brain_dir = state.project_root.join("brain");
        if !brain_dir.is_dir() {
            return Err(format!(
                "brain directory missing at {}. Set project root or run Buddy from the repo once.",
                brain_dir.display()
            ));
        }

        let venv_python = Self::venv_bin(state, "python");
        let uvicorn = Self::venv_bin(state, "uvicorn");

        let (program, args): (String, Vec<String>) = if uvicorn.exists() {
            (
                uvicorn.to_string_lossy().into_owned(),
                vec![
                    "main:app".into(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
            )
        } else if venv_python.exists() {
            (
                venv_python.to_string_lossy().into_owned(),
                vec![
                    "-m".into(),
                    "uvicorn".into(),
                    "main:app".into(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
            )
        } else {
            (
                "python3".into(),
                vec![
                    "-m".into(),
                    "uvicorn".into(),
                    "main:app".into(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
            )
        };

        info!(program = %program, port, "starting brain process");
        let child = Command::new(&program)
            .args(&args)
            .current_dir(&brain_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start brain: {e}"))?;

        *guard = Some(child);
        self.brain_owned.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn start_mlx(&self, state: &AppState) -> Result<(), String> {
        let port = Self::mlx_port(state);
        let model = Self::model_name(state);
        let mut guard = self.mlx_child.lock().map_err(|e| e.to_string())?;

        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Self::clear_port(port);

        if !Self::pids_on_port(port).is_empty() {
            return Err(format!("mlx port {port} still in use after cleanup"));
        }

        let brain_dir = state.project_root.join("brain");
        if !brain_dir.is_dir() {
            return Err(format!(
                "brain directory missing at {}. Buddy needs the repo (with brain/venv) to start MLX.",
                brain_dir.display()
            ));
        }

        let server_bin = Self::venv_bin(state, "mlx_lm.server");
        let venv_python = Self::venv_bin(state, "python");
        let start_script = brain_dir.join("scripts/start_mlx.sh");

        let (program, args, cwd): (String, Vec<String>, PathBuf) = if server_bin.exists() {
            (
                server_bin.to_string_lossy().into_owned(),
                vec![
                    "--model".into(),
                    model.clone(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
                brain_dir.clone(),
            )
        } else if venv_python.exists() {
            (
                venv_python.to_string_lossy().into_owned(),
                vec![
                    "-m".into(),
                    "mlx_lm.server".into(),
                    "--model".into(),
                    model.clone(),
                    "--host".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                ],
                brain_dir.clone(),
            )
        } else if start_script.exists() {
            (
                "/bin/bash".into(),
                vec![start_script.to_string_lossy().into_owned()],
                brain_dir.clone(),
            )
        } else {
            return Err(format!(
                "MLX server not found under {}. Create brain/venv and pip install -r brain/requirements.txt",
                Self::venv_bin(state, "").display()
            ));
        };

        info!(program = %program, %model, port, "starting mlx process");
        let child = Command::new(&program)
            .args(&args)
            .current_dir(&cwd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start mlx: {e}"))?;

        *guard = Some(child);
        self.mlx_owned.store(true, Ordering::SeqCst);
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
        self.brain_owned.store(false, Ordering::SeqCst);
    }

    pub fn stop_mlx(&self) {
        if let Ok(mut guard) = self.mlx_child.lock() {
            if let Some(mut child) = guard.take() {
                info!("stopping mlx process");
                if let Err(e) = child.kill() {
                    warn!(error = %e, "failed to kill mlx process");
                }
                let _ = child.wait();
            }
        }
        self.mlx_owned.store(false, Ordering::SeqCst);
    }

    pub fn stop_owned_services(&self) {
        if self.brain_owned.load(Ordering::SeqCst) {
            self.stop_brain();
        }
        if self.mlx_owned.load(Ordering::SeqCst) {
            self.stop_mlx();
        }
    }

    pub fn project_root_looks_valid(root: &Path) -> bool {
        root.join("brain").is_dir()
            && (root.join("app").is_dir() || root.join("Cargo.toml").is_file())
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.stop_owned_services();
    }
}
