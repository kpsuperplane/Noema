//! Bounded short-lived OAuth state for hosted MCP servers.

use std::{
    collections::HashMap,
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    CompleteMcpOAuthSetupCommand, CreateMcpServerCommand, McpOAuthSetupAttemptStatus,
    McpOAuthSetupAttemptView, McpOAuthSetupFailure, McpOAuthStoredCredentials,
    McpServerSetupResult, McpSetupTransportConfig, identity::random_hex_id,
};

use self::protocol::RmcpBackend;
use self::validation::{callback_uri, validate_https_or_loopback, validate_loopback_redirect};

const REDACTED: &str = "[REDACTED]";

/// Runtime limits for browser OAuth attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOAuthRegistryConfig {
    /// Lifetime of an attempt, including terminal state.
    pub attempt_ttl: Duration,
    /// Maximum lifetime of a cancellation-abandoned start or completion phase.
    pub in_flight_ttl: Duration,
    /// Hard maximum retained attempt count.
    pub capacity: usize,
    /// Authorization startup deadline.
    pub start_timeout: Duration,
    /// Callback exchange deadline.
    pub callback_timeout: Duration,
}

impl Default for McpOAuthRegistryConfig {
    fn default() -> Self {
        Self {
            attempt_ttl: Duration::from_secs(600),
            in_flight_ttl: Duration::from_secs(120),
            capacity: 64,
            start_timeout: Duration::from_secs(30),
            callback_timeout: Duration::from_secs(30),
        }
    }
}

/// Repository-independent work retained across browser authorization.
#[derive(Clone, PartialEq)]
pub(crate) enum McpOAuthAttemptContext {
    /// A server not persisted until authorization and discovery succeed.
    PendingCreate(Box<CreateMcpServerCommand>),
    /// Existing connection being reauthenticated.
    Reauthenticate {
        /// Durable server id.
        mcp_server_id: String,
        /// Connection generation observed before authorization.
        expected_authority_generation: String,
        /// Current Streamable HTTP endpoint.
        mcp_url: String,
    },
}

impl fmt::Debug for McpOAuthAttemptContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingCreate(_) => formatter.write_str("PendingCreate([REDACTED])"),
            Self::Reauthenticate { .. } => formatter.write_str("Reauthenticate([REDACTED])"),
        }
    }
}

impl McpOAuthAttemptContext {
    fn mcp_url(&self) -> McpOAuthResult<&str> {
        match self {
            Self::PendingCreate(command) => match &command.transport {
                McpSetupTransportConfig::StreamableHttp(config) => Ok(&config.url),
                McpSetupTransportConfig::Stdio(_) => Err(McpOAuthError::new(
                    McpOAuthErrorKind::InvalidInput,
                    "browser OAuth requires Streamable HTTP",
                )),
            },
            Self::Reauthenticate { mcp_url, .. } => Ok(mcp_url),
        }
    }
}

/// Internal service request to start browser OAuth.
#[derive(Clone, PartialEq)]
pub(crate) struct McpOAuthStartRequest {
    /// Work resumed after callback completion.
    pub context: McpOAuthAttemptContext,
    /// Listener-owned callback base URL.
    pub redirect_uri: String,
}

impl fmt::Debug for McpOAuthStartRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthStartRequest")
            .field("context", &self.context)
            .field("redirect_uri", &REDACTED)
            .finish()
    }
}

/// Callback output handed to the service for persistence and discovery.
#[derive(Clone, PartialEq)]
pub(crate) struct McpOAuthCompletion {
    attempt_id: String,
    sequence: u64,
    /// Original create or reauthentication context.
    pub context: McpOAuthAttemptContext,
    /// Credentials to merge into private secret material.
    pub credentials: McpOAuthStoredCredentials,
}

impl fmt::Debug for McpOAuthCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthCompletion")
            .field("attempt_id", &REDACTED)
            .field("context", &self.context)
            .field("credentials", &self.credentials)
            .finish()
    }
}

