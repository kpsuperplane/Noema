//! Short-lived OAuth setup attempts for hosted MCP servers.

use std::{collections::HashMap, sync::Arc};

use ring::rand::{SecureRandom, SystemRandom};
use rmcp::transport::auth::OAuthState;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use url::Url;

use crate::{
    McpTransportKind, StoreError,
    mcp::{
        secrets::{McpOAuthStoredCredentials, McpSecretMaterial},
        setup::{McpServerSetupResult, NewMcpServerSetup},
    },
};

/// Short-lived OAuth setup attempt status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpOAuthSetupAttemptStatus {
    /// Waiting for the user to complete browser authorization.
    WaitingForUser,
    /// Browser authorization and MCP metadata discovery completed.
    Completed,
    /// Authorization or metadata discovery failed.
    Failed,
}

impl McpOAuthSetupAttemptStatus {
    /// Return the GraphQL/status string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WaitingForUser => "waiting_for_user",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// Safe MCP OAuth setup attempt state returned to the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct McpOAuthSetupAttemptView {
    /// Short-lived attempt id.
    pub attempt_id: String,
    /// Current attempt status.
    pub status: McpOAuthSetupAttemptStatus,
    /// Authorization URL to open in the user's browser.
    pub authorization_url: Option<String>,
    /// Final setup result after OAuth callback and tool discovery.
    pub setup_result: Option<McpServerSetupResult>,
    /// Non-secret UI-safe failure message.
    pub error_message: Option<String>,
}

/// Request to start a hosted MCP OAuth setup attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct StartMcpOAuthSetupRequest {
    /// Pending server setup input. The server is not persisted until discovery succeeds.
    pub setup: NewMcpServerSetup,
    /// Browser callback base URL owned by the local Noema web server.
    pub redirect_uri: String,
}

/// Runtime OAuth setup state that must not be exposed to GraphQL clients.
pub struct McpOAuthSetupAttemptRuntime {
    /// SDK OAuth state machine.
    pub oauth_state: OAuthState,
    /// Pending server setup to persist after successful authorization and discovery.
    pub setup: NewMcpServerSetup,
}

/// In-memory manager for short-lived MCP OAuth setup attempts.
#[derive(Clone, Default)]
pub struct McpOAuthSetupManager {
    views: Arc<Mutex<HashMap<String, McpOAuthSetupAttemptView>>>,
    runtimes: Arc<Mutex<HashMap<String, McpOAuthSetupAttemptRuntime>>>,
}

impl McpOAuthSetupManager {
    /// Build an empty MCP OAuth setup manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a browser OAuth setup attempt for a hosted HTTP MCP server.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the setup input is not an HTTP MCP server
    /// or the SDK cannot begin OAuth metadata discovery.
    pub async fn start_attempt(
        &self,
        request: StartMcpOAuthSetupRequest,
    ) -> Result<McpOAuthSetupAttemptView, StoreError> {
        let mcp_url = mcp_url_from_setup(&request.setup)?;
        let attempt_id = random_attempt_id()?;
        let redirect_uri = callback_uri_with_attempt(&request.redirect_uri, &attempt_id)?;
        let mut oauth_state = OAuthState::new(&mcp_url, None)
            .await
            .map_err(|error| StoreError::Schema(format!("MCP OAuth setup failed: {error}")))?;
        oauth_state
            .start_authorization(&[], redirect_uri.as_str(), Some("Noema"))
            .await
            .map_err(|error| StoreError::Schema(format!("MCP OAuth setup failed: {error}")))?;
        let authorization_url = oauth_state
            .get_authorization_url()
            .await
            .map_err(|error| StoreError::Schema(format!("MCP OAuth setup failed: {error}")))?;
        let view = McpOAuthSetupAttemptView {
            attempt_id: attempt_id.clone(),
            status: McpOAuthSetupAttemptStatus::WaitingForUser,
            authorization_url: Some(authorization_url),
            setup_result: None,
            error_message: None,
        };
        self.views
            .lock()
            .await
            .insert(attempt_id.clone(), view.clone());
        self.runtimes.lock().await.insert(
            attempt_id,
            McpOAuthSetupAttemptRuntime {
                oauth_state,
                setup: request.setup,
            },
        );
        Ok(view)
    }

