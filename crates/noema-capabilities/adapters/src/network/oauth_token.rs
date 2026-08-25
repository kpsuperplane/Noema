//! OAuth authorization-code token exchange over the adapter network policy.

use super::{AdapterHttpError, MAX_RESPONSE_HEADER_BYTES, checked_client};
use crate::{
    Oauth2ClientAuthentication, OauthScopeResponsePolicy,
    json_limits::{parse_without_duplicate_keys, validate_json_shape},
};
use noema_capabilities::web::url_policy::validate_public_url;
use oauth2::{
    AuthType, AuthorizationCode, ClientId, ClientSecret, PkceCodeVerifier, RedirectUrl,
    RefreshToken, TokenResponse, TokenUrl,
    basic::{BasicClient, BasicTokenResponse, BasicTokenType},
};
use reqwest::header;
use std::{future::Future, pin::Pin};
use thiserror::Error;
use url::Url;

const MAX_TOKEN_BYTES: usize = 128 * 1024;
const MAX_SECRET_BYTES: usize = 16 * 1024;

pub(crate) type AdapterOAuthTokenFuture<'a> = Pin<
    Box<dyn Future<Output = Result<AdapterOAuthTokenOutcome, AdapterOAuthTokenError>> + Send + 'a>,
>;

/// One transient authorization-code token request. Secret-bearing fields never
/// implement `Debug` and are dropped after the exchange.
pub(crate) struct AdapterOAuthTokenRequest {
    pub(crate) token_endpoint: Url,
    pub(crate) client_authentication: Oauth2ClientAuthentication,
    pub(crate) client_id: String,
    pub(crate) client_secret: Option<String>,
    pub(crate) grant: AdapterOAuthTokenGrant,
    pub(crate) expected_scopes: Vec<String>,
    pub(crate) omitted_scope_policy: OauthScopeResponsePolicy,
    pub(crate) now_epoch_seconds: u64,
}

/// Closed OAuth grants supported by the generic connector token transport.
pub(crate) enum AdapterOAuthTokenGrant {
    AuthorizationCode {
        code: String,
        redirect_uri: String,
        pkce_verifier: String,
    },
    RefreshToken {
        refresh_token: String,
    },
}

/// Validated bearer-token material returned by a reviewed OAuth endpoint.
pub(crate) struct AdapterOAuthTokenOutcome {
    pub(crate) access_token: String,
    pub(crate) refresh_token: Option<String>,
    pub(crate) expires_at_epoch_seconds: Option<u64>,
    pub(crate) granted_scopes: Vec<String>,
}

impl std::fmt::Debug for AdapterOAuthTokenOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdapterOAuthTokenOutcome")
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .field("granted_scopes", &self.granted_scopes)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum AdapterOAuthTokenError {
    #[error("adapter OAuth token request is invalid")]
    InvalidRequest,
    #[error("adapter OAuth token endpoint is unavailable")]
    Unavailable,
    #[error("adapter OAuth token request was rejected")]
    Rejected,
    #[error("adapter OAuth token response is invalid")]
    InvalidResponse,
}

pub(super) async fn exchange(
    request: AdapterOAuthTokenRequest,
) -> Result<AdapterOAuthTokenOutcome, AdapterOAuthTokenError> {
    exchange_with_client(request, &HardenedOAuthHttpClient).await
}

struct HardenedOAuthHttpClient;

impl<'client> oauth2::AsyncHttpClient<'client> for HardenedOAuthHttpClient {
    type Error = AdapterHttpError;
    type Future = Pin<
        Box<dyn Future<Output = Result<oauth2::HttpResponse, AdapterHttpError>> + Send + 'client>,
    >;

    fn call(&'client self, request: oauth2::HttpRequest) -> Self::Future {
        Box::pin(async move { send_request(request).await })
    }
}

