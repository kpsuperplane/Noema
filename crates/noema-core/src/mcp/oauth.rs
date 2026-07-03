//! Short-lived OAuth setup attempts for hosted MCP servers.

use std::{collections::HashMap, sync::Arc, time::Duration};

use ring::rand::{SecureRandom, SystemRandom};
use rmcp::transport::auth::OAuthState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
        let oauth_state =
            start_browser_authorization_for_mcp_url(&mcp_url, redirect_uri.as_str(), Some("Noema"))
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

/// Return whether the MCP server URL exposes browser OAuth metadata.
pub async fn oauth_authorization_supported(mcp_url: &str) -> bool {
    resolved_oauth_resource_url(mcp_url).await.is_some()
}

/// Build rmcp OAuth state for an MCP endpoint URL.
///
/// Some servers publish OAuth protected-resource metadata for the resource
/// origin while serving MCP traffic from a path under that origin. The rmcp
/// metadata validator rejects that shape when initialized with the endpoint
/// URL, so retry against the origin before surfacing the mismatch.
///
/// # Errors
///
/// Returns the SDK metadata or initialization error when no compatible OAuth
/// protected resource can be discovered.
pub async fn oauth_state_for_mcp_url(mcp_url: &str) -> Result<OAuthState, String> {
    let oauth_url = resolved_oauth_resource_url(mcp_url)
        .await
        .unwrap_or_else(|| mcp_url.to_string());
    match OAuthState::new(oauth_url.as_str(), None).await {
        Ok(state) => Ok(state),
        Err(error) => {
            let primary_error = error.to_string();
            if oauth_error_indicates_browser_auth_metadata(&primary_error)
                && let Some(origin_url) = oauth_resource_origin_url(mcp_url)
            {
                return OAuthState::new(origin_url.as_str(), None)
                    .await
                    .map_err(|origin_error| {
                        format!(
                            "{primary_error}; origin OAuth discovery failed for {origin_url}: \
                             {origin_error}"
                        )
                    });
            }
            Err(primary_error)
        }
    }
}

async fn start_browser_authorization_for_mcp_url(
    mcp_url: &str,
    redirect_uri: &str,
    client_name: Option<&str>,
) -> Result<OAuthState, String> {
    let mut oauth_state = oauth_state_for_mcp_url(mcp_url).await?;
    match oauth_state
        .start_authorization(&[], redirect_uri, client_name)
        .await
    {
        Ok(()) => Ok(oauth_state),
        Err(error) => {
            let primary_error = error.to_string();
            if oauth_error_indicates_browser_auth_metadata(&primary_error)
                && let Some(origin_url) = oauth_resource_origin_url(mcp_url)
            {
                let mut origin_state =
                    OAuthState::new(origin_url.as_str(), None)
                        .await
                        .map_err(|origin_error| {
                            format!(
                                "{primary_error}; origin OAuth initialization failed for \
                             {origin_url}: {origin_error}"
                            )
                        })?;
                origin_state
                    .start_authorization(&[], redirect_uri, client_name)
                    .await
                    .map_err(|origin_error| {
                        format!(
                            "{primary_error}; origin OAuth authorization failed for \
                             {origin_url}: {origin_error}"
                        )
                    })?;
                return Ok(origin_state);
            }
            Err(primary_error)
        }
    }
}

async fn resolved_oauth_resource_url(mcp_url: &str) -> Option<String> {
    let mcp_url = Url::parse(mcp_url).ok()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    for metadata_url in protected_resource_metadata_candidate_urls(&mcp_url) {
        let Some(metadata) = fetch_protected_resource_metadata(&client, &metadata_url).await else {
            continue;
        };
        if let Some(resource) = metadata_oauth_resource_url(&mcp_url, &metadata) {
            return Some(resource);
        }
    }
    None
}

async fn fetch_protected_resource_metadata(client: &reqwest::Client, url: &Url) -> Option<Value> {
    let response = client
        .get(url.clone())
        .header("MCP-Protocol-Version", "2024-11-05")
        .send()
        .await
        .ok()?;
    if response.status().is_success() {
        return response.json::<Value>().await.ok();
    }
    if response.status() != reqwest::StatusCode::UNAUTHORIZED {
        return None;
    }
    let metadata_url = response
        .headers()
        .get_all(reqwest::header::WWW_AUTHENTICATE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|value| www_authenticate_resource_metadata_url(value, url))?;
    client
        .get(metadata_url)
        .header("MCP-Protocol-Version", "2024-11-05")
        .send()
        .await
        .ok()?
        .json::<Value>()
        .await
        .ok()
}

