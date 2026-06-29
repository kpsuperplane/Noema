//! Local web UI server for the Noema daemon.

use std::{borrow::Cow, collections::HashMap, future::Future, pin::Pin, time::Duration};

use base64::{Engine as _, engine::general_purpose};
use futures_util::StreamExt;
use ring::digest::{SHA1_FOR_LEGACY_USE_ONLY, digest};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::{
    NoemaStore, TurnActivityStatus, TurnTranscriptItem, WebConfig,
    provider_auth::{
        CodexDeviceAuthRequest, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
        ProviderAuthManager,
    },
    providers::codex_oauth::{CodexOAuthConfig, CodexTokenStore},
    {ConversationItemKind, ConversationItemRecord, ReplayMode},
};

use super::{protocol::DaemonError, runtime::CodexRuntimeHandle};

const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
const MAX_API_BODY_BYTES: usize = 64 * 1024;
const MAX_WS_FRAME_BYTES: usize = 1024 * 1024;
const HTTP_BODY_READ_TIMEOUT: Duration = Duration::from_millis(250);
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const GRAPHQL_WS_PROTOCOL: &str = "graphql-transport-ws";
const PROVIDER_AUTH_TERMINAL_PERSIST_INTERVAL: Duration = Duration::from_millis(250);

/// State shared by local web UI connections.
#[derive(Clone)]
pub(crate) struct WebState {
    runtime: CodexRuntimeHandle,
    store: NoemaStore,
    provider_auth: ProviderAuthManager,
    paths: crate::NoemaPaths,
    subscriptions: crate::graphql::ConversationSubscriptionRegistry,
}

impl WebState {
    /// Build shared web UI state.
    #[must_use]
    pub(super) fn new(
        runtime: CodexRuntimeHandle,
        store: NoemaStore,
        provider_auth: ProviderAuthManager,
        paths: crate::NoemaPaths,
    ) -> Self {
        Self {
            runtime,
            store,
            provider_auth,
            paths,
            subscriptions: crate::graphql::ConversationSubscriptionRegistry::default(),
        }
    }

    pub(crate) fn runtime(&self) -> &CodexRuntimeHandle {
        &self.runtime
    }

    pub(crate) fn store(&self) -> &NoemaStore {
        &self.store
    }

    pub(crate) fn provider_auth(&self) -> &ProviderAuthManager {
        &self.provider_auth
    }

    pub(crate) fn paths(&self) -> &crate::NoemaPaths {
        &self.paths
    }

    pub(crate) fn subscriptions(&self) -> &crate::graphql::ConversationSubscriptionRegistry {
        &self.subscriptions
    }
}

/// Bind the local web UI listener.
pub(super) async fn bind_listener(config: &WebConfig) -> Result<TcpListener, DaemonError> {
    TcpListener::bind((config.host.as_str(), config.port))
        .await
        .map_err(|source| {
            DaemonError::Protocol(format!(
                "failed to bind web UI at {}:{}: {source}",
                config.host, config.port
            ))
        })
}

/// Handle one local web UI connection.
pub(super) async fn handle_connection(
    mut stream: TcpStream,
    state: WebState,
) -> Result<(), DaemonError> {
    let request = match HttpRequest::read_from(&mut stream).await {
        Ok(request) => request,
        Err(error) => {
            write_json_error(&mut stream, error.status(), error.message()).await?;
            return Ok(());
        }
    };

    if is_graphql_ws_route(&request.method, &request.path) {
        upgrade_graphql_websocket(stream, &request, state).await?;
        return Ok(());
    }

    if is_graphql_schema_route(&request.method, &request.path) {
        let schema =
            crate::graphql::build_schema(crate::graphql::GraphqlState::from_web_state(state));
        write_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            schema.sdl().as_bytes(),
        )
        .await?;
        return Ok(());
    }

    if is_graphql_http_route(&request.method, &request.path) {
        handle_graphql_http(&mut stream, state, &request).await?;
        return Ok(());
    }

    if request.method == "GET"
        && let Some(asset) = embedded_asset(&request.path)
    {
        write_response(
            &mut stream,
            "200 OK",
            asset.content_type,
            asset.body.as_ref(),
        )
        .await?;
        return Ok(());
    }

    write_response(
        &mut stream,
        "404 Not Found",
        "text/plain; charset=utf-8",
        b"not found",
    )
    .await?;
    Ok(())
}

fn is_graphql_http_route(method: &str, path: &str) -> bool {
    method == "POST" && path == "/graphql"
}

fn is_graphql_schema_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/graphql/schema.graphql"
}

fn is_graphql_ws_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/graphql/ws"
}

#[cfg(test)]
fn is_supported_product_route(method: &str, path: &str) -> bool {
    is_graphql_http_route(method, path)
        || is_graphql_schema_route(method, path)
        || is_graphql_ws_route(method, path)
}

async fn handle_graphql_http(
    stream: &mut TcpStream,
    state: WebState,
    request: &HttpRequest,
) -> Result<(), DaemonError> {
    if let Err(error) = validate_json_post_request(request) {
        write_json_error(stream, error.status(), error.message()).await?;
        return Ok(());
    }

    let graphql_request = match serde_json::from_slice::<async_graphql::Request>(&request.body) {
        Ok(request) => request,
        Err(_) => {
            write_json_error(stream, "400 Bad Request", "invalid GraphQL request").await?;
            return Ok(());
        }
    };
    let schema = crate::graphql::build_schema(crate::graphql::GraphqlState::from_web_state(state));
    let response = schema.execute(graphql_request).await;
    write_json(stream, "200 OK", &response).await
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl HttpRequest {
    async fn read_from(stream: &mut TcpStream) -> Result<Self, HttpRequestError> {
        let mut bytes = Vec::with_capacity(1024);
        let mut buffer = [0_u8; 1024];

        loop {
            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                return Err(HttpRequestError::bad_request("missing web request"));
            }
            bytes.extend_from_slice(&buffer[..read]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            if bytes.len() > MAX_HTTP_HEADER_BYTES {
                return Err(HttpRequestError::payload_too_large(
                    "request headers too large",
                ));
            }
        }

        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| HttpRequestError::bad_request("missing web request headers"))?;
        let body_start = header_end + 4;
        let text = std::str::from_utf8(&bytes[..header_end])
            .map_err(|_| HttpRequestError::bad_request("invalid request headers"))?;
        let mut lines = text.lines();
        let request_line = lines
            .next()
            .ok_or_else(|| HttpRequestError::bad_request("missing web request line"))?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .ok_or_else(|| HttpRequestError::bad_request("missing web request method"))?;
        let path = request_parts
            .next()
            .ok_or_else(|| HttpRequestError::bad_request("missing web request path"))?;

        let mut headers = HashMap::new();
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            if name == "content-length" && headers.contains_key(&name) {
                return Err(HttpRequestError::bad_request("duplicate content length"));
            }
            headers.insert(name, value.trim().to_string());
        }

        let content_length = content_length(&headers)?;
        let mut body = bytes[body_start..].to_vec();
        if content_length > MAX_API_BODY_BYTES {
            return Err(HttpRequestError::payload_too_large(
                "request body too large",
            ));
        }
        if body.len() > content_length {
            return Err(HttpRequestError::bad_request("unexpected request bytes"));
        }
        while body.len() < content_length {
            let read = tokio::time::timeout(HTTP_BODY_READ_TIMEOUT, stream.read(&mut buffer))
                .await
                .map_err(|_| HttpRequestError::bad_request("request body timed out"))??;
            if read == 0 {
                return Err(HttpRequestError::bad_request("incomplete request body"));
            }
            body.extend_from_slice(&buffer[..read]);
            if body.len() > content_length {
                return Err(HttpRequestError::bad_request("unexpected request bytes"));
            }
        }

        Ok(Self {
            method: method.to_string(),
            path: normalized_path(path),
            headers,
            body,
        })
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Debug)]
struct HttpRequestError {
    status: &'static str,
    message: &'static str,
}