/// Fixed OAuth failure category used by service error mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpOAuthErrorKind {
    /// Invalid input or registry configuration.
    InvalidInput,
    /// Unknown attempt.
    NotFound,
    /// Registry capacity could not be maintained.
    Capacity,
    /// Attempt transition conflicts with current state.
    Conflict,
    /// Authorization was rejected or requires user action.
    Authentication,
    /// Operation exceeded its deadline.
    Timeout,
    /// Service shutdown cancelled the operation.
    Cancelled,
    /// Metadata, credentials, randomness, or internal state was unavailable.
    Unavailable,
}

/// Fixed outward error carrying logger-only diagnostic detail.
#[derive(Clone)]
pub struct McpOAuthError {
    kind: McpOAuthErrorKind,
    diagnostic_detail: String,
}

impl McpOAuthError {
    fn new(kind: McpOAuthErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            diagnostic_detail: detail.into(),
        }
    }

    /// Return the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> McpOAuthErrorKind {
        self.kind
    }

    /// Return raw detail intended only for the developer error log.
    #[must_use]
    pub fn diagnostic_detail(&self) -> &str {
        &self.diagnostic_detail
    }
}

impl fmt::Debug for McpOAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthError")
            .field("kind", &self.kind)
            .field("diagnostic_detail", &REDACTED)
            .finish()
    }
}

impl fmt::Display for McpOAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            McpOAuthErrorKind::InvalidInput => "invalid MCP OAuth input",
            McpOAuthErrorKind::NotFound => "MCP OAuth attempt was not found or expired",
            McpOAuthErrorKind::Capacity | McpOAuthErrorKind::Unavailable => {
                "MCP OAuth is unavailable"
            }
            McpOAuthErrorKind::Conflict => "MCP OAuth attempt conflicts with its current state",
            McpOAuthErrorKind::Authentication => "MCP OAuth authorization failed",
            McpOAuthErrorKind::Timeout => "MCP OAuth operation timed out",
            McpOAuthErrorKind::Cancelled => "MCP OAuth operation was cancelled",
        })
    }
}

impl std::error::Error for McpOAuthError {}

/// OAuth operation result.
pub type McpOAuthResult<T> = Result<T, McpOAuthError>;
type OAuthFuture<'a, T> = Pin<Box<dyn Future<Output = McpOAuthResult<T>> + Send + 'a>>;

pub(crate) trait McpOAuthRuntime: Send {
    fn complete<'a>(
        &'a mut self,
        callback_url: &'a str,
    ) -> OAuthFuture<'a, McpOAuthStoredCredentials>;
}

pub(crate) struct McpOAuthStarted {
    authorization_url: String,
    runtime: Box<dyn McpOAuthRuntime>,
}

pub(crate) trait McpOAuthBackend: Send + Sync {
    fn start<'a>(&'a self, url: &'a str, redirect: &'a str) -> OAuthFuture<'a, McpOAuthStarted>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Starting,
    Waiting,
    Completing,
    Completed,
    Failed,
}

impl Phase {
    const fn protected(self) -> bool {
        matches!(self, Self::Starting | Self::Completing)
    }
}

struct Entry {
    sequence: u64,
    expires_at: Instant,
    view: McpOAuthSetupAttemptView,
    context: McpOAuthAttemptContext,
    runtime: Option<Box<dyn McpOAuthRuntime>>,
    phase: Phase,
    protected_until: Option<Instant>,
}

#[derive(Default)]
struct State {
    attempts: HashMap<String, Entry>,
    next_sequence: u64,
}

/// Atomic, bounded registry for short-lived OAuth attempts.
#[derive(Clone)]
pub struct McpOAuthRegistry {
    config: McpOAuthRegistryConfig,
    backend: Arc<dyn McpOAuthBackend>,
    state: Arc<Mutex<State>>,
    shutdown: CancellationToken,
}