async fn exchange_with_client<Client>(
    request: AdapterOAuthTokenRequest,
    http_client: &Client,
) -> Result<AdapterOAuthTokenOutcome, AdapterOAuthTokenError>
where
    for<'client> Client: oauth2::AsyncHttpClient<'client>,
{
    validate_request(&request)?;
    let mut client = BasicClient::new(ClientId::new(request.client_id.clone()));
    let auth_type = match request.client_authentication {
        Oauth2ClientAuthentication::None => AuthType::RequestBody,
        Oauth2ClientAuthentication::ClientSecretBasic => AuthType::BasicAuth,
        Oauth2ClientAuthentication::ClientSecretPost => AuthType::RequestBody,
    };
    if request.client_authentication != Oauth2ClientAuthentication::None {
        client = client.set_client_secret(ClientSecret::new(
            request
                .client_secret
                .clone()
                .ok_or(AdapterOAuthTokenError::InvalidRequest)?,
        ));
    }
    let client = client
        .set_token_uri(
            TokenUrl::new(request.token_endpoint.as_str().to_string())
                .map_err(|_| AdapterOAuthTokenError::InvalidRequest)?,
        )
        .set_auth_type(auth_type);
    let is_refresh = matches!(&request.grant, AdapterOAuthTokenGrant::RefreshToken { .. });
    let response: BasicTokenResponse = match request.grant {
        AdapterOAuthTokenGrant::AuthorizationCode {
            code,
            redirect_uri,
            pkce_verifier,
        } => client
            .set_redirect_uri(
                RedirectUrl::new(redirect_uri)
                    .map_err(|_| AdapterOAuthTokenError::InvalidRequest)?,
            )
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
            .request_async(http_client)
            .await
            .map_err(map_request_error)?,
        AdapterOAuthTokenGrant::RefreshToken { refresh_token } => client
            .exchange_refresh_token(&RefreshToken::new(refresh_token))
            .request_async(http_client)
            .await
            .map_err(map_request_error)?,
    };
    if response.token_type() != &BasicTokenType::Bearer {
        return Err(AdapterOAuthTokenError::InvalidResponse);
    }
    let access_token = response.access_token().secret().to_string();
    let refresh_token = response
        .refresh_token()
        .map(|token| token.secret().to_string());
    if !valid_secret(&access_token)
        || refresh_token
            .as_deref()
            .is_some_and(|token| !valid_secret(token))
    {
        return Err(AdapterOAuthTokenError::InvalidResponse);
    }
    let requested = request
        .expected_scopes
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut granted_scopes = match response.scopes() {
        Some(scopes) => scopes
            .iter()
            .map(|scope| scope.as_ref().to_string())
            .collect::<Vec<_>>(),
        None if request.omitted_scope_policy == OauthScopeResponsePolicy::RequestedScopes => {
            request.expected_scopes
        }
        None => return Err(AdapterOAuthTokenError::InvalidResponse),
    };
    if granted_scopes.iter().any(|scope| !valid_scope(scope)) {
        return Err(AdapterOAuthTokenError::InvalidResponse);
    }
    granted_scopes.sort();
    let granted = granted_scopes
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    // The authorization server owns the reported grant. Operations check that grant before use.
    if granted_scopes.windows(2).any(|pair| pair[0] == pair[1])
        || is_refresh && !granted.is_subset(&requested)
    {
        return Err(AdapterOAuthTokenError::InvalidResponse);
    }
    let expires_at_epoch_seconds = response
        .expires_in()
        .map(|duration| {
            if duration.is_zero() {
                return Err(AdapterOAuthTokenError::InvalidResponse);
            }
            request
                .now_epoch_seconds
                .checked_add(duration.as_secs())
                .ok_or(AdapterOAuthTokenError::InvalidResponse)
        })
        .transpose()?;
    Ok(AdapterOAuthTokenOutcome {
        access_token,
        refresh_token,
        expires_at_epoch_seconds,
        granted_scopes,
    })
}

fn map_request_error<RE: std::error::Error + 'static>(
    error: oauth2::RequestTokenError<
        RE,
        oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    >,
) -> AdapterOAuthTokenError {
    match error {
        oauth2::RequestTokenError::Request(_) => AdapterOAuthTokenError::Unavailable,
        oauth2::RequestTokenError::ServerResponse(_) => AdapterOAuthTokenError::Rejected,
        oauth2::RequestTokenError::Parse(_, _) | oauth2::RequestTokenError::Other(_) => {
            AdapterOAuthTokenError::InvalidResponse
        }
    }
}