impl HttpRequestError {
    const fn bad_request(message: &'static str) -> Self {
        Self {
            status: "400 Bad Request",
            message,
        }
    }

    const fn payload_too_large(message: &'static str) -> Self {
        Self {
            status: "413 Payload Too Large",
            message,
        }
    }

    const fn status(&self) -> &'static str {
        self.status
    }

    const fn message(&self) -> &'static str {
        self.message
    }
}

impl From<std::io::Error> for HttpRequestError {
    fn from(_source: std::io::Error) -> Self {
        Self::bad_request("invalid request")
    }
}

#[derive(Debug)]
pub(crate) struct WebApiError {
    message: String,
}

impl WebApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl From<HttpRequestError> for WebApiError {
    fn from(error: HttpRequestError) -> Self {
        Self {
            message: error.message.to_string(),
        }
    }
}

fn content_length(headers: &HashMap<String, String>) -> Result<usize, HttpRequestError> {
    let Some(value) = headers.get("content-length") else {
        return Ok(0);
    };
    value
        .parse::<usize>()
        .map_err(|_| HttpRequestError::bad_request("invalid content length"))
}

fn normalized_path(path: &str) -> String {
    path.split_once('?')
        .map_or(path, |(path, _query)| path)
        .to_string()
}

struct EmbeddedAsset {
    content_type: &'static str,
    body: Cow<'static, [u8]>,
}

fn embedded_asset(path: &str) -> Option<EmbeddedAsset> {
    let (content_type, name) = match path {
        "/assets/app.js" => ("application/javascript; charset=utf-8", "app.js"),
        "/assets/styles.css" => ("text/css; charset=utf-8", "styles.css"),
        "/assets/noema-mark.svg" => ("image/svg+xml; charset=utf-8", "noema-mark.svg"),
        path if is_spa_entry_path(path) => ("text/html; charset=utf-8", "index.html"),
        _ => return None,
    };

    Some(EmbeddedAsset {
        content_type,
        body: asset_body(name)?,
    })
}

fn is_spa_entry_path(path: &str) -> bool {
    if !path.starts_with('/') {
        return false;
    }

    if path == "/assets"
        || path.starts_with("/assets/")
        || path == "/api"
        || path.starts_with("/api/")
        || path == "/graphql"
        || path.starts_with("/graphql/")
    {
        return false;
    }

    path.rsplit('/')
        .next()
        .is_some_and(|segment| !segment.contains('.'))
}

/// Resolve a web asset's bytes for release builds: embed them into the binary.
#[cfg(not(debug_assertions))]
fn asset_body(name: &str) -> Option<Cow<'static, [u8]>> {
    let body: &'static [u8] = match name {
        "index.html" => include_bytes!("assets/index.html"),
        "app.js" => include_bytes!("assets/app.js"),
        "styles.css" => include_bytes!("assets/styles.css"),
        "noema-mark.svg" => include_bytes!("assets/noema-mark.svg"),
        _ => return None,
    };
    Some(Cow::Borrowed(body))
}

/// Resolve a web asset's bytes for debug builds: read them from disk at runtime.
///
/// Unlike `include_*!`, `env!` does not register the asset files as build
/// inputs, so the dev daemon can serve freshly rebuilt web assets (e.g. from a
/// running `vite build --watch`) without forcing a recompile of this crate.
#[cfg(debug_assertions)]
fn asset_body(name: &str) -> Option<Cow<'static, [u8]>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/daemon/web/assets");
    std::fs::read(dir.join(name)).ok().map(Cow::Owned)
}

async fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), DaemonError> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}

async fn write_json<T: Serialize>(
    stream: &mut TcpStream,
    status: &str,
    value: &T,
) -> Result<(), DaemonError> {
    let body =
        serde_json::to_vec(value).map_err(|source| DaemonError::Protocol(source.to_string()))?;
    write_response(stream, status, "application/json; charset=utf-8", &body).await
}

