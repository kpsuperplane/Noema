//! Streamable HTTP MCP session preparation.

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    sync::Arc,
};

use http::{HeaderName, HeaderValue};
use rmcp::{
    ServiceExt,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Map, Value};
use url::{Host, Url};

use crate::{
    McpDiagnosticHandle, McpOAuthClientCredentials, McpOAuthStoredCredentials, McpSecretMaterial,
    McpServerRecord, McpTransportKind,
    client::{
        McpClientError, McpClientFuture, McpClientResult, McpRequestContext, McpSessionFactory,
        McpSessionPreparation, RmcpPreparedSession, initialize_error, run_with_context,
    },
};

const TRANSPORT_KIND: &str = "streamable_http";

mod client;

/// OAuth authorization material prepared for one HTTP session.
pub struct McpHttpAuthorization {
    access_token: String,
    refreshed_oauth_credentials: Option<McpOAuthStoredCredentials>,
}

impl McpHttpAuthorization {
    /// Construct prepared bearer authorization and optional refreshed state.
    #[must_use]
    pub fn new(
        access_token: String,
        refreshed_oauth_credentials: Option<McpOAuthStoredCredentials>,
    ) -> Self {
        Self {
            access_token,
            refreshed_oauth_credentials,
        }
    }

    fn into_parts(self) -> (String, Option<McpOAuthStoredCredentials>) {
        (self.access_token, self.refreshed_oauth_credentials)
    }
}

impl fmt::Debug for McpHttpAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpHttpAuthorization")
            .field("access_token", &"[REDACTED]")
            .field(
                "refreshed_oauth_credentials",
                &self
                    .refreshed_oauth_credentials
                    .as_ref()
                    .map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// Narrow OAuth boundary used by Streamable HTTP preparation.
pub trait McpHttpAuthorizationProvider: Send + Sync + fmt::Debug {
    /// Resolve an access token and return any refreshed stored credentials.
    fn authorize<'a>(
        &'a self,
        url: &'a str,
        client_credentials: Option<&'a McpOAuthClientCredentials>,
        stored_credentials: Option<&'a McpOAuthStoredCredentials>,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpHttpAuthorization>;
}

/// Shared Streamable HTTP OAuth authorization provider.
pub type McpHttpAuthorizationHandle = Arc<dyn McpHttpAuthorizationProvider>;

/// Factory for initialized Streamable HTTP MCP sessions.
#[derive(Clone)]
pub struct StreamableHttpMcpSessionFactory {
    diagnostics: Option<McpDiagnosticHandle>,
    authorization: Option<McpHttpAuthorizationHandle>,
}

impl StreamableHttpMcpSessionFactory {
    /// Construct the HTTP factory with injectable diagnostics and OAuth.
    #[must_use]
    pub const fn new(
        diagnostics: Option<McpDiagnosticHandle>,
        authorization: Option<McpHttpAuthorizationHandle>,
    ) -> Self {
        Self {
            diagnostics,
            authorization,
        }
    }
}

impl fmt::Debug for StreamableHttpMcpSessionFactory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StreamableHttpMcpSessionFactory")
            .field("diagnostics_configured", &self.diagnostics.is_some())
            .field("authorization_configured", &self.authorization.is_some())
            .finish()
    }
}