impl fmt::Debug for McpOAuthRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthRegistry")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl McpOAuthRegistry {
    /// Construct the rmcp-backed registry.
    ///
    /// # Errors
    ///
    /// Returns an error for zero bounds.
    pub fn new(
        config: McpOAuthRegistryConfig,
        shutdown: CancellationToken,
    ) -> McpOAuthResult<Self> {
        validate_config(&config)?;
        Ok(Self::with_backend(config, shutdown, Arc::new(RmcpBackend)))
    }

    pub(crate) fn with_backend(
        config: McpOAuthRegistryConfig,
        shutdown: CancellationToken,
        backend: Arc<dyn McpOAuthBackend>,
    ) -> Self {
        Self {
            config,
            backend,
            state: Arc::new(Mutex::new(State::default())),
            shutdown,
        }
    }

    /// Start a browser authorization attempt.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, exhausted registry capacity, backend
    /// startup failure, timeout, cancellation, or a conflicting state transition.
    pub(crate) async fn start_attempt(
        &self,
        request: McpOAuthStartRequest,
    ) -> McpOAuthResult<McpOAuthSetupAttemptView> {
        let mcp_url = request.context.mcp_url()?.to_string();
        validate_https_or_loopback(&mcp_url, "MCP endpoint")?;
        let redirect = validate_loopback_redirect(&request.redirect_uri)?;
        let (attempt_id, sequence) = self.reserve(request.context).await?;
        let callback = callback_uri(redirect, &attempt_id);
        let started = self
            .bounded(
                self.config.start_timeout,
                "authorization startup",
                self.backend.start(&mcp_url, callback.as_str()),
            )
            .await;
        let started = match started {
            Ok(started) => started,
            Err(error) => {
                self.remove_if_sequence(&attempt_id, sequence).await;
                return Err(error);
            }
        };
        if let Err(error) =
            validate_https_or_loopback(&started.authorization_url, "authorization URL")
        {
            self.remove_if_sequence(&attempt_id, sequence).await;
            return Err(McpOAuthError::new(
                McpOAuthErrorKind::Unavailable,
                error.diagnostic_detail,
            ));
        }
        let expires_at = expires_after(self.config.attempt_ttl)?;
        let mut state = self.state.lock().await;
        purge_expired(&mut state, Instant::now());
        let entry = matching_entry(&mut state, &attempt_id, sequence)?;
        if entry.phase != Phase::Starting {
            return Err(conflict("attempt changed during authorization startup"));
        }
        entry.view.authorization_url = Some(started.authorization_url);
        entry.runtime = Some(started.runtime);
        entry.phase = Phase::Waiting;
        entry.expires_at = expires_at;
        entry.protected_until = None;
        Ok(entry.view.clone())
    }

    /// Return one unexpired safe attempt view.
    pub(crate) async fn attempt(&self, attempt_id: &str) -> Option<McpOAuthSetupAttemptView> {
        let mut state = self.state.lock().await;
        purge_expired(&mut state, Instant::now());
        state
            .attempts
            .get(attempt_id)
            .map(|entry| entry.view.clone())
    }

    /// Consume a callback exactly once and return credentials to the service.
    ///
    /// # Errors
    ///
    /// Returns an error when the attempt is absent, expired, already consumed,
    /// conflicts with its current state, or callback exchange fails or is cancelled.
    pub(crate) async fn complete_callback(
        &self,
        command: CompleteMcpOAuthSetupCommand,
    ) -> McpOAuthResult<McpOAuthCompletion> {
        let attempt_id = command.attempt_id;
        let completion_expires_at = expires_after(self.config.attempt_ttl)?;
        let protected_until = expires_after(self.config.in_flight_ttl)?;
        let (sequence, context, mut runtime) = {
            let mut state = self.state.lock().await;
            purge_expired(&mut state, Instant::now());
            let entry = state.attempts.get_mut(&attempt_id).ok_or_else(not_found)?;
            if entry.phase != Phase::Waiting {
                return Err(conflict("callback was already consumed or not ready"));
            }
            entry.phase = Phase::Completing;
            entry.expires_at = completion_expires_at;
            entry.protected_until = Some(protected_until);
            entry.view.authorization_url = None;
            let runtime = entry
                .runtime
                .take()
                .ok_or_else(|| conflict("waiting attempt had no runtime"))?;
            (entry.sequence, entry.context.clone(), runtime)
        };
        let credentials = self
            .bounded(
                self.config.callback_timeout,
                "callback exchange",
                runtime.complete(&command.callback_url),
            )
            .await;
        let credentials = match credentials {
            Ok(credentials) => credentials,
            Err(error) => {
                self.fail_callback(&attempt_id, sequence, &error).await;
                return Err(error);
            }
        };
        let mut state = self.state.lock().await;
        purge_expired(&mut state, Instant::now());
        let entry = matching_entry(&mut state, &attempt_id, sequence)?;
        if entry.phase != Phase::Completing {
            return Err(conflict("attempt left callback completion"));
        }
        Ok(McpOAuthCompletion {
            attempt_id,
            sequence,
            context,
            credentials,
        })
    }

    /// Mark persistence and discovery successful.
    ///
    /// # Errors
    ///
    /// Returns an error when the attempt is absent, expired, or no longer awaiting
    /// service-side completion.
    pub(crate) async fn finish_success(
        &self,
        completion: &McpOAuthCompletion,
        result: McpServerSetupResult,
    ) -> McpOAuthResult<McpOAuthSetupAttemptView> {
        let expires_at = expires_after(self.config.attempt_ttl)?;
        let mut state = self.state.lock().await;
        purge_expired(&mut state, Instant::now());
        let entry = matching_entry(&mut state, &completion.attempt_id, completion.sequence)?;
        require_completing(entry)?;
        entry.view.status = McpOAuthSetupAttemptStatus::Completed;
        entry.view.setup_result = Some(result);
        entry.view.failure = None;
        entry.phase = Phase::Completed;
        entry.expires_at = expires_at;
        entry.protected_until = None;
        Ok(entry.view.clone())
    }

    /// Mark service-side persistence or discovery failed.
    ///
    /// # Errors
    ///
    /// Returns an error when the attempt is absent, expired, or no longer awaiting
    /// service-side completion.
    pub(crate) async fn finish_failure(
        &self,
        completion: &McpOAuthCompletion,
        failure: McpOAuthSetupFailure,
    ) -> McpOAuthResult<McpOAuthSetupAttemptView> {
        let expires_at = expires_after(self.config.attempt_ttl)?;
        let mut state = self.state.lock().await;
        purge_expired(&mut state, Instant::now());
        let entry = matching_entry(&mut state, &completion.attempt_id, completion.sequence)?;
        require_completing(entry)?;
        entry.view.status = McpOAuthSetupAttemptStatus::Failed;
        entry.view.setup_result = None;
        entry.view.failure = Some(failure);
        entry.phase = Phase::Failed;
        entry.expires_at = expires_at;
        entry.protected_until = None;
        Ok(entry.view.clone())
    }

    async fn reserve(&self, context: McpOAuthAttemptContext) -> McpOAuthResult<(String, u64)> {
        let now = Instant::now();
        let expires_at = expires_after(self.config.attempt_ttl)?;
        let protected_until = expires_after(self.config.in_flight_ttl)?;
        let mut state = self.state.lock().await;
        purge_expired(&mut state, now);
        while state.attempts.len() >= self.config.capacity {
            evict_oldest(&mut state)?;
        }
        state.next_sequence = state.next_sequence.checked_add(1).ok_or_else(|| {
            McpOAuthError::new(McpOAuthErrorKind::Unavailable, "attempt sequence exhausted")
        })?;
        let sequence = state.next_sequence;
        let attempt_id = unique_attempt_id(&state.attempts)?;
        state.attempts.insert(
            attempt_id.clone(),
            Entry {
                sequence,
                expires_at,
                view: McpOAuthSetupAttemptView {
                    attempt_id: attempt_id.clone(),
                    status: McpOAuthSetupAttemptStatus::WaitingForUser,
                    authorization_url: None,
                    setup_result: None,
                    failure: None,
                },
                context,
                runtime: None,
                phase: Phase::Starting,
                protected_until: Some(protected_until),
            },
        );
        Ok((attempt_id, sequence))
    }

    async fn remove_if_sequence(&self, attempt_id: &str, sequence: u64) {
        let mut state = self.state.lock().await;
        if state
            .attempts
            .get(attempt_id)
            .is_some_and(|entry| entry.sequence == sequence)
        {
            state.attempts.remove(attempt_id);
        }
    }

    async fn fail_callback(&self, attempt_id: &str, sequence: u64, error: &McpOAuthError) {
        let mut state = self.state.lock().await;
        let Ok(entry) = matching_entry(&mut state, attempt_id, sequence) else {
            return;
        };
        if entry.phase != Phase::Completing {
            return;
        }
        entry.view.status = McpOAuthSetupAttemptStatus::Failed;
        entry.view.failure = Some(
            if matches!(
                error.kind,
                McpOAuthErrorKind::Authentication
                    | McpOAuthErrorKind::Timeout
                    | McpOAuthErrorKind::Cancelled
            ) {
                McpOAuthSetupFailure::AuthorizationRejected
            } else {
                McpOAuthSetupFailure::CredentialPersistence
            },
        );
        entry.phase = Phase::Failed;
        entry.protected_until = None;
    }

    async fn bounded<T: Send>(
        &self,
        timeout: Duration,
        operation: &'static str,
        future: impl Future<Output = McpOAuthResult<T>> + Send,
    ) -> McpOAuthResult<T> {
        tokio::select! {
            biased;
            () = self.shutdown.cancelled() => Err(McpOAuthError::new(
                McpOAuthErrorKind::Cancelled, format!("{operation} cancelled"))),
            result = tokio::time::timeout(timeout, future) => result.map_err(|_| {
                McpOAuthError::new(McpOAuthErrorKind::Timeout, format!("{operation} exceeded {timeout:?}"))
            })?,
        }
    }
}

