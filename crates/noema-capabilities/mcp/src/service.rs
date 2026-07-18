//! Root-bound local MCP service composition and shared lifecycle.

use noema_capabilities::{
    CapabilityBindingSourceHandle, CapabilityInvokerRegistration, InvokerKey,
};
use std::{collections::HashMap, fmt, future::Future, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::{Mutex, OwnedMutexGuard, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

use crate::catalog::{MCP_INVOKER_KEY, catalog_from_servers, map_repository_error};
use crate::{
    McpAutofillCompletionHandle, McpClientError, McpControlPlaneHandle, McpDiagnosticEvent,
    McpDiagnosticHandle, McpDiagnosticKind, McpOAuthError, McpOAuthRegistry,
    McpOAuthRegistryConfig, McpOperationError, McpPreparedSession, McpRepositoryError,
    McpRepositoryErrorKind, McpRepositoryHandle, McpRequestContext, McpSecretStoreError,
    McpSecretStoreHandle, McpSessionFactoryHandle, lifecycle::McpServiceLifecycle,
};

/// Local MCP deadlines and OAuth registry bounds.
#[derive(Debug, Clone)]
pub struct LocalMcpServiceConfig {
    /// Maximum time for transport initialization and metadata discovery.
    pub discovery_timeout: Duration,
    /// Maximum time for transport initialization and one tool call.
    pub invocation_timeout: Duration,
    /// Maximum graceful shutdown drain before returning to the host.
    pub shutdown_timeout: Duration,
    /// Short-lived browser OAuth attempt policy.
    pub oauth: McpOAuthRegistryConfig,
}

impl Default for LocalMcpServiceConfig {
    fn default() -> Self {
        Self {
            discovery_timeout: Duration::from_secs(30),
            invocation_timeout: Duration::from_secs(60),
            shutdown_timeout: Duration::from_secs(5),
            oauth: McpOAuthRegistryConfig::default(),
        }
    }
}

/// Failure constructing the root-bound MCP service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LocalMcpServiceConstructionError {
    /// Abandoned filesystem staging could not be cleaned safely.
    #[error("MCP secret storage is unavailable")]
    SecretStorage,
    /// OAuth registry policy is invalid.
    #[error("MCP OAuth setup is unavailable")]
    OAuth,
}

/// One root-bound MCP service shared by API and runtime consumers.
#[derive(Clone)]
pub struct LocalMcpService {
    pub(crate) inner: Arc<LocalMcpServiceInner>,
}

pub(crate) struct LocalMcpServiceInner {
    pub(crate) repository: McpRepositoryHandle,
    pub(crate) secrets: McpSecretStoreHandle,
    pub(crate) sessions: McpSessionFactoryHandle,
    pub(crate) diagnostics: McpDiagnosticHandle,
    pub(crate) completion: Option<McpAutofillCompletionHandle>,
    pub(crate) oauth: McpOAuthRegistry,
    pub(crate) lifecycle: Arc<McpServiceLifecycle>,
    pub(crate) config: LocalMcpServiceConfig,
    server_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    policy_locks: Mutex<HashMap<String, Arc<RwLock<()>>>>,
}

impl fmt::Debug for LocalMcpService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalMcpService")
            .field("inner", &"[CONFIGURED]")
            .finish()
    }
}

impl fmt::Debug for LocalMcpServiceInner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalMcpServiceInner")
            .field("repository", &"[CONFIGURED]")
            .field("secrets", &"[REDACTED]")
            .field("sessions", &"[CONFIGURED]")
            .field("diagnostics", &"[CONFIGURED]")
            .field(
                "completion",
                &self.completion.as_ref().map(|_| "[CONFIGURED]"),
            )
            .finish_non_exhaustive()
    }
}

impl LocalMcpService {
    pub(crate) async fn run_admitted_operation<T>(
        &self,
        operation: impl Future<Output = Result<T, McpOperationError>>,
    ) -> Result<T, McpOperationError> {
        self.inner.lifecycle.run_admitted(operation).await?
    }

