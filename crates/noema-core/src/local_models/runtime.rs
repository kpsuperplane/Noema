//! Supervised lifecycle for the bundled loopback-only `llama-server` runtime.

use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex as StdMutex, PoisonError},
    time::Duration,
};

use reqwest::StatusCode;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex, OwnedSemaphorePermit, Semaphore, watch},
    task::JoinHandle,
    time::{Instant, sleep},
};
use url::Url;

use super::LocalModelBackend;

const LOOPBACK_HOST: &str = "127.0.0.1";
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const STDERR_HISTORY_LINES: usize = 32;

/// One backend-specific `llama-server` launch candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlamaServerCandidate {
    /// Backend represented by this runtime executable and argument set.
    pub backend: LocalModelBackend,
    /// Path to the pinned `llama-server` executable.
    pub executable_path: PathBuf,
    /// Additional backend-specific arguments supplied by the runtime bundle.
    pub extra_args: Vec<String>,
}

impl LlamaServerCandidate {
    /// Creates a backend candidate without additional arguments.
    #[must_use]
    pub fn new(backend: LocalModelBackend, executable_path: impl Into<PathBuf>) -> Self {
        Self {
            backend,
            executable_path: executable_path.into(),
            extra_args: Vec::new(),
        }
    }

    /// Adds backend-specific arguments to the candidate.
    #[must_use]
    pub fn with_extra_args(mut self, extra_args: impl IntoIterator<Item = String>) -> Self {
        self.extra_args.extend(extra_args);
        self
    }
}

/// Configuration for one installed model served by `llama-server`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlamaServerConfig {
    /// Installed Noema model id exposed as the server alias.
    pub model_id: String,
    /// Absolute path to the checksum-verified GGUF blob.
    pub model_path: PathBuf,
    /// Backend candidates in fallback order.
    pub candidates: Vec<LlamaServerCandidate>,
    /// Context window loaded by llama.cpp.
    pub context_window_tokens: u32,
    /// Time allowed for a candidate to load and become healthy.
    pub startup_timeout: Duration,
}