impl McpSessionFactory for StreamableHttpMcpSessionFactory {
    fn prepare<'a>(
        &'a self,
        server: &'a McpServerRecord,
        secrets: &'a McpSecretMaterial,
        context: &'a McpRequestContext,
    ) -> McpClientFuture<'a, McpSessionPreparation> {
        Box::pin(async move {
            context.check("initialize")?;
            let config = http_config_from_server(server, secrets)?;
            let uses_oauth =
                config.oauth_client_credentials.is_some() || config.oauth_credentials.is_some();
            if uses_oauth && has_authorization_header(&config.headers) {
                return Err(McpClientError::Malformed(
                    "MCP HTTP configuration contains multiple authorization methods".to_string(),
                ));
            }

            let (access_token, refreshed_oauth_credentials) = if uses_oauth {
                let provider = self.authorization.as_ref().ok_or_else(|| {
                    McpClientError::AuthenticationRequired(
                        "OAuth authorization is not configured".to_string(),
                    )
                })?;
                let authorization = run_with_context(
                    context,
                    "oauth/authorize",
                    provider.authorize(
                        &config.url,
                        config.oauth_client_credentials.as_ref(),
                        config.oauth_credentials.as_ref(),
                        context,
                    ),
                )
                .await?;
                let (access_token, refreshed) = authorization.into_parts();
                (Some(access_token), refreshed)
            } else {
                (None, None)
            };

            let client = restricted_http_client(&config.url).await?;
            let mut transport_config = StreamableHttpClientTransportConfig::with_uri(config.url)
                .custom_headers(config.headers);
            if let Some(access_token) = access_token {
                transport_config = transport_config.auth_header(access_token);
            }
            let transport = StreamableHttpClientTransport::with_client(
                client::BoundedReqwestMcpClient::new(client),
                transport_config,
            );
            let service_cancellation = context.cancellation_token().child_token();
            let initialize = ().serve_with_ct(transport, service_cancellation.clone());
            let service = match run_with_context(context, "initialize", async {
                initialize
                    .await
                    .map_err(|error| initialize_error("initialize", error))
            })
            .await
            {
                Ok(service) => service,
                Err(error) => {
                    service_cancellation.cancel();
                    return Err(error);
                }
            };
            let session = RmcpPreparedSession::new(
                service,
                self.diagnostics.clone(),
                server.mcp_server_id.clone(),
                TRANSPORT_KIND,
            );
            Ok(McpSessionPreparation::new(
                Box::new(session),
                refreshed_oauth_credentials,
            ))
        })
    }
}

struct HttpConfig {
    url: String,
    headers: HashMap<HeaderName, HeaderValue>,
    oauth_client_credentials: Option<McpOAuthClientCredentials>,
    oauth_credentials: Option<McpOAuthStoredCredentials>,
}

fn http_config_from_server(
    server: &McpServerRecord,
    secrets: &McpSecretMaterial,
) -> McpClientResult<HttpConfig> {
    if server.transport_kind != McpTransportKind::StreamableHttp {
        return Err(McpClientError::Malformed(
            "MCP server is not configured for Streamable HTTP".to_string(),
        ));
    }
    let object = server.safe_config.as_object().ok_or_else(|| {
        McpClientError::Malformed("MCP Streamable HTTP config must be an object".to_string())
    })?;
    let url = string_field(object, "url")?;
    validate_http_url(&url)?;
    let safe_headers = string_map_field(object, "headers")?;
    let headers = merge_headers(&safe_headers, &secrets.headers)?;
    Ok(HttpConfig {
        url,
        headers,
        oauth_client_credentials: secrets.oauth_client_credentials.clone(),
        oauth_credentials: secrets.oauth_credentials.clone(),
    })
}