    /// Construct the single service instance owned by the process root.
    ///
    /// # Errors
    ///
    /// Returns a fixed construction failure if abandoned secret staging cannot
    /// be cleaned or the OAuth registry policy is invalid.
    pub fn new(
        repository: McpRepositoryHandle,
        secrets: McpSecretStoreHandle,
        diagnostics: McpDiagnosticHandle,
        completion: Option<McpAutofillCompletionHandle>,
        config: LocalMcpServiceConfig,
        build_sessions: impl FnOnce(McpOAuthRegistry) -> McpSessionFactoryHandle,
    ) -> Result<Self, LocalMcpServiceConstructionError> {
        secrets
            .cleanup_abandoned_staging()
            .map_err(|_| LocalMcpServiceConstructionError::SecretStorage)?;
        let lifecycle = Arc::new(McpServiceLifecycle::default());
        let oauth = McpOAuthRegistry::new(config.oauth.clone(), lifecycle.cancellation_token())
            .map_err(|_| LocalMcpServiceConstructionError::OAuth)?;
        let sessions = build_sessions(oauth.clone());
        Ok(Self {
            inner: Arc::new(LocalMcpServiceInner {
                repository,
                secrets,
                sessions,
                diagnostics,
                completion,
                oauth,
                lifecycle,
                config,
                server_locks: Mutex::new(HashMap::new()),
                policy_locks: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Return the settings/setup control-plane handle.
    #[must_use]
    pub fn operations(&self) -> McpControlPlaneHandle {
        Arc::new(self.clone())
    }

    /// Return the request-local catalog source sharing this service lifecycle.
    #[must_use]
    pub fn binding_source(&self) -> CapabilityBindingSourceHandle {
        Arc::new(self.clone())
    }

    /// Return the keyed registration consumed by a generic capability router.
    #[must_use]
    pub fn invoker_registration(&self) -> CapabilityInvokerRegistration {
        CapabilityInvokerRegistration::new(InvokerKey::new(MCP_INVOKER_KEY), Arc::new(self.clone()))
    }

    /// Reject new work and cancel admitted transport/OAuth work.
    pub fn begin_shutdown(&self) {
        self.inner.lifecycle.begin_shutdown();
    }

    /// Cancel, drain, and finish the service lifecycle.
    ///
    /// Returns `false` if admitted work did not cooperate before the configured
    /// drain deadline.
    pub async fn shutdown(&self) -> bool {
        self.inner
            .lifecycle
            .drain(self.inner.config.shutdown_timeout)
            .await
    }
}

impl LocalMcpServiceInner {
    pub(crate) fn request_context(&self, timeout: Duration) -> McpRequestContext {
        McpRequestContext::with_timeout(timeout, self.lifecycle.cancellation_token())
    }

    pub(crate) async fn lock_server(
        &self,
        mcp_server_id: &str,
        context: &McpRequestContext,
    ) -> Result<OwnedMutexGuard<()>, McpOperationError> {
        let lock = {
            let mut locks = self.server_locks.lock().await;
            locks
                .entry(mcp_server_id.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        Self::lock_with_context(context, lock.lock_owned()).await
    }

    pub(crate) async fn lock_policy_read(
        &self,
        mcp_server_id: &str,
        context: &McpRequestContext,
    ) -> Result<OwnedRwLockReadGuard<()>, McpOperationError> {
        let lock = {
            let mut locks = self.policy_locks.lock().await;
            locks
                .entry(mcp_server_id.to_string())
                .or_insert_with(|| Arc::new(RwLock::new(())))
                .clone()
        };
        Self::lock_with_context(context, lock.read_owned()).await
    }

    pub(crate) async fn lock_policy_write(
        &self,
        mut mcp_server_ids: Vec<String>,
        context: &McpRequestContext,
    ) -> Result<Vec<OwnedRwLockWriteGuard<()>>, McpOperationError> {
        mcp_server_ids.sort_unstable();
        mcp_server_ids.dedup();
        let locks = {
            let mut locks = self.policy_locks.lock().await;
            mcp_server_ids
                .into_iter()
                .map(|mcp_server_id| {
                    locks
                        .entry(mcp_server_id)
                        .or_insert_with(|| Arc::new(RwLock::new(())))
                        .clone()
                })
                .collect::<Vec<_>>()
        };
        let mut guards = Vec::with_capacity(locks.len());
        for lock in locks {
            guards.push(Self::lock_with_context(context, lock.write_owned()).await?);
        }
        Ok(guards)
    }

    async fn lock_with_context<T>(
        context: &McpRequestContext,
        lock: impl Future<Output = T>,
    ) -> Result<T, McpOperationError> {
        let cancellation = context.cancellation_token();
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(McpOperationError::Cancelled),
            result = tokio::time::timeout_at(
                tokio::time::Instant::from_std(context.deadline()), lock
            ) => result.map_err(|_| McpOperationError::TimedOut),
        }
    }

    pub(crate) async fn close_session(
        &self,
        session: Box<dyn McpPreparedSession>,
        mcp_server_id: Option<&str>,
        operation: &'static str,
    ) {
        if let Err(error) = session.close().await {
            self.record_failure(
                mcp_server_id,
                None,
                operation,
                "MCP session cleanup failed",
                &error,
            );
        }
    }

    pub(crate) fn record_failure(
        &self,
        mcp_server_id: Option<&str>,
        tool_name: Option<&str>,
        operation: &'static str,
        message: &'static str,
        error: &impl fmt::Display,
    ) {
        self.diagnostics.record(McpDiagnosticEvent {
            kind: McpDiagnosticKind::OperationFailure,
            message: message.to_string(),
            mcp_server_id: mcp_server_id.map(str::to_string),
            tool_name: tool_name.map(str::to_string),
            operation,
            error_chain: vec![error.to_string()],
            raw: None,
        });
    }
}

pub(crate) fn map_repository_operation_error(error: &McpRepositoryError) -> McpOperationError {
    match error.kind() {
        McpRepositoryErrorKind::NotFound => McpOperationError::NotFound,
        McpRepositoryErrorKind::Conflict => McpOperationError::Conflict,
        McpRepositoryErrorKind::Invariant | McpRepositoryErrorKind::Unavailable => {
            McpOperationError::Unavailable
        }
    }
}

pub(crate) fn map_secret_operation_error(_error: &McpSecretStoreError) -> McpOperationError {
    McpOperationError::Unavailable
}

pub(crate) fn map_client_operation_error(error: &McpClientError) -> McpOperationError {
    match error {
        McpClientError::AuthenticationRequired(_) => McpOperationError::AuthenticationRequired,
        McpClientError::Unavailable(_) | McpClientError::Protocol(_) => {
            McpOperationError::Unavailable
        }
        McpClientError::Malformed(_) => McpOperationError::MalformedResponse,
        McpClientError::Timeout { .. } => McpOperationError::TimedOut,
        McpClientError::Cancelled { .. } => McpOperationError::Cancelled,
    }
}

pub(crate) fn map_oauth_operation_error(error: &McpOAuthError) -> McpOperationError {
    use crate::McpOAuthErrorKind;

    match error.kind() {
        McpOAuthErrorKind::InvalidInput => McpOperationError::InvalidInput,
        McpOAuthErrorKind::NotFound => McpOperationError::NotFound,
        McpOAuthErrorKind::Capacity => McpOperationError::Unavailable,
        McpOAuthErrorKind::Conflict => McpOperationError::Conflict,
        McpOAuthErrorKind::Authentication => McpOperationError::AuthenticationRequired,
        McpOAuthErrorKind::Timeout => McpOperationError::TimedOut,
        McpOAuthErrorKind::Cancelled => McpOperationError::Cancelled,
        McpOAuthErrorKind::Unavailable => McpOperationError::Unavailable,
    }
}

impl noema_capabilities::CapabilityBindingSource for LocalMcpService {
    fn catalog(
        &self,
    ) -> noema_capabilities::CapabilityFuture<
        '_,
        Result<
            noema_capabilities::CapabilityCatalogResult,
            noema_capabilities::CapabilityBindingSourceError,
        >,
    > {
        Box::pin(async move {
            self.inner
                .lifecycle
                .run_admitted(async {
                    let servers = self
                        .inner
                        .repository
                        .control_plane_catalog()
                        .await
                        .map_err(map_repository_error)?;
                    catalog_from_servers(&servers)
                })
                .await
                .map_err(|_| noema_capabilities::CapabilityBindingSourceError::Unavailable)?
        })
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