fn validate_request(request: &AdapterOAuthTokenRequest) -> Result<(), AdapterOAuthTokenError> {
    if validate_public_url(request.token_endpoint.as_str()).is_err()
        || request.token_endpoint.scheme() != "https"
        || request.token_endpoint.query().is_some()
        || request.token_endpoint.fragment().is_some()
        || !valid_secret(&request.client_id)
        || request.expected_scopes.len() > 256
        || request
            .expected_scopes
            .iter()
            .any(|scope| !valid_scope(scope))
        || request
            .expected_scopes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || (request.client_authentication != Oauth2ClientAuthentication::None
            && request
                .client_secret
                .as_deref()
                .is_none_or(|secret| !valid_secret(secret)))
    {
        return Err(AdapterOAuthTokenError::InvalidRequest);
    }
    match &request.grant {
        AdapterOAuthTokenGrant::AuthorizationCode {
            code,
            redirect_uri,
            pkce_verifier,
        } if valid_secret(code)
            && valid_secret(pkce_verifier)
            && redirect_uri.len() <= 8 * 1024
            && !redirect_uri.bytes().any(|byte| byte.is_ascii_control()) => {}
        AdapterOAuthTokenGrant::RefreshToken { refresh_token } if valid_secret(refresh_token) => {}
        _ => return Err(AdapterOAuthTokenError::InvalidRequest),
    }
    Ok(())
}

async fn send_request(
    request: oauth2::HttpRequest,
) -> Result<oauth2::HttpResponse, AdapterHttpError> {
    if request.method() != oauth2::http::Method::POST
        || request.body().len() > MAX_TOKEN_BYTES
        || request
            .headers()
            .iter()
            .map(|(name, value)| name.as_str().len() + value.as_bytes().len())
            .sum::<usize>()
            > MAX_RESPONSE_HEADER_BYTES
    {
        return Err(AdapterHttpError::InvalidResponse);
    }
    let url = Url::parse(&request.uri().to_string()).map_err(|_| AdapterHttpError::Unavailable)?;
    let client = checked_client(&url).await?;
    let mut builder = client.post(url).header(header::ACCEPT_ENCODING, "identity");
    for (name, value) in request.headers() {
        let mut value = reqwest::header::HeaderValue::from_bytes(value.as_bytes())
            .map_err(|_| AdapterHttpError::InvalidResponse)?;
        if name == oauth2::http::header::AUTHORIZATION {
            value.set_sensitive(true);
        }
        builder = builder.header(name.as_str(), value);
    }
    let response = builder
        .body(request.body().clone())
        .send()
        .await
        .map_err(|_| AdapterHttpError::Unavailable)?;
    if response
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.as_bytes().len())
        .sum::<usize>()
        > MAX_RESPONSE_HEADER_BYTES
        || response
            .headers()
            .get(header::CONTENT_ENCODING)
            .is_some_and(|value| value.as_bytes() != b"identity")
    {
        return Err(AdapterHttpError::InvalidResponse);
    }
    let status = response.status().as_u16();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let body = bounded_body(response).await?;
    validate_response_json(&body)?;
    let mut response = oauth2::http::Response::builder().status(status);
    if let Some(content_type) = content_type {
        response = response.header(oauth2::http::header::CONTENT_TYPE, content_type.as_bytes());
    }
    response
        .body(body)
        .map_err(|_| AdapterHttpError::InvalidResponse)
}

fn validate_response_json(body: &[u8]) -> Result<(), AdapterHttpError> {
    let value =
        parse_without_duplicate_keys(body).map_err(|_| AdapterHttpError::InvalidResponse)?;
    validate_json_shape(&value)
        .then_some(())
        .ok_or(AdapterHttpError::InvalidResponse)
}