fn validate_config(config: &McpOAuthRegistryConfig) -> McpOAuthResult<()> {
    if config.capacity == 0
        || config.attempt_ttl.is_zero()
        || config.in_flight_ttl.is_zero()
        || config.start_timeout.is_zero()
        || config.callback_timeout.is_zero()
    {
        return Err(McpOAuthError::new(
            McpOAuthErrorKind::InvalidInput,
            "OAuth capacity, TTL, and deadlines must be non-zero",
        ));
    }
    Ok(())
}

fn matching_entry<'a>(
    state: &'a mut State,
    id: &str,
    sequence: u64,
) -> McpOAuthResult<&'a mut Entry> {
    state
        .attempts
        .get_mut(id)
        .filter(|entry| entry.sequence == sequence)
        .ok_or_else(not_found)
}

fn require_completing(entry: &Entry) -> McpOAuthResult<()> {
    (entry.phase == Phase::Completing)
        .then_some(())
        .ok_or_else(|| conflict("attempt was already terminal"))
}

fn not_found() -> McpOAuthError {
    McpOAuthError::new(McpOAuthErrorKind::NotFound, "attempt absent from registry")
}

fn conflict(detail: &'static str) -> McpOAuthError {
    McpOAuthError::new(McpOAuthErrorKind::Conflict, detail)
}

