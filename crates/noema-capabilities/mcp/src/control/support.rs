use crate::{
    LocalMcpService, McpClientError, McpOAuthSetupFailure, McpOperationError, McpSecretMaterial,
    McpSecretStage, McpServerAuthStatus, McpServerHealthStatus, McpServerRecord,
    McpServerSetupResult, McpSetupAuthDetails, McpSetupStatus, McpTransportKind,
    service::{
        map_oauth_operation_error, map_repository_operation_error, map_secret_operation_error,
    },
};

#[derive(Debug, Clone, Copy)]
pub(super) struct SetupRunError {
    pub(super) operation: McpOperationError,
    pub(super) oauth_failure: McpOAuthSetupFailure,
}

impl SetupRunError {
    pub(super) const fn discovery(operation: McpOperationError) -> Self {
        Self {
            operation,
            oauth_failure: McpOAuthSetupFailure::DiscoveryFailed,
        }
    }

    pub(super) const fn credentials(operation: McpOperationError) -> Self {
        Self {
            operation,
            oauth_failure: McpOAuthSetupFailure::CredentialPersistence,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct SetupFailureProjection {
    pub(super) setup_status: McpSetupStatus,
    pub(super) health_status: McpServerHealthStatus,
    pub(super) auth_status: McpServerAuthStatus,
    pub(super) auth: Option<McpSetupAuthDetails>,
}

impl LocalMcpService {
    pub(super) fn stage_secrets(
        &self,
        secrets: &McpSecretMaterial,
        id: Option<&str>,
        operation: &'static str,
    ) -> Result<McpSecretStage, SetupRunError> {
        self.inner.secrets.stage(secrets).map_err(|error| {
            self.diagnostic(id, operation, "MCP credentials could not be staged", &error);
            SetupRunError::credentials(map_secret_operation_error(&error))
        })
    }

    pub(super) fn discard_stage(
        &self,
        stage: McpSecretStage,
        id: Option<&str>,
        operation: &'static str,
    ) {
        if let Err(error) = self.inner.secrets.discard(stage) {
            self.diagnostic(
                id,
                operation,
                "MCP staged credential cleanup failed",
                &error,
            );
        }
    }

    pub(super) fn finalize_secret_commit(
        &self,
        commit: crate::McpSecretCommit,
        id: Option<&str>,
        operation: &'static str,
    ) {
        if let Err(error) = self.inner.secrets.finalize(commit) {
            self.diagnostic(
                id,
                operation,
                "MCP credential backup cleanup failed",
                &error,
            );
        }
    }

    pub(super) async fn compensate_initial_commit(&self, id: &str) {
        match self.inner.repository.begin_delete(id.to_string()).await {
            Ok(Some(ticket)) => match self.inner.repository.finish_delete(ticket).await {
                Ok(true) => {}
                Ok(false) => self.diagnostic(
                    Some(id),
                    "create_server_compensation",
                    "MCP server compensation did not remove the committed row",
                    &"the deletion fence was no longer current",
                ),
                Err(error) => {
                    self.diagnostic(
                        Some(id),
                        "create_server_compensation",
                        "MCP server compensation failed",
                        &error,
                    );
                }
            },
            Ok(None) => self.diagnostic(
                Some(id),
                "create_server_compensation",
                "MCP server compensation could not find the committed row",
                &"the newly committed server was not found",
            ),
            Err(error) => self.diagnostic(
                Some(id),
                "create_server_compensation",
                "MCP server compensation could not begin",
                &error,
            ),
        }
    }

    pub(super) fn repository_error(
        &self,
        id: Option<&str>,
        operation: &'static str,
        error: &crate::McpRepositoryError,
    ) -> McpOperationError {
        self.diagnostic(id, operation, "MCP repository operation failed", error);
        map_repository_operation_error(error)
    }

    pub(super) fn oauth_error(
        &self,
        id: Option<&str>,
        operation: &'static str,
        error: &crate::McpOAuthError,
    ) -> McpOperationError {
        self.diagnostic(
            id,
            operation,
            "MCP OAuth operation failed",
            &error.diagnostic_detail(),
        );
        map_oauth_operation_error(error)
    }

    pub(super) fn diagnostic(
        &self,
        id: Option<&str>,
        operation: &'static str,
        message: &'static str,
        error: &impl std::fmt::Display,
    ) {
        self.inner
            .record_failure(id, None, operation, message, error);
    }
}

pub(super) fn validate_id(id: String) -> Result<String, McpOperationError> {
    let id = id.trim();
    if id.is_empty() {
        Err(McpOperationError::InvalidInput)
    } else {
        Ok(id.to_string())
    }
}

pub(super) fn streamable_http_url(server: &McpServerRecord) -> Option<String> {
    if server.transport_kind != McpTransportKind::StreamableHttp {
        return None;
    }
    server
        .safe_config
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

pub(super) fn success_result(server: McpServerRecord, tool_count: usize) -> McpServerSetupResult {
    McpServerSetupResult {
        server: Some(server),
        setup_status: McpSetupStatus::ReadyForPolicy,
        discovered_tool_count: tool_count,
        auth: None,
    }
}

pub(super) fn authentication_available_result(tool_count: usize) -> McpServerSetupResult {
    McpServerSetupResult {
        server: None,
        setup_status: McpSetupStatus::AuthenticationAvailable,
        discovered_tool_count: tool_count,
        auth: Some(McpSetupAuthDetails {
            oauth_client_credentials_supported: true,
            oauth_authorization_supported: true,
            scopes: Vec::new(),
        }),
    }
}

pub(super) fn setup_failure_result(
    server: Option<McpServerRecord>,
    projection: SetupFailureProjection,
) -> McpServerSetupResult {
    McpServerSetupResult {
        server,
        setup_status: projection.setup_status,
        discovered_tool_count: 0,
        auth: projection.auth,
    }
}

pub(super) fn client_failure_projection(
    transport_kind: McpTransportKind,
    error: &McpClientError,
) -> SetupFailureProjection {
    match error {
        McpClientError::AuthenticationRequired(_) => SetupFailureProjection {
            setup_status: McpSetupStatus::NeedsAuth,
            health_status: McpServerHealthStatus::Unavailable,
            auth_status: McpServerAuthStatus::NeedsAuth,
            auth: (transport_kind == McpTransportKind::StreamableHttp).then(|| {
                McpSetupAuthDetails {
                    oauth_client_credentials_supported: true,
                    oauth_authorization_supported: true,
                    scopes: Vec::new(),
                }
            }),
        },
        McpClientError::Malformed(_) => malformed_projection(),
        McpClientError::Unavailable(_)
        | McpClientError::UnsupportedMetadata(_)
        | McpClientError::Protocol(_)
        | McpClientError::Timeout { .. }
        | McpClientError::Cancelled { .. } => SetupFailureProjection {
            setup_status: McpSetupStatus::Unavailable,
            health_status: McpServerHealthStatus::Unavailable,
            auth_status: McpServerAuthStatus::Unavailable,
            auth: None,
        },
    }
}

pub(super) fn malformed_projection() -> SetupFailureProjection {
    SetupFailureProjection {
        setup_status: McpSetupStatus::Malformed,
        health_status: McpServerHealthStatus::Unavailable,
        auth_status: McpServerAuthStatus::Unavailable,
        auth: None,
    }
}