async fn bounded_body(mut response: reqwest::Response) -> Result<Vec<u8>, AdapterHttpError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_TOKEN_BYTES as u64)
    {
        return Err(AdapterHttpError::InvalidResponse);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AdapterHttpError::OutcomeUncertain)?
    {
        if chunk.len() > MAX_TOKEN_BYTES.saturating_sub(body.len()) {
            return Err(AdapterHttpError::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn valid_secret(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SECRET_BYTES
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

fn valid_scope(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, future, sync::Mutex};

    struct RecordingOAuthClient {
        response_body: Vec<u8>,
        recorded: Mutex<Option<(Option<String>, String)>>,
    }

    impl RecordingOAuthClient {
        fn new(response_body: &str) -> Self {
            Self {
                response_body: response_body.as_bytes().to_vec(),
                recorded: Mutex::new(None),
            }
        }
    }

    impl<'client> oauth2::AsyncHttpClient<'client> for RecordingOAuthClient {
        type Error = AdapterHttpError;
        type Future = future::Ready<Result<oauth2::HttpResponse, AdapterHttpError>>;

        fn call(&'client self, request: oauth2::HttpRequest) -> Self::Future {
            let authorization = request
                .headers()
                .get(oauth2::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let body = String::from_utf8(request.body().clone()).expect("form body");
            *self.recorded.lock().expect("recorded") = Some((authorization, body));
            future::ready(Ok(oauth2::http::Response::builder()
                .status(200)
                .header(oauth2::http::header::CONTENT_TYPE, "application/json")
                .body(self.response_body.clone())
                .expect("response")))
        }
    }

    fn token_request(
        client_authentication: Oauth2ClientAuthentication,
    ) -> AdapterOAuthTokenRequest {
        AdapterOAuthTokenRequest {
            token_endpoint: Url::parse("https://auth.example.com/token").expect("endpoint"),
            client_authentication,
            client_id: "client-marker".to_string(),
            client_secret: Some("secret-marker".to_string()),
            grant: AdapterOAuthTokenGrant::AuthorizationCode {
                code: "code-marker".to_string(),
                redirect_uri: "http://127.0.0.1:43123/adapter/oauth/callback".to_string(),
                pkce_verifier: "verifier-marker".to_string(),
            },
            expected_scopes: vec!["read".to_string(), "write".to_string()],
            omitted_scope_policy: OauthScopeResponsePolicy::RequestedScopes,
            now_epoch_seconds: 1_000,
        }
    }

    #[tokio::test]
    async fn uses_reviewed_client_auth_and_validates_tokens() {
        let response = r#"{"access_token":"access-marker","token_type":"Bearer","refresh_token":"refresh-marker","expires_in":3600,"scope":"read write"}"#;
        for mode in [
            Oauth2ClientAuthentication::None,
            Oauth2ClientAuthentication::ClientSecretBasic,
            Oauth2ClientAuthentication::ClientSecretPost,
        ] {
            let client = RecordingOAuthClient::new(response);
            let token = exchange_with_client(token_request(mode), &client)
                .await
                .expect("exchange");
            assert_eq!(token.expires_at_epoch_seconds, Some(4_600));
            assert_eq!(token.granted_scopes, ["read", "write"]);
            assert!(!format!("{token:?}").contains("access-marker"));
            let (authorization, body) = client
                .recorded
                .lock()
                .expect("recorded")
                .take()
                .expect("request");
            let form = url::form_urlencoded::parse(body.as_bytes())
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(form.get("code").map(String::as_str), Some("code-marker"));
            assert_eq!(
                form.get("code_verifier").map(String::as_str),
                Some("verifier-marker")
            );
            match mode {
                Oauth2ClientAuthentication::None => {
                    assert!(authorization.is_none());
                    assert_eq!(
                        form.get("client_id").map(String::as_str),
                        Some("client-marker")
                    );
                    assert!(!form.contains_key("client_secret"));
                }
                Oauth2ClientAuthentication::ClientSecretBasic => {
                    assert!(
                        authorization
                            .as_deref()
                            .is_some_and(|value| value.starts_with("Basic "))
                    );
                    assert!(!form.contains_key("client_id"));
                    assert!(!form.contains_key("client_secret"));
                }
                Oauth2ClientAuthentication::ClientSecretPost => {
                    assert!(authorization.is_none());
                    assert_eq!(
                        form.get("client_secret").map(String::as_str),
                        Some("secret-marker")
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn rejects_ambiguous_json_and_scope_or_expiry_drift() {
        assert_eq!(
            validate_response_json(
                br#"{"access_token":"one","access_token":"two","token_type":"Bearer"}"#,
            ),
            Err(AdapterHttpError::InvalidResponse)
        );
        for response in [
            r#"{"access_token":"access","token_type":"Bearer","scope":"read read"}"#,
            r#"{"access_token":"access","token_type":"Bearer","expires_in":0,"scope":"read write"}"#,
            r#"{"access_token":"access","token_type":"MAC","scope":"read write"}"#,
        ] {
            let client = RecordingOAuthClient::new(response);
            assert_eq!(
                exchange_with_client(
                    token_request(Oauth2ClientAuthentication::ClientSecretPost),
                    &client,
                )
                .await
                .expect_err("invalid token"),
                AdapterOAuthTokenError::InvalidResponse
            );
        }
    }

    #[tokio::test]
    async fn authorization_code_preserves_provider_reported_scopes() {
        let client = RecordingOAuthClient::new(
            r#"{"access_token":"access","token_type":"Bearer","scope":"calendar read write"}"#,
        );
        let token = exchange_with_client(
            token_request(Oauth2ClientAuthentication::ClientSecretPost),
            &client,
        )
        .await
        .expect("combined grant");

        assert_eq!(token.granted_scopes, ["calendar", "read", "write"]);

        let client = RecordingOAuthClient::new(
            r#"{"access_token":"access","token_type":"Bearer","scope":"read"}"#,
        );
        let token = exchange_with_client(
            token_request(Oauth2ClientAuthentication::ClientSecretPost),
            &client,
        )
        .await
        .expect("partial grant");
        assert_eq!(token.granted_scopes, ["read"]);
    }

    #[tokio::test]
    async fn refresh_uses_the_reviewed_client_auth_without_requesting_new_scopes() {
        let client = RecordingOAuthClient::new(
            r#"{"access_token":"fresh-access","token_type":"Bearer","expires_in":3600,"scope":"read"}"#,
        );
        let mut request = token_request(Oauth2ClientAuthentication::ClientSecretPost);
        request.grant = AdapterOAuthTokenGrant::RefreshToken {
            refresh_token: "refresh-marker".to_string(),
        };
        let token = exchange_with_client(request, &client)
            .await
            .expect("refresh exchange");
        assert_eq!(token.granted_scopes, ["read"]);
        let (_, body) = client
            .recorded
            .lock()
            .expect("recorded")
            .take()
            .expect("request");
        assert!(body.contains("grant_type=refresh_token"));
        assert!(body.contains("refresh_token=refresh-marker"));
        assert!(body.contains("client_secret=secret-marker"));
        assert!(!body.contains("scope="));

        let client = RecordingOAuthClient::new(
            r#"{"access_token":"fresh-access","token_type":"Bearer","scope":"read unknown"}"#,
        );
        let mut request = token_request(Oauth2ClientAuthentication::ClientSecretPost);
        request.grant = AdapterOAuthTokenGrant::RefreshToken {
            refresh_token: "refresh-marker".to_string(),
        };
        assert_eq!(
            exchange_with_client(request, &client)
                .await
                .expect_err("unexpected scope expansion"),
            AdapterOAuthTokenError::InvalidResponse
        );
    }

    #[tokio::test]
    async fn omitted_scope_requires_the_reviewed_profile_rule() {
        let client =
            RecordingOAuthClient::new(r#"{"access_token":"access","token_type":"Bearer"}"#);
        let mut request = token_request(Oauth2ClientAuthentication::ClientSecretPost);
        request.omitted_scope_policy = OauthScopeResponsePolicy::RequireScope;

        assert_eq!(
            exchange_with_client(request, &client)
                .await
                .expect_err("missing scope"),
            AdapterOAuthTokenError::InvalidResponse
        );
    }
}