fn validate_http_url(value: &str) -> McpClientResult<()> {
    let url = Url::parse(value)
        .map_err(|_| McpClientError::Malformed("MCP HTTP URL is invalid".to_string()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || (url.scheme() == "http" && !url.host().is_some_and(is_loopback_host))
    {
        return Err(McpClientError::Malformed(
            "MCP HTTP URL is invalid".to_string(),
        ));
    }
    Ok(())
}

fn is_loopback_host(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

async fn restricted_http_client(value: &str) -> McpClientResult<reqwest::Client> {
    let url = Url::parse(value)
        .map_err(|_| McpClientError::Malformed("MCP HTTP URL is invalid".to_string()))?;
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0);
    if url.scheme() == "http" && matches!(url.host(), Some(Host::Domain("localhost"))) {
        let port = url
            .port_or_known_default()
            .ok_or_else(|| McpClientError::Malformed("MCP HTTP URL has no port".to_string()))?;
        let addresses = tokio::net::lookup_host(("localhost", port))
            .await
            .map_err(|_| {
                McpClientError::Unavailable("failed to resolve MCP loopback host".to_string())
            })?
            .collect::<Vec<_>>();
        if addresses.is_empty() || addresses.iter().any(|address| !address.ip().is_loopback()) {
            return Err(McpClientError::Unavailable(
                "MCP loopback host resolved outside loopback".to_string(),
            ));
        }
        builder = builder.resolve_to_addrs("localhost", &addresses);
    }
    builder.build().map_err(|error| {
        McpClientError::Unavailable(format!("failed to configure MCP HTTP client: {error}"))
    })
}

fn string_field(object: &Map<String, Value>, field: &'static str) -> McpClientResult<String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            McpClientError::Malformed(format!("MCP config field {field} must be a string"))
        })
}

fn string_map_field(
    object: &Map<String, Value>,
    field: &'static str,
) -> McpClientResult<BTreeMap<String, String>> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(BTreeMap::new()),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|value| (key.clone(), value.to_string()))
                    .ok_or_else(|| {
                        McpClientError::Malformed(format!(
                            "MCP config field {field} must contain string values"
                        ))
                    })
            })
            .collect(),
        Some(_) => Err(McpClientError::Malformed(format!(
            "MCP config field {field} must be an object"
        ))),
    }
}

fn has_authorization_header(headers: &HashMap<HeaderName, HeaderValue>) -> bool {
    headers.contains_key(&http::header::AUTHORIZATION)
}

fn merge_headers(
    safe_headers: &BTreeMap<String, String>,
    secret_headers: &BTreeMap<String, String>,
) -> McpClientResult<HashMap<HeaderName, HeaderValue>> {
    let mut headers = parse_headers(safe_headers, false)?;
    headers.extend(parse_headers(secret_headers, true)?);
    Ok(headers)
}