async fn write_json_error(
    stream: &mut TcpStream,
    status: &str,
    message: &str,
) -> Result<(), DaemonError> {
    write_json(stream, status, &serde_json::json!({ "error": message })).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderAuthStartRequest {
    pub(crate) provider_kind: String,
    pub(crate) provider_account_id: String,
    pub(crate) method: crate::ProviderAuthMethod,
}

pub(crate) async fn start_provider_auth_attempt_view(
    state: &WebState,
    body: ProviderAuthStartRequest,
) -> Result<ProviderAuthAttemptView, WebApiError> {
    if body.provider_kind != "codex" {
        return Err(WebApiError::bad_request("unsupported provider"));
    }

    if body.method != crate::ProviderAuthMethod::OauthDeviceCode {
        return Err(WebApiError::bad_request("unsupported provider auth method"));
    }

    let Some(account) = state
        .store
        .get_provider_account(&body.provider_account_id)
        .await
        .map_err(|_| WebApiError::internal("provider auth status unavailable"))?
    else {
        return Err(WebApiError::not_found("provider account not found"));
    };

    if let Err(error) = validate_provider_auth_account(&account, &body.provider_kind, body.method) {
        return Err(error.into());
    }

    match start_codex_provider_auth_attempt(
        &state.provider_auth,
        &state.store,
        &state.paths,
        &account,
    )
    .await
    {
        Ok(attempt) => {
            if !should_persist_provider_auth_attempt_status(&attempt) {
                spawn_provider_auth_terminal_persistence(
                    state.provider_auth.clone(),
                    state.store.clone(),
                    attempt.attempt_id.clone(),
                );
            }
            Ok(attempt)
        }
        Err(StartProviderAuthAttemptError::ProviderUnavailable(message)) => {
            Err(WebApiError::internal(message))
        }
        Err(StartProviderAuthAttemptError::StatusUnavailable) => {
            Err(WebApiError::internal("provider auth status unavailable"))
        }
    }
}

fn validate_provider_auth_account(
    account: &crate::ProviderAccountRecord,
    provider_kind: &str,
    method: crate::ProviderAuthMethod,
) -> Result<(), HttpRequestError> {
    if account.provider_kind != provider_kind {
        return Err(HttpRequestError::bad_request("provider account mismatch"));
    }
    if !account.is_active {
        return Err(HttpRequestError::bad_request("provider account not found"));
    }
    if account.auth_method != method {
        return Err(HttpRequestError::bad_request(
            "provider account auth method mismatch",
        ));
    }
    Ok(())
}

fn validate_json_post_request(request: &HttpRequest) -> Result<(), HttpRequestError> {
    validate_mutation_request(request)?;
    let content_type = request.header("content-type").unwrap_or_default();
    let content_type = content_type.split(';').next().unwrap_or_default().trim();
    if !content_type.eq_ignore_ascii_case("application/json") {
        return Err(HttpRequestError::bad_request("expected JSON request"));
    }
    Ok(())
}

fn validate_mutation_request(request: &HttpRequest) -> Result<(), HttpRequestError> {
    let Some(origin) = request.header("origin") else {
        return Ok(());
    };
    let host = request.header("host").unwrap_or_default();
    if origin_matches_host(origin, host) {
        Ok(())
    } else {
        Err(HttpRequestError::bad_request("invalid request origin"))
    }
}

// Browser POSTs should be same-origin. Keep this intentionally small for the
// local daemon: exact host matches are accepted, as are localhost aliases with
// the same port.
fn origin_matches_host(origin: &str, host: &str) -> bool {
    let Some(origin_host) = origin_authority(origin) else {
        return false;
    };
    origin_host.eq_ignore_ascii_case(host) || local_authorities_match(origin_host, host)
}

fn origin_authority(origin: &str) -> Option<&str> {
    let authority = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    Some(authority.split('/').next().unwrap_or_default())
}

fn local_authorities_match(left: &str, right: &str) -> bool {
    let (left_host, left_port) = split_authority(left);
    let (right_host, right_port) = split_authority(right);
    left_port == right_port && is_local_host(left_host) && is_local_host(right_host)
}

fn split_authority(authority: &str) -> (&str, Option<&str>) {
    authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)))
}

fn is_local_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderAccountStatusUpdate {
    status: crate::ProviderAccountStatus,
    error_code: Option<String>,
    error_message: Option<String>,
}

pub(crate) trait ProviderAccountStatusStore {
    fn update_provider_account_status<'a>(
        &'a self,
        provider_account_id: &'a str,
        status: crate::ProviderAccountStatus,
        error_code: Option<&'a str>,
        error_message: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<(), DaemonError>> + Send + 'a>>;
}

impl ProviderAccountStatusStore for NoemaStore {
    fn update_provider_account_status<'a>(
        &'a self,
        provider_account_id: &'a str,
        status: crate::ProviderAccountStatus,
        error_code: Option<&'a str>,
        error_message: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<(), DaemonError>> + Send + 'a>> {
        Box::pin(async move {
            self.update_provider_account_status(
                provider_account_id,
                status,
                error_code,
                error_message,
            )
            .await?;
            Ok(())
        })
    }
}

trait ProviderAuthAttemptPoller {
    fn poll_provider_auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<ProviderAuthAttemptView>, DaemonError>> + Send + 'a>,
    >;
}

impl ProviderAuthAttemptPoller for ProviderAuthManager {
    fn poll_provider_auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<ProviderAuthAttemptView>, DaemonError>> + Send + 'a>,
    > {
        Box::pin(async move {
            self.poll_attempt(attempt_id)
                .await
                .map_err(|source| DaemonError::Protocol(source.to_string()))
        })
    }
}

trait CodexDeviceAuthStarter {
    fn start_codex_device_code<'a>(
        &'a self,
        request: CodexDeviceAuthRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>> + Send + 'a>,
    >;
}

