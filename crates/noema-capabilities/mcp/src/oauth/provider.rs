//! Streamable HTTP bearer authorization and credential refresh.

use rmcp::transport::auth::{ClientCredentialsConfig, OAuthState, OAuthTokenResponse};
use serde_json::Value;

use super::{McpOAuthError, McpOAuthErrorKind, McpOAuthRegistry, McpOAuthResult};
use crate::{
    McpOAuthClientCredentials, McpOAuthStoredCredentials,
    client::{McpClientError, McpClientFuture, McpRequestContext},
    http::{McpHttpAuthorization, McpHttpAuthorizationProvider},
    secrets::now_epoch_seconds,
};

use super::http_client::strict_oauth_http_client;
use super::protocol::{auth_error, resolved_resource, stored_credentials, unavailable_error};

impl McpHttpAuthorizationProvider for McpOAuthRegistry {
    fn authorize<'a>(
        &'a self,
        url: &'a str,
        client_credentials: Option<&'a McpOAuthClientCredentials>,
        stored_credentials: Option<&'a McpOAuthStoredCredentials>,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpHttpAuthorization> {
        Box::pin(async move {
            context.check("oauth/authorize")?;
            authorize(url, client_credentials, stored_credentials)
                .await
                .map_err(client_error)
        })
    }
}

async fn authorize(
    url: &str,
    client_credentials: Option<&McpOAuthClientCredentials>,
    stored_credentials: Option<&McpOAuthStoredCredentials>,
) -> McpOAuthResult<McpHttpAuthorization> {
    super::validate_https_or_loopback(url, "MCP endpoint")?;
    if let Some(stored) = stored_credentials {
        if let Some(token) = access_token(stored)
            && !needs_refresh(stored)
        {
            return Ok(McpHttpAuthorization::new(token.to_string(), None));
        }
        match refresh(url, stored).await {
            Ok(refreshed) => return from_stored(refreshed, true),
            Err(error) if client_credentials.is_none() => return Err(error),
            Err(_) => {}
        }
    }
    let credentials = client_credentials.ok_or_else(|| {
        McpOAuthError::new(
            McpOAuthErrorKind::Authentication,
            "OAuth credentials are missing or expired",
        )
    })?;
    from_stored(client_credentials_flow(url, credentials).await?, true)
}

async fn client_credentials_flow(
    url: &str,
    credentials: &McpOAuthClientCredentials,
) -> McpOAuthResult<McpOAuthStoredCredentials> {
    let resource = resolved_resource(url)
        .await
        .unwrap_or_else(|| url.to_string());
    let mut state = OAuthState::new_with_oauth_http_client(&resource, strict_oauth_http_client())
        .await
        .map_err(unavailable_error)?;
    state
        .authenticate_client_credentials(ClientCredentialsConfig::ClientSecret {
            client_id: credentials.client_id.clone(),
            client_secret: credentials.client_secret.clone(),
            scopes: credentials.scopes.clone(),
            resource: Some(resource),
        })
        .await
        .map_err(auth_error)?;
    stored_credentials(&state).await
}

async fn refresh(
    url: &str,
    stored: &McpOAuthStoredCredentials,
) -> McpOAuthResult<McpOAuthStoredCredentials> {
    let response: OAuthTokenResponse =
        serde_json::from_value(stored.token_response.clone()).map_err(unavailable_error)?;
    let resource = resolved_resource(url)
        .await
        .unwrap_or_else(|| url.to_string());
    let mut state = OAuthState::new_with_oauth_http_client(resource, strict_oauth_http_client())
        .await
        .map_err(unavailable_error)?;
    state
        .set_credentials(&stored.client_id, response)
        .await
        .map_err(auth_error)?;
    state.refresh_token().await.map_err(auth_error)?;
    let mut refreshed = stored_credentials(&state).await?;
    preserve_refresh_token(&mut refreshed.token_response, &stored.token_response);
    Ok(refreshed)
}

fn preserve_refresh_token(refreshed: &mut Value, previous: &Value) {
    if refreshed.get("refresh_token").is_none()
        && let Some(token) = previous.get("refresh_token")
        && let Some(object) = refreshed.as_object_mut()
    {
        object.insert("refresh_token".to_string(), token.clone());
    }
}

fn from_stored(
    credentials: McpOAuthStoredCredentials,
    refreshed: bool,
) -> McpOAuthResult<McpHttpAuthorization> {
    let token = access_token(&credentials)
        .ok_or_else(|| {
            McpOAuthError::new(
                McpOAuthErrorKind::Authentication,
                "OAuth access token is missing",
            )
        })?
        .to_string();
    Ok(McpHttpAuthorization::new(
        token,
        refreshed.then_some(credentials),
    ))
}

fn access_token(credentials: &McpOAuthStoredCredentials) -> Option<&str> {
    credentials
        .token_response
        .get("access_token")
        .and_then(Value::as_str)
}

fn needs_refresh(credentials: &McpOAuthStoredCredentials) -> bool {
    let Some(expires_in) = credentials
        .token_response
        .get("expires_in")
        .and_then(Value::as_u64)
    else {
        return false;
    };
    let Some(received) = credentials.token_received_at else {
        return credentials
            .token_response
            .get("refresh_token")
            .and_then(Value::as_str)
            .is_some_and(|token| !token.is_empty());
    };
    received.saturating_add(expires_in) <= now_epoch_seconds().saturating_add(60)
}

fn client_error(error: McpOAuthError) -> McpClientError {
    match error.kind() {
        McpOAuthErrorKind::InvalidInput => McpClientError::Malformed(error.to_string()),
        McpOAuthErrorKind::Authentication => {
            McpClientError::AuthenticationRequired(error.to_string())
        }
        McpOAuthErrorKind::Timeout => McpClientError::Timeout {
            operation: "oauth/authorize",
        },
        McpOAuthErrorKind::Cancelled => McpClientError::Cancelled {
            operation: "oauth/authorize",
        },
        McpOAuthErrorKind::NotFound
        | McpOAuthErrorKind::Expired
        | McpOAuthErrorKind::Capacity
        | McpOAuthErrorKind::Conflict
        | McpOAuthErrorKind::Unavailable => McpClientError::Unavailable(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn refreshes_inside_safety_window_and_preserves_unbounded_tokens() {
        let now = now_epoch_seconds();
        let expiring = McpOAuthStoredCredentials {
            client_id: "client".to_string(),
            token_response: json!({"access_token": "token", "expires_in": 60}),
            token_received_at: Some(now),
        };
        let unbounded = McpOAuthStoredCredentials {
            token_response: json!({"access_token": "token"}),
            token_received_at: None,
            ..expiring.clone()
        };

        assert!(needs_refresh(&expiring));
        assert!(!needs_refresh(&unbounded));
        assert_eq!(access_token(&expiring), Some("token"));
    }

    #[test]
    fn stored_oauth_credentials_refresh_legacy_expiring_token_when_refresh_token_exists() {
        let legacy = McpOAuthStoredCredentials {
            client_id: "client".to_string(),
            token_response: json!({
                "access_token": "legacy-token",
                "expires_in": 3600,
                "refresh_token": "refresh-token"
            }),
            token_received_at: None,
        };

        assert!(needs_refresh(&legacy));
    }

    #[test]
    fn omitted_refresh_token_is_preserved_after_refresh() {
        let mut refreshed = json!({"access_token": "new-token"});
        let previous = json!({"access_token": "old-token", "refresh_token": "refresh-token"});

        preserve_refresh_token(&mut refreshed, &previous);

        assert_eq!(refreshed["refresh_token"], "refresh-token");
    }
}