fn parse_headers(
    headers: &BTreeMap<String, String>,
    sensitive: bool,
) -> McpClientResult<HashMap<HeaderName, HeaderValue>> {
    headers
        .iter()
        .map(|(key, value)| {
            let name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
                McpClientError::Malformed(format!("invalid MCP HTTP header name {key}: {error}"))
            })?;
            let mut value = HeaderValue::from_str(value).map_err(|error| {
                McpClientError::Malformed(format!(
                    "invalid MCP HTTP header value for {key}: {error}"
                ))
            })?;
            value.set_sensitive(sensitive);
            Ok((name, value))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, time::Duration};

    use serde_json::json;

    use super::*;
    use crate::{McpServerAuthStatus, McpServerHealthStatus};

    fn server() -> McpServerRecord {
        McpServerRecord {
            mcp_server_id: "mcp:remote".to_string(),
            display_name: "Remote".to_string(),
            transport_kind: McpTransportKind::StreamableHttp,
            safe_config: json!({
                "url": "https://example.com/mcp",
                "headers": { "X-Team": "infra", "X-Override": "safe" }
            }),
            enabled: true,
            health_status: McpServerHealthStatus::Unknown,
            auth_status: McpServerAuthStatus::None,
            tool_count: 0,
            authority_generation: "generation".to_string(),
        }
    }

    #[test]
    fn config_merges_safe_and_secret_headers_with_secret_precedence() {
        let secrets = McpSecretMaterial {
            headers: BTreeMap::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("X-Override".to_string(), "secret".to_string()),
            ]),
            ..McpSecretMaterial::default()
        };

        let config = http_config_from_server(&server(), &secrets).expect("config");

        assert_eq!(config.url, "https://example.com/mcp");
        assert_eq!(config.headers[&HeaderName::from_static("x-team")], "infra");
        assert_eq!(
            config.headers[&HeaderName::from_static("x-override")],
            "secret"
        );
        assert_eq!(
            config.headers[&http::header::AUTHORIZATION],
            "Bearer secret"
        );
        assert!(config.headers[&http::header::AUTHORIZATION].is_sensitive());
        assert!(config.headers[&HeaderName::from_static("x-override")].is_sensitive());
        assert!(!config.headers[&HeaderName::from_static("x-team")].is_sensitive());
    }

    #[test]
    fn secret_headers_override_safe_headers_case_insensitively() {
        let mut server = server();
        server.safe_config["headers"] = json!({
            "authorization": "Bearer safe",
            "X-TEAM": "safe"
        });
        let secrets = McpSecretMaterial {
            headers: BTreeMap::from([
                ("Authorization".to_string(), "Bearer secret".to_string()),
                ("x-team".to_string(), "secret".to_string()),
            ]),
            ..McpSecretMaterial::default()
        };

        let config = http_config_from_server(&server, &secrets).expect("config");

        assert_eq!(config.headers.len(), 2);
        assert_eq!(
            config.headers[&http::header::AUTHORIZATION],
            "Bearer secret"
        );
        assert_eq!(config.headers[&HeaderName::from_static("x-team")], "secret");
    }

    #[test]
    fn authorization_debug_redacts_access_and_refresh_tokens() {
        let authorization = McpHttpAuthorization::new(
            "access-secret".to_string(),
            Some(McpOAuthStoredCredentials {
                client_id: "client-secret".to_string(),
                token_response: json!({"refresh_token": "refresh-secret"}),
                token_received_at: Some(42),
            }),
        );

        let debug = format!("{authorization:?}");

        for secret in ["access-secret", "client-secret", "refresh-secret"] {
            assert!(!debug.contains(secret), "debug output leaked {secret:?}");
        }
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn url_validation_rejects_embedded_credentials_and_fragments() {
        for url in [
            "https://user:password@example.com/mcp",
            "https://example.com/mcp#fragment",
            "http://example.com/mcp",
            "file:///tmp/mcp",
        ] {
            let error = validate_http_url(url).expect_err("invalid URL");
            assert!(matches!(error, McpClientError::Malformed(_)));
        }
        validate_http_url("http://127.0.0.1:8080/mcp").expect("IPv4 loopback");
        validate_http_url("http://[::1]:8080/mcp").expect("IPv6 loopback");
        validate_http_url("http://localhost:8080/mcp").expect("localhost");
    }

    #[test]
    fn oauth_and_authorization_header_cannot_be_combined() {
        let headers = merge_headers(
            &BTreeMap::new(),
            &BTreeMap::from([("AUTHORIZATION".to_string(), "Bearer secret".to_string())]),
        )
        .expect("headers");
        assert!(has_authorization_header(&headers));
    }

    #[tokio::test]
    async fn restricted_client_never_follows_redirects_with_secret_headers() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };

        let redirect_target = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("redirect target");
        let target_address = redirect_target.local_addr().expect("target address");
        let origin = TcpListener::bind("127.0.0.1:0").await.expect("origin");
        let origin_address = origin.local_addr().expect("origin address");
        let origin_task = tokio::spawn(async move {
            let (mut stream, _) = origin.accept().await.expect("origin request");
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).await.expect("read request");
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{target_address}/leak\r\nContent-Length: 0\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .expect("write redirect");
        });

        let url = format!("http://{origin_address}/mcp");
        let response = restricted_http_client(&url)
            .await
            .expect("client")
            .get(url)
            .header("X-API-Key", "private-secret")
            .send()
            .await
            .expect("response");
        assert_eq!(response.status(), reqwest::StatusCode::TEMPORARY_REDIRECT);
        origin_task.await.expect("origin task");
        assert!(
            tokio::time::timeout(Duration::from_millis(50), redirect_target.accept())
                .await
                .is_err(),
            "redirect target received a request"
        );
    }
}