    /// Return a safe attempt view, if one is still known.
    pub async fn attempt(&self, attempt_id: &str) -> Option<McpOAuthSetupAttemptView> {
        self.views.lock().await.get(attempt_id).cloned()
    }

    /// Remove runtime state before completing a callback.
    pub async fn take_runtime(&self, attempt_id: &str) -> Option<McpOAuthSetupAttemptRuntime> {
        self.runtimes.lock().await.remove(attempt_id)
    }

    /// Mark an attempt completed with the final setup result.
    pub async fn complete_attempt(
        &self,
        attempt_id: &str,
        setup_result: McpServerSetupResult,
    ) -> Option<McpOAuthSetupAttemptView> {
        self.update_attempt(attempt_id, |view| {
            view.status = McpOAuthSetupAttemptStatus::Completed;
            view.authorization_url = None;
            view.setup_result = Some(setup_result);
            view.error_message = None;
        })
        .await
    }

    /// Mark an attempt failed with a safe message.
    pub async fn fail_attempt(
        &self,
        attempt_id: &str,
        message: impl Into<String>,
    ) -> Option<McpOAuthSetupAttemptView> {
        let message = message.into();
        self.update_attempt(attempt_id, |view| {
            view.status = McpOAuthSetupAttemptStatus::Failed;
            view.authorization_url = None;
            view.error_message = Some(message);
        })
        .await
    }

    async fn update_attempt(
        &self,
        attempt_id: &str,
        update: impl FnOnce(&mut McpOAuthSetupAttemptView),
    ) -> Option<McpOAuthSetupAttemptView> {
        let mut views = self.views.lock().await;
        let view = views.get_mut(attempt_id)?;
        update(view);
        Some(view.clone())
    }
}

/// Build secret material containing SDK OAuth credentials after callback completion.
///
/// # Errors
///
/// Returns [`StoreError`] when the SDK cannot expose serializable credentials.
pub async fn oauth_secret_material(
    oauth_state: &OAuthState,
    mut base: McpSecretMaterial,
) -> Result<McpSecretMaterial, StoreError> {
    let (client_id, token_response) = oauth_state
        .get_credentials()
        .await
        .map_err(|error| StoreError::Schema(format!("MCP OAuth credentials failed: {error}")))?;
    let token_response = token_response.ok_or_else(|| {
        StoreError::Schema("MCP OAuth did not return token credentials".to_string())
    })?;
    base.oauth_credentials = Some(McpOAuthStoredCredentials {
        client_id,
        token_response: serde_json::to_value(token_response).map_err(|error| {
            StoreError::Schema(format!(
                "MCP OAuth credentials could not be stored: {error}"
            ))
        })?,
    });
    Ok(base)
}

fn mcp_url_from_setup(setup: &NewMcpServerSetup) -> Result<String, StoreError> {
    if !matches!(
        setup.transport_kind,
        McpTransportKind::Sse | McpTransportKind::StreamableHttp
    ) {
        return Err(StoreError::Schema(
            "MCP OAuth setup is only available for HTTP transports".to_string(),
        ));
    }
    setup
        .safe_config
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| StoreError::Schema("MCP OAuth setup requires an HTTP URL".to_string()))
}

fn callback_uri_with_attempt(redirect_uri: &str, attempt_id: &str) -> Result<String, StoreError> {
    let mut url = Url::parse(redirect_uri)
        .map_err(|error| StoreError::Schema(format!("invalid MCP OAuth redirect URI: {error}")))?;
    url.query_pairs_mut().append_pair("attemptId", attempt_id);
    Ok(url.to_string())
}

fn random_attempt_id() -> Result<String, StoreError> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| StoreError::Schema("failed to create MCP OAuth attempt id".to_string()))?;
    Ok(format!("mcp_oauth:{}", hex_bytes(&bytes)))
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