impl CodexDeviceAuthStarter for ProviderAuthManager {
    fn start_codex_device_code<'a>(
        &'a self,
        request: CodexDeviceAuthRequest,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>> + Send + 'a>,
    > {
        Box::pin(async move { self.start_codex_device_code(request).await })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StartProviderAuthAttemptError {
    ProviderUnavailable(String),
    StatusUnavailable,
}

async fn start_codex_provider_auth_attempt(
    starter: &impl CodexDeviceAuthStarter,
    status_store: &impl ProviderAccountStatusStore,
    paths: &crate::NoemaPaths,
    account: &crate::ProviderAccountRecord,
) -> Result<ProviderAuthAttemptView, StartProviderAuthAttemptError> {
    let attempt = starter
        .start_codex_device_code(CodexDeviceAuthRequest {
            provider_account_id: account.provider_account_id.clone(),
            account_home: paths.provider_account_home(&account.provider_kind, &account.account_key),
            oauth: CodexOAuthConfig::default(),
            attempt_timeout: None,
        })
        .await
        .map_err(|error| StartProviderAuthAttemptError::ProviderUnavailable(error.to_string()))?;

    if should_persist_provider_auth_attempt_status(&attempt) {
        persist_provider_account_status_from_attempt(status_store, &attempt)
            .await
            .map_err(|_| StartProviderAuthAttemptError::StatusUnavailable)?;
    }

    Ok(attempt)
}

fn provider_account_status_update_from_attempt(
    attempt: &ProviderAuthAttemptView,
) -> Option<ProviderAccountStatusUpdate> {
    match attempt.status {
        ProviderAuthAttemptStatus::Completed => Some(ProviderAccountStatusUpdate {
            status: crate::ProviderAccountStatus::Authenticated,
            error_code: None,
            error_message: None,
        }),
        ProviderAuthAttemptStatus::Failed
        | ProviderAuthAttemptStatus::Expired
        | ProviderAuthAttemptStatus::Cancelled => Some(ProviderAccountStatusUpdate {
            status: crate::ProviderAccountStatus::Unauthenticated,
            error_code: attempt.error_code.clone(),
            error_message: attempt.error_message.clone(),
        }),
        ProviderAuthAttemptStatus::Starting | ProviderAuthAttemptStatus::WaitingForUser => None,
    }
}

fn should_persist_provider_auth_attempt_status(attempt: &ProviderAuthAttemptView) -> bool {
    provider_account_status_update_from_attempt(attempt).is_some()
}

pub(crate) async fn persist_provider_account_status_from_attempt(
    store: &impl ProviderAccountStatusStore,
    attempt: &ProviderAuthAttemptView,
) -> Result<(), DaemonError> {
    let Some(update) = provider_account_status_update_from_attempt(attempt) else {
        return Ok(());
    };
    store
        .update_provider_account_status(
            &attempt.provider_account_id,
            update.status,
            update.error_code.as_deref(),
            update.error_message.as_deref(),
        )
        .await?;
    Ok(())
}

fn spawn_provider_auth_terminal_persistence(
    poller: ProviderAuthManager,
    status_store: NoemaStore,
    attempt_id: String,
) {
    tokio::spawn(async move {
        let _ = persist_provider_auth_attempt_terminal_status(
            &poller,
            &status_store,
            &attempt_id,
            PROVIDER_AUTH_TERMINAL_PERSIST_INTERVAL,
        )
        .await;
    });
}

async fn persist_provider_auth_attempt_terminal_status(
    poller: &impl ProviderAuthAttemptPoller,
    status_store: &impl ProviderAccountStatusStore,
    attempt_id: &str,
    poll_interval: Duration,
) -> Result<(), DaemonError> {
    loop {
        let Some(attempt) = poller.poll_provider_auth_attempt(attempt_id).await? else {
            return Ok(());
        };
        if should_persist_provider_auth_attempt_status(&attempt) {
            persist_provider_account_status_from_attempt(status_store, &attempt).await?;
            return Ok(());
        }
        tokio::time::sleep(poll_interval).await;
    }
}

pub(crate) async fn reconcile_onboarding_provider_account(
    status_store: &impl ProviderAccountStatusStore,
    paths: &crate::NoemaPaths,
    account: Option<crate::ProviderAccountRecord>,
) -> Result<Option<crate::ProviderAccountRecord>, DaemonError> {
    let Some(mut account) = account else {
        return Ok(None);
    };
    if account.status == crate::ProviderAccountStatus::Authenticated
        || !codex_account_home_has_noema_tokens(paths, &account)
    {
        return Ok(Some(account));
    }

    status_store
        .update_provider_account_status(
            &account.provider_account_id,
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await?;
    account.status = crate::ProviderAccountStatus::Authenticated;
    account.last_error_code = None;
    account.last_error_message = None;
    Ok(Some(account))
}

fn codex_account_home_has_noema_tokens(
    paths: &crate::NoemaPaths,
    account: &crate::ProviderAccountRecord,
) -> bool {
    if account.provider_kind != "codex" {
        return false;
    }
    let account_home = paths.provider_account_home(&account.provider_kind, &account.account_key);
    CodexTokenStore::new(account_home).has_usable_tokens()
}

async fn upgrade_graphql_websocket(
    mut stream: TcpStream,
    request: &HttpRequest,
    state: WebState,
) -> Result<(), DaemonError> {
    let key = request
        .header("sec-websocket-key")
        .ok_or_else(|| DaemonError::Protocol("missing websocket key".to_string()))?;
    let upgrade = request.header("upgrade").unwrap_or_default();
    if !upgrade.eq_ignore_ascii_case("websocket") {
        write_response(
            &mut stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"expected websocket upgrade",
        )
        .await?;
        return Ok(());
    }

    let response = graphql_websocket_upgrade_response(
        key,
        request.header("sec-websocket-protocol").unwrap_or_default(),
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;

    handle_graphql_websocket(stream, state).await
}

async fn handle_graphql_websocket(
    mut stream: TcpStream,
    state: WebState,
) -> Result<(), DaemonError> {
    let schema = crate::graphql::build_schema(crate::graphql::GraphqlState::from_web_state(state));
    loop {
        let Some(frame) = WebSocketFrame::read_from(&mut stream).await? else {
            return Ok(());
        };

        match frame.opcode {
            WebSocketOpcode::Text => {
                let text = String::from_utf8(frame.payload)
                    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                let message = serde_json::from_str::<Value>(&text)
                    .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                let message_type =
                    message.get("type").and_then(Value::as_str).ok_or_else(|| {
                        DaemonError::Protocol("missing GraphQL websocket message type".to_string())
                    })?;

                match message_type {
                    "connection_init" => {
                        send_ws_json(&mut stream, &json!({ "type": "connection_ack" })).await?;
                    }
                    "subscribe" => {
                        let id = message
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or("1")
                            .to_string();
                        let payload = message.get("payload").cloned().ok_or_else(|| {
                            DaemonError::Protocol("missing GraphQL websocket payload".to_string())
                        })?;
                        let request = serde_json::from_value::<async_graphql::Request>(payload)
                            .map_err(|source| DaemonError::Protocol(source.to_string()))?;
                        let mut responses = schema.execute_stream(request);
                        while let Some(response) = responses.next().await {
                            send_ws_json(
                                &mut stream,
                                &json!({
                                    "id": id,
                                    "type": "next",
                                    "payload": response,
                                }),
                            )
                            .await?;
                        }
                        send_ws_json(
                            &mut stream,
                            &json!({
                                "id": id,
                                "type": "complete",
                            }),
                        )
                        .await?;
                    }
                    "complete" => return Ok(()),
                    _ => {
                        send_ws_json(
                            &mut stream,
                            &json!({
                                "type": "error",
                                "payload": [
                                    { "message": "unsupported GraphQL websocket message type" }
                                ],
                            }),
                        )
                        .await?;
                    }
                }
            }
            WebSocketOpcode::Ping => {
                write_ws_frame(&mut stream, WebSocketOpcode::Pong, &frame.payload).await?;
            }
            WebSocketOpcode::Pong => {}
        }
    }
}

pub(crate) fn is_user_onboarded_for_chat(account: Option<crate::ProviderAccountRecord>) -> bool {
    crate::onboarding_status_from_account(account).is_user_onboarded
}

pub(crate) async fn visible_conversation_replay(
    repo: &NoemaStore,
    conversation_id: &str,
) -> Result<Vec<ConversationItemRecord>, DaemonError> {
    Ok(repo
        .list_conversation_items(conversation_id, ReplayMode::Visible)
        .await?)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConversationReplayItem {
    pub(crate) item_id: String,
    pub(crate) turn_id: Option<String>,
    pub(crate) item: TurnTranscriptItem,
}

impl ConversationReplayItem {
    pub(crate) fn new(item_id: String, turn_id: Option<String>, item: TurnTranscriptItem) -> Self {
        Self {
            item_id,
            turn_id,
            item,
        }
    }
}

pub(crate) fn web_conversation_item_from_record(
    record: ConversationItemRecord,
) -> Result<Option<ConversationReplayItem>, DaemonError> {
    let Some(item) = turn_transcript_item_from_record(&record)? else {
        return Ok(None);
    };
    Ok(Some(ConversationReplayItem::new(
        record.item_id,
        record.turn_id,
        item,
    )))
}

fn turn_transcript_item_from_record(
    record: &ConversationItemRecord,
) -> Result<Option<TurnTranscriptItem>, DaemonError> {
    match record.kind {
        ConversationItemKind::UserText => Ok(Some(TurnTranscriptItem::UserText {
            text: required_content_text(record)?,
        })),
        ConversationItemKind::AssistantText => Ok(Some(TurnTranscriptItem::AssistantText {
            text: required_content_text(record)?,
        })),
        ConversationItemKind::Activity => {
            let payload: ReplayActivityPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::Activity {
                id: payload.id,
                activity_kind: payload.activity_kind,
                status: payload.status,
                title: payload.title,
                summary: payload.summary,
                metadata: payload.metadata,
            }))
        }
        ConversationItemKind::A2uiCard => {
            let payload: ReplayA2uiCardPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::A2uiCard {
                id: payload.id,
                schema: payload.schema,
                payload: payload.payload,
            }))
        }
        ConversationItemKind::ErrorNotice => {
            let payload: ReplayErrorNoticePayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::ErrorNotice {
                message: payload
                    .message
                    .or_else(|| record.content_text.clone())
                    .ok_or_else(|| missing_replay_field(record, "message"))?,
                recoverable: payload.recoverable,
            }))
        }
        ConversationItemKind::ToolCall
        | ConversationItemKind::ToolResult
        | ConversationItemKind::ApprovalRequest
        | ConversationItemKind::ApprovalResult => {
            let payload: ReplayActivityPayload = replay_payload(record)?;
            Ok(Some(TurnTranscriptItem::Activity {
                id: payload.id,
                activity_kind: payload.activity_kind,
                status: payload.status,
                title: payload.title,
                summary: payload.summary,
                metadata: payload.metadata,
            }))
        }
    }
}