fn protected_resource_metadata_candidate_urls(mcp_url: &Url) -> Vec<Url> {
    let mut urls = Vec::new();
    let trimmed_path = mcp_url.path().trim_start_matches('/').trim_end_matches('/');
    if !trimmed_path.is_empty() {
        urls.push(with_path(
            mcp_url,
            &format!("/.well-known/oauth-protected-resource/{trimmed_path}"),
        ));
    }
    urls.push(with_path(mcp_url, "/.well-known/oauth-protected-resource"));
    urls
}

fn with_path(base: &Url, path: &str) -> Url {
    let mut url = base.clone();
    url.set_query(None);
    url.set_fragment(None);
    url.set_path(path);
    url
}

fn metadata_oauth_resource_url(mcp_url: &Url, metadata: &Value) -> Option<String> {
    let resource = metadata.get("resource")?.as_str()?;
    if !metadata_has_authorization_server(metadata) {
        return None;
    }
    if resource_identifiers_match(mcp_url.as_str(), resource) {
        return Some(resource.to_string());
    }
    let origin = oauth_resource_origin_url(mcp_url.as_str())?;
    resource_identifiers_match(origin.as_str(), resource).then(|| resource.to_string())
}

fn metadata_has_authorization_server(metadata: &Value) -> bool {
    metadata
        .get("authorization_server")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty())
        || metadata
            .get("authorization_servers")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str().is_some()))
}

fn www_authenticate_resource_metadata_url(header: &str, base: &Url) -> Option<Url> {
    let prefix = "resource_metadata=\"";
    let start = header.find(prefix)? + prefix.len();
    let end = header[start..].find('"')? + start;
    let value = &header[start..end];
    Url::parse(value).or_else(|_| base.join(value)).ok()
}

fn oauth_error_indicates_browser_auth_metadata(message: &str) -> bool {
    message.contains("Protected resource metadata resource mismatch")
}

fn resource_identifiers_match(expected: &str, actual: &str) -> bool {
    expected == actual
        || (is_root_resource_identifier(expected) && actual == expected.trim_end_matches('/'))
        || (is_root_resource_identifier(actual) && expected == actual.trim_end_matches('/'))
}

fn is_root_resource_identifier(value: &str) -> bool {
    Url::parse(value)
        .is_ok_and(|url| url.path() == "/" && url.query().is_none() && url.fragment().is_none())
}

fn oauth_resource_origin_url(mcp_url: &str) -> Option<String> {
    let mut url = Url::parse(mcp_url).ok()?;
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_resource_mismatch_still_indicates_browser_auth_metadata() {
        assert!(oauth_error_indicates_browser_auth_metadata(
            "Metadata error: Protected resource metadata resource mismatch: expected \
             'https://mcp.getdex.com/mcp', got 'https://mcp.getdex.com/'"
        ));
    }

    #[test]
    fn oauth_resource_origin_url_uses_url_origin() {
        assert_eq!(
            oauth_resource_origin_url("https://mcp.getdex.com/mcp?transport=streamable#fragment")
                .as_deref(),
            Some("https://mcp.getdex.com/")
        );
    }

    #[test]
    fn protected_resource_metadata_candidates_include_path_then_origin() {
        let url = Url::parse("https://mcp.getdex.com/mcp").expect("url");

        let candidates = protected_resource_metadata_candidate_urls(&url);

        assert_eq!(
            candidates.iter().map(Url::as_str).collect::<Vec<_>>(),
            vec![
                "https://mcp.getdex.com/.well-known/oauth-protected-resource/mcp",
                "https://mcp.getdex.com/.well-known/oauth-protected-resource"
            ]
        );
    }

    #[test]
    fn metadata_oauth_resource_accepts_origin_resource() {
        let url = Url::parse("https://mcp.getdex.com/mcp").expect("url");
        let metadata = serde_json::json!({
            "resource": "https://mcp.getdex.com/",
            "authorization_servers": ["https://mcp.getdex.com/"]
        });

        assert_eq!(
            metadata_oauth_resource_url(&url, &metadata).as_deref(),
            Some("https://mcp.getdex.com/")
        );
    }

    #[test]
    fn metadata_oauth_resource_rejects_unrelated_resource() {
        let url = Url::parse("https://mcp.getdex.com/mcp").expect("url");
        let metadata = serde_json::json!({
            "resource": "https://other.example/",
            "authorization_servers": ["https://mcp.getdex.com/"]
        });

        assert_eq!(metadata_oauth_resource_url(&url, &metadata), None);
    }
}