impl LlamaServerConfig {
    fn validate(&self) -> Result<(), LlamaServerError> {
        if self.model_id.trim().is_empty() {
            return Err(LlamaServerError::InvalidConfig(
                "local model id cannot be empty".to_string(),
            ));
        }
        if self.model_path.as_os_str().is_empty() {
            return Err(LlamaServerError::InvalidConfig(
                "local model path cannot be empty".to_string(),
            ));
        }
        if self.candidates.is_empty() {
            return Err(LlamaServerError::InvalidConfig(
                "at least one llama-server backend candidate is required".to_string(),
            ));
        }
        if self
            .candidates
            .iter()
            .any(|candidate| candidate.executable_path.as_os_str().is_empty())
        {
            return Err(LlamaServerError::InvalidConfig(
                "llama-server executable path cannot be empty".to_string(),
            ));
        }
        if self.context_window_tokens == 0 {
            return Err(LlamaServerError::InvalidConfig(
                "local model context window must be greater than zero".to_string(),
            ));
        }
        if self.startup_timeout.is_zero() {
            return Err(LlamaServerError::InvalidConfig(
                "llama-server startup timeout must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

/// Observable state of the local inference runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalModelRuntimeStatus {
    /// No server process is running.
    Stopped,
    /// One backend candidate is loading the installed model.
    Starting {
        /// Candidate currently being started.
        backend: LocalModelBackend,
    },
    /// A backend candidate is healthy and ready for generation.
    Ready {
        /// Active backend.
        backend: LocalModelBackend,
        /// Loopback API base URL.
        endpoint: String,
        /// Installed model id loaded by the process.
        model_id: String,
    },
    /// A candidate failed and the supervisor is trying the next backend.
    Retrying {
        /// Candidate that failed.
        failed_backend: LocalModelBackend,
        /// Next candidate to try.
        next_backend: LocalModelBackend,
        /// Concise launch or health failure.
        message: String,
    },
    /// Every configured backend candidate failed.
    Failed {
        /// Combined candidate failures.
        message: String,
    },
}

/// Healthy endpoint returned by the supervisor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlamaServerEndpoint {
    /// Loopback-only API base URL.
    pub base_url: Url,
    /// Backend that successfully loaded the model.
    pub backend: LocalModelBackend,
    /// Installed model id loaded by this endpoint.
    pub model_id: String,
}

/// Errors starting or supervising the local inference runtime.
#[derive(Debug, Error)]
pub enum LlamaServerError {
    /// The runtime configuration is incomplete or invalid.
    #[error("invalid llama-server configuration: {0}")]
    InvalidConfig(String),
    /// A loopback port could not be reserved.
    #[error("failed to reserve a loopback port: {0}")]
    PortReservation(#[source] std::io::Error),
    /// Every backend launch candidate failed.
    #[error("llama-server is unavailable: {0}")]
    Unavailable(String),
    /// The generation gate closed unexpectedly.
    #[error("local generation gate is closed")]
    GenerationGateClosed,
}

/// A supervised, loopback-only llama.cpp server for one installed model.
#[derive(Clone, Debug)]
pub struct LlamaServerSupervisor {
    inner: Arc<SupervisorInner>,
}

#[derive(Debug)]
struct SupervisorInner {
    config: LlamaServerConfig,
    health_client: reqwest::Client,
    state: Mutex<SupervisorState>,
    startup_gate: Mutex<()>,
    generation_gate: Arc<Semaphore>,
    status_tx: watch::Sender<LocalModelRuntimeStatus>,
}

#[derive(Debug, Default)]
struct SupervisorState {
    running: Option<RunningServer>,
}

#[derive(Debug)]
struct RunningServer {
    child: Child,
    endpoint: LlamaServerEndpoint,
    stderr_lines: Arc<StdMutex<VecDeque<String>>>,
    stderr_task: JoinHandle<()>,
}

impl RunningServer {
    fn stderr_summary(&self) -> String {
        let lines = self
            .stderr_lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if lines.is_empty() {
            "llama-server exited without diagnostics".to_string()
        } else {
            lines.iter().cloned().collect::<Vec<_>>().join(" | ")
        }
    }

    async fn terminate(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        self.stderr_task.abort();
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stderr_task.abort();
    }
}

impl LlamaServerSupervisor {
    /// Creates a supervisor without starting a process.
    ///
    /// # Errors
    ///
    /// Returns [`LlamaServerError`] when configuration or the health client is invalid.
    pub fn new(config: LlamaServerConfig) -> Result<Self, LlamaServerError> {
        config.validate()?;
        let health_client = reqwest::Client::builder()
            .timeout(HEALTH_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| LlamaServerError::InvalidConfig(error.to_string()))?;
        let (status_tx, _) = watch::channel(LocalModelRuntimeStatus::Stopped);
        Ok(Self {
            inner: Arc::new(SupervisorInner {
                config,
                health_client,
                state: Mutex::new(SupervisorState::default()),
                startup_gate: Mutex::new(()),
                generation_gate: Arc::new(Semaphore::new(1)),
                status_tx,
            }),
        })
    }

    /// Subscribes to runtime state changes for settings and retry UI.
    #[must_use]
    pub fn subscribe_status(&self) -> watch::Receiver<LocalModelRuntimeStatus> {
        self.inner.status_tx.subscribe()
    }

    /// Returns the latest runtime state without waiting for a change.
    #[must_use]
    pub fn status(&self) -> LocalModelRuntimeStatus {
        self.inner.status_tx.borrow().clone()
    }

    /// Returns the active llama-server process id when the runtime is ready.
    #[must_use]
    pub fn process_id(&self) -> Option<u32> {
        let state = self.inner.state.try_lock().ok()?;
        state.running.as_ref()?.child.id()
    }

    /// Acquires the process-wide single-generation permit.
    ///
    /// # Errors
    ///
    /// Returns [`LlamaServerError::GenerationGateClosed`] only when the
    /// supervisor is being torn down.
    pub async fn acquire_generation(&self) -> Result<OwnedSemaphorePermit, LlamaServerError> {
        Arc::clone(&self.inner.generation_gate)
            .acquire_owned()
            .await
            .map_err(|_| LlamaServerError::GenerationGateClosed)
    }

    /// Ensures one backend candidate is running and healthy.
    ///
    /// # Errors
    ///
    /// Returns [`LlamaServerError::Unavailable`] when all candidates fail.
    pub async fn ensure_ready(&self) -> Result<LlamaServerEndpoint, LlamaServerError> {
        let _startup_guard = self.inner.startup_gate.lock().await;
        if let Some(endpoint) = self.ready_endpoint() {
            return Ok(endpoint);
        }

        let mut failures = Vec::with_capacity(self.inner.config.candidates.len());
        for (index, candidate) in self.inner.config.candidates.iter().enumerate() {
            self.set_status(LocalModelRuntimeStatus::Starting {
                backend: candidate.backend,
            });
            match self.start_candidate(candidate).await {
                Ok(running) => {
                    let endpoint = running.endpoint.clone();
                    self.inner.state.lock().await.running = Some(running);
                    self.set_status(LocalModelRuntimeStatus::Ready {
                        backend: endpoint.backend,
                        endpoint: endpoint.base_url.to_string(),
                        model_id: endpoint.model_id.clone(),
                    });
                    return Ok(endpoint);
                }
                Err(message) => {
                    failures.push(format!("{}: {message}", candidate.backend.display_name()));
                    if let Some(next) = self.inner.config.candidates.get(index + 1) {
                        self.set_status(LocalModelRuntimeStatus::Retrying {
                            failed_backend: candidate.backend,
                            next_backend: next.backend,
                            message,
                        });
                    }
                }
            }
        }

        let message = failures.join("; ");
        self.set_status(LocalModelRuntimeStatus::Failed {
            message: message.clone(),
        });
        Err(LlamaServerError::Unavailable(message))
    }

    /// Stops the current process and retries candidates from the beginning.
    ///
    /// # Errors
    ///
    /// Returns [`LlamaServerError::Unavailable`] when all retry candidates fail.
    pub async fn retry(&self) -> Result<LlamaServerEndpoint, LlamaServerError> {
        self.shutdown().await;
        self.ensure_ready().await
    }

    /// Stops the active server process, if any.
    pub async fn shutdown(&self) {
        let _startup_guard = self.inner.startup_gate.lock().await;
        let running = self.inner.state.lock().await.running.take();
        if let Some(running) = running {
            running.terminate().await;
        }
        self.set_status(LocalModelRuntimeStatus::Stopped);
    }

    fn ready_endpoint(&self) -> Option<LlamaServerEndpoint> {
        let Ok(mut state) = self.inner.state.try_lock() else {
            return None;
        };
        let running = state.running.as_mut()?;
        match running.child.try_wait() {
            Ok(None) => Some(running.endpoint.clone()),
            Ok(Some(_)) | Err(_) => {
                let message = running.stderr_summary();
                state.running.take();
                self.set_status(LocalModelRuntimeStatus::Failed { message });
                None
            }
        }
    }

    async fn start_candidate(
        &self,
        candidate: &LlamaServerCandidate,
    ) -> Result<RunningServer, String> {
        let port = reserve_loopback_port()
            .await
            .map_err(|error| error.to_string())?;
        let base_url = Url::parse(&format!("http://{LOOPBACK_HOST}:{port}/"))
            .map_err(|error| error.to_string())?;
        let mut command = Command::new(&candidate.executable_path);
        command
            .args(server_args(&self.inner.config, candidate.backend, port))
            .args(&candidate.extra_args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to launch {}: {error}",
                candidate.executable_path.display()
            )
        })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "llama-server stderr pipe was not available".to_string())?;
        let stderr_lines = Arc::new(StdMutex::new(VecDeque::new()));
        let stderr_task = capture_stderr(stderr, Arc::clone(&stderr_lines));
        let mut running = RunningServer {
            child,
            endpoint: LlamaServerEndpoint {
                base_url,
                backend: candidate.backend,
                model_id: self.inner.config.model_id.clone(),
            },
            stderr_lines,
            stderr_task,
        };

        if let Err(message) = self.wait_until_healthy(&mut running).await {
            let diagnostics = running.stderr_summary();
            running.terminate().await;
            return Err(
                if diagnostics == "llama-server exited without diagnostics" {
                    message
                } else {
                    format!("{message} ({diagnostics})")
                },
            );
        }
        Ok(running)
    }

    async fn wait_until_healthy(&self, running: &mut RunningServer) -> Result<(), String> {
        let deadline = Instant::now() + self.inner.config.startup_timeout;
        let health_url = running
            .endpoint
            .base_url
            .join("health")
            .map_err(|error| error.to_string())?;
        loop {
            match running.child.try_wait() {
                Ok(Some(status)) => {
                    return Err(format!("llama-server exited during startup with {status}"));
                }
                Err(error) => return Err(format!("failed to inspect llama-server: {error}")),
                Ok(None) => {}
            }

            match self
                .inner
                .health_client
                .get(health_url.clone())
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => return Ok(()),
                Ok(response) if response.status() == StatusCode::SERVICE_UNAVAILABLE => {}
                Ok(_) | Err(_) => {}
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "llama-server did not become healthy within {} seconds",
                    self.inner.config.startup_timeout.as_secs()
                ));
            }
            sleep(HEALTH_POLL_INTERVAL).await;
        }
    }

    fn set_status(&self, status: LocalModelRuntimeStatus) {
        let _previous = self.inner.status_tx.send_replace(status);
    }
}