fn replay_payload<T: DeserializeOwned>(record: &ConversationItemRecord) -> Result<T, DaemonError> {
    serde_json::from_value(record.payload_json.clone()).map_err(|source| {
        DaemonError::Protocol(format!(
            "invalid replay payload for {} {}: {source}",
            record.kind.as_str(),
            record.item_id
        ))
    })
}

fn required_content_text(record: &ConversationItemRecord) -> Result<String, DaemonError> {
    record
        .content_text
        .clone()
        .ok_or_else(|| missing_replay_field(record, "content_text"))
}

fn missing_replay_field(record: &ConversationItemRecord, field: &str) -> DaemonError {
    DaemonError::Protocol(format!(
        "missing replay field {field} for {} {}",
        record.kind.as_str(),
        record.item_id
    ))
}

#[derive(Debug, Deserialize)]
struct ReplayActivityPayload {
    id: String,
    activity_kind: String,
    status: TurnActivityStatus,
    title: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    metadata: Value,
}

#[derive(Debug, Deserialize)]
struct ReplayA2uiCardPayload {
    id: String,
    schema: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
struct ReplayErrorNoticePayload {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    recoverable: bool,
}

async fn send_ws_json<T: Serialize>(stream: &mut TcpStream, value: &T) -> Result<(), DaemonError> {
    let bytes =
        serde_json::to_vec(value).map_err(|source| DaemonError::Protocol(source.to_string()))?;
    write_ws_frame(stream, WebSocketOpcode::Text, &bytes).await
}

fn websocket_accept_key(key: &str) -> String {
    let mut value = String::with_capacity(key.len() + WEBSOCKET_GUID.len());
    value.push_str(key.trim());
    value.push_str(WEBSOCKET_GUID);
    let digest = digest(&SHA1_FOR_LEGACY_USE_ONLY, value.as_bytes());
    general_purpose::STANDARD.encode(digest.as_ref())
}

fn graphql_websocket_upgrade_response(key: &str, requested_protocols: &str) -> String {
    let accept = websocket_accept_key(key);
    let protocol_header = websocket_protocol_header(requested_protocols);
    format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n{protocol_header}\r\n"
    )
}

fn websocket_protocol_header(requested_protocols: &str) -> &'static str {
    if requested_protocols
        .split(',')
        .map(str::trim)
        .any(|protocol| protocol == GRAPHQL_WS_PROTOCOL)
    {
        "Sec-WebSocket-Protocol: graphql-transport-ws\r\n"
    } else {
        ""
    }
}

#[derive(Debug)]
struct WebSocketFrame {
    opcode: WebSocketOpcode,
    payload: Vec<u8>,
}

impl WebSocketFrame {
    async fn read_from(stream: &mut TcpStream) -> Result<Option<Self>, DaemonError> {
        let mut header = [0_u8; 2];
        match stream.read_exact(&mut header).await {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error.into()),
        }

        let opcode = match header[0] & 0x0f {
            0x1 => WebSocketOpcode::Text,
            0x8 => return Ok(None),
            0x9 => WebSocketOpcode::Ping,
            0xA => WebSocketOpcode::Pong,
            other => {
                return Err(DaemonError::Protocol(format!(
                    "unsupported websocket opcode: {other}"
                )));
            }
        };
        let masked = header[1] & 0x80 != 0;
        if !masked {
            return Err(DaemonError::Protocol(
                "client websocket frame was not masked".to_string(),
            ));
        }

        let payload_len = read_payload_len(stream, header[1] & 0x7f).await?;
        if payload_len > MAX_WS_FRAME_BYTES {
            return Err(DaemonError::Protocol(
                "websocket frame too large".to_string(),
            ));
        }

        let mut mask = [0_u8; 4];
        stream.read_exact(&mut mask).await?;
        let mut payload = vec![0_u8; payload_len];
        stream.read_exact(&mut payload).await?;
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % mask.len()];
        }

        Ok(Some(Self { opcode, payload }))
    }
}