fn purge_expired(state: &mut State, now: Instant) {
    state.attempts.retain(|_, entry| {
        entry
            .protected_until
            .is_some_and(|protected_until| protected_until > now)
            || (!entry.phase.protected() && entry.expires_at > now)
    });
}

fn evict_oldest(state: &mut State) -> McpOAuthResult<()> {
    let id = state
        .attempts
        .iter()
        .filter(|(_, entry)| entry.protected_until.is_none())
        .min_by_key(|(_, entry)| entry.sequence)
        .map(|(id, _)| id.clone())
        .ok_or_else(|| McpOAuthError::new(McpOAuthErrorKind::Capacity, "no attempt to evict"))?;
    state.attempts.remove(&id);
    Ok(())
}

fn expires_after(ttl: Duration) -> McpOAuthResult<Instant> {
    Instant::now().checked_add(ttl).ok_or_else(|| {
        McpOAuthError::new(McpOAuthErrorKind::InvalidInput, "attempt TTL overflowed")
    })
}

fn unique_attempt_id(attempts: &HashMap<String, Entry>) -> McpOAuthResult<String> {
    for _ in 0..8 {
        let random = random_hex_id().map_err(|_| {
            McpOAuthError::new(McpOAuthErrorKind::Unavailable, "secure random failure")
        })?;
        let id = format!("mcp_oauth:{random}");
        if !attempts.contains_key(&id) {
            return Ok(id);
        }
    }
    Err(McpOAuthError::new(
        McpOAuthErrorKind::Unavailable,
        "secure attempt id collisions exhausted retries",
    ))
}

mod http_client;
mod protocol;
pub(crate) use protocol::resolved_resource;
mod provider;
mod validation;

#[cfg(test)]
mod tests;