async fn reserve_loopback_port() -> Result<u16, LlamaServerError> {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .map_err(LlamaServerError::PortReservation)?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(LlamaServerError::PortReservation)
}

fn server_args(config: &LlamaServerConfig, backend: LocalModelBackend, port: u16) -> Vec<String> {
    vec![
        "--model".to_string(),
        config.model_path.display().to_string(),
        "--alias".to_string(),
        config.model_id.clone(),
        "--host".to_string(),
        LOOPBACK_HOST.to_string(),
        "--port".to_string(),
        port.to_string(),
        "--ctx-size".to_string(),
        config.context_window_tokens.to_string(),
        "--parallel".to_string(),
        "1".to_string(),
        "--cache-ram".to_string(),
        "0".to_string(),
        "--n-gpu-layers".to_string(),
        if backend == LocalModelBackend::Cpu {
            "0".to_string()
        } else {
            "999".to_string()
        },
        "--no-webui".to_string(),
    ]
}

fn capture_stderr(
    stderr: tokio::process::ChildStderr,
    lines: Arc<StdMutex<VecDeque<String>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let mut history = lines.lock().unwrap_or_else(PoisonError::into_inner);
            if history.len() == STDERR_HISTORY_LINES {
                history.pop_front();
            }
            history.push_back(line);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(candidates: Vec<LlamaServerCandidate>) -> LlamaServerConfig {
        LlamaServerConfig {
            model_id: "test-model".to_string(),
            model_path: PathBuf::from("/models/test.gguf"),
            candidates,
            context_window_tokens: 8_192,
            startup_timeout: Duration::from_millis(10),
        }
    }

    #[test]
    fn launch_args_always_bind_loopback_and_limit_parallelism() {
        let args = server_args(
            &config(vec![LlamaServerCandidate::new(
                LocalModelBackend::Cpu,
                "llama-server",
            )]),
            LocalModelBackend::Cpu,
            43123,
        );

        assert!(args.windows(2).any(|pair| pair == ["--host", "127.0.0.1"]));
        assert!(args.windows(2).any(|pair| pair == ["--parallel", "1"]));
        assert!(args.windows(2).any(|pair| pair == ["--cache-ram", "0"]));
        assert!(args.windows(2).any(|pair| pair == ["--n-gpu-layers", "0"]));
    }

    #[test]
    fn accelerated_candidate_offloads_layers() {
        let args = server_args(
            &config(vec![LlamaServerCandidate::new(
                LocalModelBackend::Metal,
                "llama-server",
            )]),
            LocalModelBackend::Metal,
            43123,
        );

        assert!(
            args.windows(2)
                .any(|pair| pair == ["--n-gpu-layers", "999"])
        );
    }

    #[test]
    fn supervisor_rejects_missing_candidates() {
        let error = LlamaServerSupervisor::new(config(Vec::new())).expect_err("invalid config");

        assert!(matches!(error, LlamaServerError::InvalidConfig(_)));
    }

    #[tokio::test]
    async fn generation_gate_allows_only_one_active_generation() {
        let supervisor = LlamaServerSupervisor::new(config(vec![LlamaServerCandidate::new(
            LocalModelBackend::Cpu,
            "llama-server",
        )]))
        .expect("supervisor");
        let permit = supervisor.acquire_generation().await.expect("first permit");

        assert!(supervisor.inner.generation_gate.try_acquire().is_err());
        drop(permit);
        assert!(supervisor.inner.generation_gate.try_acquire().is_ok());
    }

    #[tokio::test]
    async fn failed_launch_exposes_failed_runtime_status() {
        let supervisor = LlamaServerSupervisor::new(config(vec![LlamaServerCandidate::new(
            LocalModelBackend::Cpu,
            "/definitely/missing/llama-server",
        )]))
        .expect("supervisor");

        let error = supervisor.ensure_ready().await.expect_err("launch failure");

        assert!(matches!(error, LlamaServerError::Unavailable(_)));
        assert!(matches!(
            supervisor.status(),
            LocalModelRuntimeStatus::Failed { .. }
        ));
    }
}