async fn read_payload_len(stream: &mut TcpStream, initial: u8) -> Result<usize, DaemonError> {
    match initial {
        len @ 0..=125 => Ok(usize::from(len)),
        126 => {
            let mut bytes = [0_u8; 2];
            stream.read_exact(&mut bytes).await?;
            Ok(usize::from(u16::from_be_bytes(bytes)))
        }
        127 => {
            let mut bytes = [0_u8; 8];
            stream.read_exact(&mut bytes).await?;
            usize::try_from(u64::from_be_bytes(bytes)).map_err(|_| {
                DaemonError::Protocol("websocket frame length exceeds platform size".to_string())
            })
        }
        _ => unreachable!("masked bit is stripped before payload length parsing"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebSocketOpcode {
    Text,
    Ping,
    Pong,
}

async fn write_ws_frame(
    stream: &mut TcpStream,
    opcode: WebSocketOpcode,
    payload: &[u8],
) -> Result<(), DaemonError> {
    let opcode = match opcode {
        WebSocketOpcode::Text => 0x1,
        WebSocketOpcode::Ping => 0x9,
        WebSocketOpcode::Pong => 0xA,
    };
    let mut header = Vec::with_capacity(10);
    header.push(0x80 | opcode);
    match payload.len() {
        len @ 0..=125 => header.push(u8::try_from(len).expect("small websocket payload")),
        len @ 126..=65_535 => {
            header.push(126);
            header.extend_from_slice(
                &u16::try_from(len)
                    .expect("medium websocket payload")
                    .to_be_bytes(),
            );
        }
        len => {
            header.push(127);
            header.extend_from_slice(
                &u64::try_from(len)
                    .expect("large websocket payload")
                    .to_be_bytes(),
            );
        }
    }

    stream.write_all(&header).await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::codex_oauth::CodexOAuthTokens;
    use crate::{
        provider_auth::ProviderAuthAttemptView,
        {ConversationItemKind, ConversationItemRecord, ConversationItemStatus},
    };
    use serde_json::json;

    #[test]
    fn graphql_endpoint_accepts_post_path() {
        assert!(is_graphql_http_route("POST", "/graphql"));
    }

    #[test]
    fn graphql_schema_endpoint_accepts_get_path() {
        assert!(is_graphql_schema_route("GET", "/graphql/schema.graphql"));
    }

    #[test]
    fn graphql_ws_endpoint_accepts_get_path() {
        assert!(is_graphql_ws_route("GET", "/graphql/ws"));
    }

    #[test]
    fn legacy_chat_ws_is_no_longer_client_product_api() {
        assert!(!is_supported_product_route("GET", "/api/chat/ws"));
    }

    #[test]
    fn legacy_status_is_no_longer_client_product_api() {
        assert!(!is_supported_product_route("GET", "/api/status"));
    }

    #[test]
    fn legacy_provider_auth_routes_are_no_longer_client_product_api() {
        assert!(!is_supported_product_route("GET", "/api/onboarding/status"));
        assert!(!is_supported_product_route("GET", "/api/provider-accounts"));
        assert!(!is_supported_product_route(
            "POST",
            "/api/provider-auth/attempts"
        ));
        assert!(!is_supported_product_route(
            "GET",
            "/api/provider-auth/attempts/attempt_1"
        ));
        assert!(!is_supported_product_route(
            "POST",
            "/api/provider-auth/attempts/attempt_1/cancel"
        ));
    }

    #[test]
    fn graphql_routes_remain_supported_product_api() {
        assert!(is_supported_product_route("POST", "/graphql"));
        assert!(is_supported_product_route("GET", "/graphql/ws"));
        assert!(is_supported_product_route("GET", "/graphql/schema.graphql"));
    }

    #[test]
    fn memory_routes_serve_spa_entry_asset() {
        for path in ["/memory", "/memory/graph", "/memory/nope"] {
            let asset = embedded_asset(path).expect("memory route should serve index");
            assert_eq!(asset.content_type, "text/html; charset=utf-8");
        }
    }

    #[test]
    fn unknown_web_routes_serve_spa_entry_asset() {
        let asset = embedded_asset("/not-a-real-route").expect("unknown route should serve index");
        assert_eq!(asset.content_type, "text/html; charset=utf-8");
    }

    #[test]
    fn missing_static_assets_still_miss() {
        assert!(embedded_asset("/assets/missing.css").is_none());
    }

    #[tokio::test]
    async fn http_request_reads_json_body() {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            HttpRequest::read_from(&mut stream).await.expect("request")
        });

        let mut client = TcpStream::connect(address).await.expect("connect client");
        client
            .write_all(
                b"POST /graphql?ignore=true HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 21\r\n\r\n{\"hello\":\"from-body\"}",
            )
            .await
            .expect("write request");
        client.flush().await.expect("flush request");

        let request = server.await.expect("server task");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/graphql");
        assert_eq!(request.body, br#"{"hello":"from-body"}"#);
    }

    #[tokio::test]
    async fn http_request_rejects_oversized_body() {
        let error = read_test_request_error(format!(
            "POST /graphql HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_API_BODY_BYTES + 1
        )
        .as_bytes())
        .await;

        assert_eq!(error.status(), "413 Payload Too Large");
        assert_eq!(error.message(), "request body too large");
    }

    #[tokio::test]
    async fn http_request_rejects_invalid_content_length() {
        let error = read_test_request_error(
            b"POST /graphql HTTP/1.1\r\nHost: localhost\r\nContent-Length: nope\r\n\r\n",
        )
        .await;

        assert_eq!(error.status(), "400 Bad Request");
        assert_eq!(error.message(), "invalid content length");
    }

    #[tokio::test]
    async fn http_request_rejects_duplicate_content_length() {
        let error = read_test_request_error(
            b"POST /graphql HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
        )
        .await;

        assert_eq!(error.status(), "400 Bad Request");
        assert_eq!(error.message(), "duplicate content length");
    }

    #[tokio::test]
    async fn http_request_rejects_surplus_body_bytes() {
        let error = read_test_request_error(
            b"POST /graphql HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\n\r\n{}extra",
        )
        .await;

        assert_eq!(error.status(), "400 Bad Request");
        assert_eq!(error.message(), "unexpected request bytes");
    }

    #[tokio::test]
    async fn http_request_times_out_waiting_for_body() {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            HttpRequest::read_from(&mut stream)
                .await
                .expect_err("request should time out")
        });

        let mut client = TcpStream::connect(address).await.expect("connect client");
        client
            .write_all(b"POST /graphql HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\n\r\n{}")
            .await
            .expect("write partial request");
        client.flush().await.expect("flush request");

        let error = server.await.expect("server task");
        assert_eq!(error.status(), "400 Bad Request");
        assert_eq!(error.message(), "request body timed out");
    }

    #[test]
    fn start_auth_requires_local_json_post_context() {
        let mut request = test_request("POST", "/graphql");
        request
            .headers
            .insert("host".to_string(), "localhost:8765".to_string());
        request.headers.insert(
            "content-type".to_string(),
            "application/json; charset=utf-8".to_string(),
        );
        request
            .headers
            .insert("origin".to_string(), "http://localhost:8765".to_string());

        assert!(validate_json_post_request(&request).is_ok());

        request
            .headers
            .insert("content-type".to_string(), "text/plain".to_string());
        assert_eq!(
            validate_json_post_request(&request).unwrap_err().message(),
            "expected JSON request"
        );

        request
            .headers
            .insert("content-type".to_string(), "application/json".to_string());
        request
            .headers
            .insert("origin".to_string(), "https://example.com".to_string());
        assert_eq!(
            validate_json_post_request(&request).unwrap_err().message(),
            "invalid request origin"
        );
    }

    #[test]
    fn provider_auth_account_validation_checks_active_kind_and_method() {
        let mut account = test_provider_account();
        assert!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .is_ok()
        );

        account.provider_kind = "other".to_string();
        assert_eq!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .unwrap_err()
            .message(),
            "provider account mismatch"
        );

        account = test_provider_account();
        account.is_active = false;
        assert_eq!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .unwrap_err()
            .message(),
            "provider account not found"
        );

        account = test_provider_account();
        account.auth_method = crate::ProviderAuthMethod::ExternalManual;
        assert_eq!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .unwrap_err()
            .message(),
            "provider account auth method mismatch"
        );
    }

    #[test]
    fn provider_auth_attempt_status_maps_to_safe_account_status() {
        let mut attempt = test_provider_auth_attempt();
        attempt.status = crate::provider_auth::ProviderAuthAttemptStatus::Completed;
        assert_eq!(
            provider_account_status_update_from_attempt(&attempt),
            Some(ProviderAccountStatusUpdate {
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            })
        );

        attempt.status = crate::provider_auth::ProviderAuthAttemptStatus::Failed;
        attempt.error_code = Some("codex_login_failed".to_string());
        attempt.error_message = Some("codex auth failed".to_string());
        assert_eq!(
            provider_account_status_update_from_attempt(&attempt),
            Some(ProviderAccountStatusUpdate {
                status: crate::ProviderAccountStatus::Unauthenticated,
                error_code: Some("codex_login_failed".to_string()),
                error_message: Some("codex auth failed".to_string()),
            })
        );

        attempt.status = crate::provider_auth::ProviderAuthAttemptStatus::WaitingForUser;
        assert_eq!(provider_account_status_update_from_attempt(&attempt), None);
    }

    #[tokio::test]
    async fn start_auth_returned_completed_attempt_persists_authenticated_status() {
        let store = RecordingProviderAccountStatusStore::default();
        let mut attempt = test_provider_auth_attempt();
        attempt.status = crate::provider_auth::ProviderAuthAttemptStatus::Completed;
        let starter = RecordingCodexDeviceAuthStarter { attempt };
        let paths =
            crate::NoemaPaths::from_noema_home(tempfile::tempdir().expect("temp dir").path())
                .expect("paths");

        start_codex_provider_auth_attempt(&starter, &store, &paths, &test_provider_account())
            .await
            .expect("start provider auth");

        let updates = store.updates.lock().expect("updates lock");
        assert_eq!(
            updates.as_slice(),
            [RecordedProviderAccountStatusUpdate {
                provider_account_id: "provider_account:codex:default".to_string(),
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }]
        );
    }

    #[tokio::test]
    async fn start_auth_preserves_provider_start_error_message() {
        let store = RecordingProviderAccountStatusStore::default();
        let starter = FailingCodexDeviceAuthStarter {
            message: "device code request returned status 403",
        };
        let paths =
            crate::NoemaPaths::from_noema_home(tempfile::tempdir().expect("temp dir").path())
                .expect("paths");

        let error =
            start_codex_provider_auth_attempt(&starter, &store, &paths, &test_provider_account())
                .await
                .expect_err("auth start should fail");

        assert_eq!(
            error,
            StartProviderAuthAttemptError::ProviderUnavailable(
                "codex provider is unavailable: device code request returned status 403"
                    .to_string()
            )
        );
        assert!(store.updates.lock().expect("updates lock").is_empty());
    }

    #[tokio::test]
    async fn auth_terminal_watcher_persists_completed_attempt_without_http_poll() {
        let store = RecordingProviderAccountStatusStore::default();
        let mut waiting = test_provider_auth_attempt();
        waiting.status = crate::provider_auth::ProviderAuthAttemptStatus::WaitingForUser;
        let mut completed = waiting.clone();
        completed.status = crate::provider_auth::ProviderAuthAttemptStatus::Completed;
        let poller = RecordingProviderAuthAttemptPoller::new(vec![waiting, completed]);

        persist_provider_auth_attempt_terminal_status(
            &poller,
            &store,
            "provider_auth_attempt_test",
            Duration::from_millis(1),
        )
        .await
        .expect("persist terminal status");

        let updates = store.updates.lock().expect("updates lock");
        assert_eq!(
            updates.as_slice(),
            [RecordedProviderAccountStatusUpdate {
                provider_account_id: "provider_account:codex:default".to_string(),
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }]
        );
    }

    #[tokio::test]
    async fn onboarding_reconciles_existing_noema_codex_tokens() {
        let store = RecordingProviderAccountStatusStore::default();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let paths = crate::NoemaPaths::from_noema_home(temp_dir.path()).expect("paths");
        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Unauthenticated;
        let account_home =
            paths.provider_account_home(&account.provider_kind, &account.account_key);
        CodexTokenStore::new(account_home)
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 123,
            })
            .expect("credential marker");

        let reconciled = reconcile_onboarding_provider_account(&store, &paths, Some(account))
            .await
            .expect("reconcile account")
            .expect("account");

        assert_eq!(
            reconciled.status,
            crate::ProviderAccountStatus::Authenticated
        );
        let updates = store.updates.lock().expect("updates lock");
        assert_eq!(
            updates.as_slice(),
            [RecordedProviderAccountStatusUpdate {
                provider_account_id: "provider_account:codex:default".to_string(),
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }]
        );
    }

    async fn read_test_request_error(bytes: &[u8]) -> HttpRequestError {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            HttpRequest::read_from(&mut stream)
                .await
                .expect_err("request should fail")
        });

        let mut client = TcpStream::connect(address).await.expect("connect client");
        client.write_all(bytes).await.expect("write request");
        client.flush().await.expect("flush request");

        server.await.expect("server task")
    }

    fn test_request(method: &str, path: &str) -> HttpRequest {
        HttpRequest {
            method: method.to_string(),
            path: path.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }

    fn test_provider_account() -> crate::ProviderAccountRecord {
        crate::ProviderAccountRecord {
            provider_account_id: "provider_account:codex:default".to_string(),
            provider_kind: "codex".to_string(),
            account_key: "default".to_string(),
            display_name: "Codex".to_string(),
            auth_method: crate::ProviderAuthMethod::OauthDeviceCode,
            is_active: true,
            is_default: true,
            status: crate::ProviderAccountStatus::Unknown,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({}),
        }
    }

    fn test_provider_auth_attempt() -> ProviderAuthAttemptView {
        ProviderAuthAttemptView {
            attempt_id: "provider_auth_attempt_test".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: crate::ProviderAuthMethod::OauthDeviceCode,
            status: crate::provider_auth::ProviderAuthAttemptStatus::Starting,
            verification_url: None,
            user_code: None,
            instructions: None,
            error_code: None,
            error_message: None,
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedProviderAccountStatusUpdate {
        provider_account_id: String,
        status: crate::ProviderAccountStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    }

    #[derive(Default)]
    struct RecordingProviderAccountStatusStore {
        updates: std::sync::Mutex<Vec<RecordedProviderAccountStatusUpdate>>,
    }

    struct RecordingCodexDeviceAuthStarter {
        attempt: ProviderAuthAttemptView,
    }

    struct FailingCodexDeviceAuthStarter {
        message: &'static str,
    }

    struct RecordingProviderAuthAttemptPoller {
        attempts: std::sync::Mutex<Vec<ProviderAuthAttemptView>>,
    }

    impl RecordingProviderAuthAttemptPoller {
        fn new(attempts: Vec<ProviderAuthAttemptView>) -> Self {
            Self {
                attempts: std::sync::Mutex::new(attempts),
            }
        }
    }

    impl CodexDeviceAuthStarter for RecordingCodexDeviceAuthStarter {
        fn start_codex_device_code<'a>(
            &'a self,
            _request: CodexDeviceAuthRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>>
                    + Send
                    + 'a,
            >,
        > {
            let attempt = self.attempt.clone();
            Box::pin(async move { Ok(attempt) })
        }
    }

    impl CodexDeviceAuthStarter for FailingCodexDeviceAuthStarter {
        fn start_codex_device_code<'a>(
            &'a self,
            _request: CodexDeviceAuthRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>>
                    + Send
                    + 'a,
            >,
        > {
            let message = self.message.to_string();
            Box::pin(async move {
                Err(crate::ProviderError::ProviderUnavailable {
                    provider: "codex".to_string(),
                    message,
                })
            })
        }
    }

    impl ProviderAuthAttemptPoller for RecordingProviderAuthAttemptPoller {
        fn poll_provider_auth_attempt<'a>(
            &'a self,
            _attempt_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Option<ProviderAuthAttemptView>, DaemonError>>
                    + Send
                    + 'a,
            >,
        > {
            let attempt = {
                let mut attempts = self.attempts.lock().expect("attempts lock");
                if attempts.len() > 1 {
                    Some(attempts.remove(0))
                } else {
                    attempts.first().cloned()
                }
            };
            Box::pin(async move { Ok(attempt) })
        }
    }

    impl ProviderAccountStatusStore for RecordingProviderAccountStatusStore {
        fn update_provider_account_status<'a>(
            &'a self,
            provider_account_id: &'a str,
            status: crate::ProviderAccountStatus,
            error_code: Option<&'a str>,
            error_message: Option<&'a str>,
        ) -> Pin<Box<dyn Future<Output = Result<(), DaemonError>> + Send + 'a>> {
            Box::pin(async move {
                self.updates.lock().expect("updates lock").push(
                    RecordedProviderAccountStatusUpdate {
                        provider_account_id: provider_account_id.to_string(),
                        status,
                        error_code: error_code.map(str::to_string),
                        error_message: error_message.map(str::to_string),
                    },
                );
                Ok(())
            })
        }
    }

    #[test]
    fn websocket_accept_key_matches_rfc_example() {
        assert_eq!(
            websocket_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn graphql_websocket_upgrade_echoes_requested_subprotocol() {
        let response =
            graphql_websocket_upgrade_response("dGhlIHNhbXBsZSBub25jZQ==", "graphql-transport-ws");

        assert!(response.starts_with("HTTP/1.1 101 Switching Protocols\r\n"));
        assert!(response.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"));
        assert!(response.contains("Sec-WebSocket-Protocol: graphql-transport-ws\r\n"));
    }

    #[test]
    fn graphql_websocket_upgrade_omits_unrequested_subprotocol() {
        let response = graphql_websocket_upgrade_response("dGhlIHNhbXBsZSBub25jZQ==", "");

        assert!(!response.contains("Sec-WebSocket-Protocol:"));
    }

    #[test]
    fn chat_onboarding_gate_requires_authenticated_provider_account() {
        assert!(!is_user_onboarded_for_chat(None));

        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Unknown;
        assert!(!is_user_onboarded_for_chat(Some(account)));

        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Unauthenticated;
        assert!(!is_user_onboarded_for_chat(Some(account)));

        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Authenticated;
        assert!(is_user_onboarded_for_chat(Some(account)));
    }

    #[test]
    fn conversation_replay_item_converts_assistant_text_record() {
        let item = web_conversation_item_from_record(ConversationItemRecord {
            item_id: "item_1".to_string(),
            conversation_id: "conversation_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            kind: ConversationItemKind::AssistantText,
            status: ConversationItemStatus::Completed,
            content_text: Some("hello from replay".to_string()),
            payload_json: json!({}),
        })
        .expect("convert record")
        .expect("visible item");

        assert_eq!(item.item_id, "item_1");
        assert_eq!(item.turn_id.as_deref(), Some("turn_1"));
        assert_eq!(
            item.item,
            TurnTranscriptItem::AssistantText {
                text: "hello from replay".to_string()
            }
        );
    }

    #[test]
    fn conversation_replay_item_converts_persisted_action_record() {
        let item = web_conversation_item_from_record(ConversationItemRecord {
            item_id: "item_tool_1".to_string(),
            conversation_id: "conversation_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            kind: ConversationItemKind::ToolCall,
            status: ConversationItemStatus::Completed,
            content_text: Some("Tool call: search_memory".to_string()),
            payload_json: json!({
                "id": "tool_call:conversation_1:0:1",
                "activity_kind": "tool_call",
                "status": "completed",
                "title": "Tool call: search_memory",
                "summary": "provider id call_1",
                "metadata": {"action": {"name": "search_memory"}},
            }),
        })
        .expect("convert record")
        .expect("visible item");

        assert_eq!(item.item_id, "item_tool_1");
        assert!(matches!(
            item.item,
            TurnTranscriptItem::Activity {
                ref activity_kind,
                ref title,
                ..
            } if activity_kind == "tool_call" && title == "Tool call: search_memory"
        ));
    }

    #[test]
    fn conversation_replay_item_rejects_malformed_activity_record() {
        let error = web_conversation_item_from_record(ConversationItemRecord {
            item_id: "item_bad".to_string(),
            conversation_id: "conversation_1".to_string(),
            turn_id: Some("turn_1".to_string()),
            kind: ConversationItemKind::Activity,
            status: ConversationItemStatus::Completed,
            content_text: None,
            payload_json: json!({ "not": "an activity payload" }),
        })
        .expect_err("malformed record should fail");

        assert!(error.to_string().contains("invalid replay payload"));
    }
}
