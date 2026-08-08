//! Transport-neutral MCP OAuth commands and attempt views.

use std::{collections::BTreeSet, fmt};

use thiserror::Error;

use crate::setup_model::{CreateMcpServerCommand, McpServerSetupResult};

const REDACTED: &str = "[REDACTED]";

/// Short-lived OAuth setup attempt status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpOAuthSetupAttemptStatus {
    /// Waiting for browser authorization.
    WaitingForUser,
    /// Browser authorization and metadata discovery completed.
    Completed,
    /// Authorization or discovery failed.
    Failed,
}

/// Fixed OAuth failure safe for control-plane consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpOAuthSetupFailure {
    /// The attempt does not exist or has expired.
    #[error("the MCP OAuth setup attempt was not found or has expired")]
    Expired,
    /// The authorization server rejected callback completion.
    #[error("Noema could not complete MCP OAuth authorization")]
    AuthorizationRejected,
    /// Credentials could not be committed safely.
    #[error("Noema could not store MCP OAuth credentials")]
    CredentialPersistence,
    /// OAuth completed but metadata discovery did not reach provider-policy setup.
    #[error("OAuth completed, but Noema could not list tools from this MCP server")]
    DiscoveryFailed,
    /// A newer attempt or connection deletion invalidated this attempt.
    #[error("this MCP OAuth setup attempt was superseded")]
    Superseded,
}

/// Start browser OAuth for a pending, not-yet-persisted server setup.
#[derive(Clone, PartialEq)]
pub struct StartMcpOAuthSetupCommand {
    /// Authenticated human initiating the browser flow.
    pub owner_human_id: String,
    /// Pending server setup to commit only after authorization and discovery.
    pub setup: CreateMcpServerCommand,
    /// Callback base URL owned by the server or desktop listener.
    pub redirect_uri: String,
}

impl fmt::Debug for StartMcpOAuthSetupCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StartMcpOAuthSetupCommand")
            .field("setup", &self.setup)
            .field("redirect_uri", &self.redirect_uri)
            .finish()
    }
}

/// Start browser OAuth reauthentication for an existing MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartMcpOAuthReauthenticationCommand {
    /// Authenticated human initiating the browser flow.
    pub owner_human_id: String,
    /// Existing durable MCP server identifier.
    pub mcp_server_id: String,
    /// Callback base URL owned by the server or desktop listener.
    pub redirect_uri: String,
}

/// Query one short-lived OAuth setup attempt.
#[derive(Clone, PartialEq, Eq)]
pub struct McpOAuthSetupAttemptQuery {
    /// Opaque attempt identifier returned by a start operation.
    pub attempt_id: String,
    /// Authenticated owner polling the attempt; callbacks may omit this.
    pub owner_human_id: Option<String>,
}

impl fmt::Debug for McpOAuthSetupAttemptQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthSetupAttemptQuery")
            .field("attempt_id", &self.attempt_id)
            .field("owner_human_id", &self.owner_human_id)
            .finish()
    }
}

/// Complete an OAuth setup attempt from the listener-owned callback URL.
#[derive(Clone, PartialEq, Eq)]
pub struct CompleteMcpOAuthSetupCommand {
    /// Opaque attempt identifier returned by a start operation.
    pub attempt_id: String,
    /// Full callback URL containing authorization response parameters.
    pub callback_url: String,
}

impl fmt::Debug for CompleteMcpOAuthSetupCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompleteMcpOAuthSetupCommand")
            .field("attempt_id", &self.attempt_id)
            .field("callback_url", &REDACTED)
            .finish()
    }
}

/// Safe OAuth attempt state returned to control-plane consumers.
#[derive(Clone, PartialEq)]
pub struct McpOAuthSetupAttemptView {
    /// Opaque short-lived attempt identifier.
    pub attempt_id: String,
    /// Current attempt status.
    pub status: McpOAuthSetupAttemptStatus,
    /// Authorization URL intentionally returned for browser navigation.
    pub authorization_url: Option<String>,
    /// Final setup result after callback completion and discovery.
    pub setup_result: Option<McpServerSetupResult>,
    /// Fixed safe failure, when the attempt failed.
    pub failure: Option<McpOAuthSetupFailure>,
}

impl fmt::Debug for McpOAuthSetupAttemptView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpOAuthSetupAttemptView")
            .field("attempt_id", &self.attempt_id)
            .field("status", &self.status)
            .field(
                "authorization_url",
                &self
                    .authorization_url
                    .as_deref()
                    .map(sanitized_authorization_url),
            )
            .field("setup_result", &self.setup_result)
            .field("failure", &self.failure)
            .finish()
    }
}

fn sanitized_authorization_url(raw_url: &str) -> String {
    noema_capabilities::sanitize_url_credentials(raw_url, &BTreeSet::from(["state".to_string()]))
        .map_or_else(|| "[INVALID URL]".to_string(), |(url, _)| url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_preserves_attempt_identity_without_exposing_oauth_secrets() {
        let query = McpOAuthSetupAttemptQuery {
            attempt_id: "attempt-public".to_string(),
            owner_human_id: Some("human:local".to_string()),
        };
        let callback = CompleteMcpOAuthSetupCommand {
            attempt_id: "attempt-public".to_string(),
            callback_url: "http://127.0.0.1/callback?code=callback-secret".to_string(),
        };
        let view = McpOAuthSetupAttemptView {
            attempt_id: "attempt-public".to_string(),
            status: McpOAuthSetupAttemptStatus::WaitingForUser,
            authorization_url: Some(
                "https://auth.example/authorize?prompt=consent&state=authorization-secret"
                    .to_string(),
            ),
            setup_result: None,
            failure: None,
        };

        let debug = format!("{query:?} {callback:?} {view:?}");

        assert!(!debug.contains("callback-secret"));
        assert!(!debug.contains("authorization-secret"));
        assert!(debug.contains("attempt-public"));
        assert!(debug.contains("human:local"));
        assert!(debug.contains("https://auth.example/authorize?prompt=consent"));
        assert!(debug.contains(REDACTED));
    }
}
