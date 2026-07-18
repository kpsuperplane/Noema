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
    sync::{Mutex, watch},
    task::JoinHandle,
    time::{Instant, sleep},
};
use url::Url;

use crate::{GenerationPriority, LocalModelBackend, LocalModelRuntimeStatus};

use super::hardware::detect_ram_gb;
use generation_arbiter::{GenerationArbiter, GenerationPermit};

mod generation_arbiter;

const LOOPBACK_HOST: &str = "127.0.0.1";
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const STDERR_HISTORY_LINES: usize = 32;
const MIB_PER_GIB: u64 = 1024;
const CHECKPOINT_CACHE_RAM_DIVISOR: u64 = 32;
const MAX_CHECKPOINT_CACHE_MIB: u64 = 2 * MIB_PER_GIB;

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
    checkpoint_cache_mib: u64,
    health_client: reqwest::Client,
    state: Mutex<SupervisorState>,
    startup_gate: Mutex<()>,
    generation_arbiter: GenerationArbiter,
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
        let checkpoint_cache_mib = detect_ram_gb().map_or(0, checkpoint_cache_mib);
        let health_client = reqwest::Client::builder()
            .timeout(HEALTH_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| LlamaServerError::InvalidConfig(error.to_string()))?;
        let (status_tx, _) = watch::channel(LocalModelRuntimeStatus::Stopped);
        Ok(Self {
            inner: Arc::new(SupervisorInner {
                config,
                checkpoint_cache_mib,
                health_client,
                state: Mutex::new(SupervisorState::default()),
                startup_gate: Mutex::new(()),
                generation_arbiter: GenerationArbiter::default(),
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
    #[cfg(feature = "local-model-evals")]
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
    pub(crate) async fn acquire_generation(
        &self,
        priority: GenerationPriority,
    ) -> Result<GenerationPermit, LlamaServerError> {
        self.inner
            .generation_arbiter
            .acquire(priority)
            .await
            .map_err(|_| LlamaServerError::GenerationGateClosed)
    }

    /// Ensures one backend candidate is running and healthy.
    ///
    /// # Errors
    ///
    /// Returns [`LlamaServerError::Unavailable`] when all candidates fail, or
    /// [`LlamaServerError::GenerationGateClosed`] after permanent shutdown.
    pub async fn ensure_ready(&self) -> Result<LlamaServerEndpoint, LlamaServerError> {
        let _startup_guard = self.inner.startup_gate.lock().await;
        if self.inner.generation_arbiter.is_closed() {
            return Err(LlamaServerError::GenerationGateClosed);
        }
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

    /// Permanently closes generation and stops the active server process, if any.
    pub async fn shutdown(&self) {
        self.inner.generation_arbiter.close();
        self.stop_runtime().await;
    }

    async fn stop_runtime(&self) {
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
            .args(server_args(
                &self.inner.config,
                candidate.backend,
                port,
                self.inner.checkpoint_cache_mib,
            ))
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

fn server_args(
    config: &LlamaServerConfig,
    backend: LocalModelBackend,
    port: u16,
    checkpoint_cache_mib: u64,
) -> Vec<String> {
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
        checkpoint_cache_mib.to_string(),
        "--n-gpu-layers".to_string(),
        if backend == LocalModelBackend::Cpu {
            "0".to_string()
        } else {
            "999".to_string()
        },
        "--no-webui".to_string(),
    ]
}

fn checkpoint_cache_mib(ram_gb: u64) -> u64 {
    ram_gb
        .saturating_mul(MIB_PER_GIB)
        .checked_div(CHECKPOINT_CACHE_RAM_DIVISOR)
        .unwrap_or_default()
        .min(MAX_CHECKPOINT_CACHE_MIB)
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
    fn launch_args_bind_loopback_limit_parallelism_and_select_offload() {
        let cpu = server_args(
            &config(vec![LlamaServerCandidate::new(
                LocalModelBackend::Cpu,
                "llama-server",
            )]),
            LocalModelBackend::Cpu,
            43123,
            512,
        );

        assert!(cpu.windows(2).any(|pair| pair == ["--host", "127.0.0.1"]));
        assert!(cpu.windows(2).any(|pair| pair == ["--parallel", "1"]));
        assert!(cpu.windows(2).any(|pair| pair == ["--cache-ram", "512"]));
        assert!(cpu.windows(2).any(|pair| pair == ["--n-gpu-layers", "0"]));
        let accelerated = server_args(
            &config(vec![LlamaServerCandidate::new(
                LocalModelBackend::Metal,
                "llama-server",
            )]),
            LocalModelBackend::Metal,
            43123,
            1024,
        );

        assert!(
            accelerated
                .windows(2)
                .any(|pair| pair == ["--n-gpu-layers", "999"])
        );
    }

    #[test]
    fn checkpoint_cache_scales_with_system_memory_and_stays_bounded() {
        assert_eq!(checkpoint_cache_mib(0), 0);
        assert_eq!(checkpoint_cache_mib(16), 512);
        assert_eq!(checkpoint_cache_mib(32), 1024);
        assert_eq!(checkpoint_cache_mib(128), MAX_CHECKPOINT_CACHE_MIB);
    }

    #[tokio::test]
    async fn shutdown_drains_queued_generation_and_permanently_closes_runtime() {
        let supervisor = LlamaServerSupervisor::new(config(vec![LlamaServerCandidate::new(
            LocalModelBackend::Cpu,
            "llama-server",
        )]))
        .expect("supervisor");
        let active = supervisor
            .acquire_generation(GenerationPriority::Foreground)
            .await
            .expect("active permit");
        let queued_supervisor = supervisor.clone();
        let queued = tokio::spawn(async move {
            queued_supervisor
                .acquire_generation(GenerationPriority::Background)
                .await
        });
        tokio::time::timeout(Duration::from_secs(1), async {
            while supervisor
                .inner
                .generation_arbiter
                .queued(GenerationPriority::Background)
                != 1
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("background generation should queue");

        supervisor.shutdown().await;

        let queued_error = tokio::time::timeout(Duration::from_secs(1), queued)
            .await
            .expect("queued generation should drain")
            .expect("queued task")
            .expect_err("queued generation should fail");
        assert!(matches!(
            queued_error,
            LlamaServerError::GenerationGateClosed
        ));
        assert!(matches!(
            supervisor
                .acquire_generation(GenerationPriority::Foreground)
                .await,
            Err(LlamaServerError::GenerationGateClosed)
        ));
        assert!(matches!(
            supervisor.ensure_ready().await,
            Err(LlamaServerError::GenerationGateClosed)
        ));
        drop(active);
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

        let permit = supervisor
            .acquire_generation(GenerationPriority::Background)
            .await
            .expect("failed startup should not permanently close generation");
        drop(permit);
    }
}
