//! Local web UI server for the Noema daemon.

mod assets;
mod graphql_ws;
mod http;
mod origin;
mod provider_auth;
mod replay;

use tokio::net::{TcpListener, TcpStream};

use crate::WebConfig;

use self::{
    assets::embedded_asset,
    graphql_ws::upgrade_graphql_websocket,
    http::{HttpRequest, write_json, write_json_error, write_response},
    origin::validate_json_post_request,
};

pub(crate) use self::{
    provider_auth::{
        ProviderAuthStartRequest, is_user_onboarded_for_chat,
        persist_provider_account_status_from_attempt, reconcile_onboarding_provider_account,
        start_provider_auth_attempt_view_from_parts,
    },
    replay::{
        ConversationReplayItem, visible_conversation_replay, web_conversation_item_from_record,
    },
};

use super::protocol::DaemonError;

/// State shared by local web UI connections.
#[derive(Clone)]
pub(crate) struct WebState {
    graphql_state: crate::graphql::GraphqlState,
}

impl WebState {
    /// Build shared web UI state.
    #[must_use]
    pub(super) fn new(graphql_state: crate::graphql::GraphqlState) -> Self {
        Self { graphql_state }
    }

    pub(crate) fn graphql_state(&self) -> &crate::graphql::GraphqlState {
        &self.graphql_state
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

    if is_graphiql_route(&request.method, &request.path) {
        let page = async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish();
        write_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            page.as_bytes(),
        )
        .await?;
        return Ok(());
    }

    if is_graphql_schema_route(&request.method, &request.path) {
        let schema = crate::graphql::build_schema(state.graphql_state().clone());
        write_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            schema.sdl().as_bytes(),
        )
        .await?;
        return Ok(());
    }

    if is_mcp_oauth_callback_route(&request.method, &request.path) {
        handle_mcp_oauth_callback(&mut stream, state, &request).await?;
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

fn is_graphiql_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/graphql"
}

fn is_graphql_schema_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/graphql/schema.graphql"
}

fn is_graphql_ws_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/graphql/ws"
}

fn is_mcp_oauth_callback_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/mcp/oauth/callback"
}

#[cfg(test)]
fn is_supported_product_route(method: &str, path: &str) -> bool {
    is_graphiql_route(method, path)
        || is_graphql_http_route(method, path)
        || is_graphql_schema_route(method, path)
        || is_graphql_ws_route(method, path)
        || is_mcp_oauth_callback_route(method, path)
}

async fn handle_mcp_oauth_callback(
    stream: &mut TcpStream,
    state: WebState,
    request: &HttpRequest,
) -> Result<(), DaemonError> {
    let Some(query) = request.query.as_deref() else {
        write_response(
            stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"missing OAuth callback query",
        )
        .await?;
        return Ok(());
    };
    let Some(attempt_id) = query_value(query, "attemptId") else {
        write_response(
            stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"missing MCP OAuth attempt id",
        )
        .await?;
        return Ok(());
    };
    let Some(callback_url) = callback_url_from_request(request) else {
        write_response(
            stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"invalid MCP OAuth callback",
        )
        .await?;
        return Ok(());
    };
    let response = crate::graphql::complete_mcp_server_oauth_setup(
        state.graphql_state(),
        &attempt_id,
        &callback_url,
    )
    .await;
    match response {
        Ok(attempt) if attempt.status == "completed" => {
            write_response(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                b"<!doctype html><title>Noema MCP OAuth</title><p>Authentication completed. You can return to Noema.</p>",
            )
            .await?;
        }
        Ok(_) => {
            write_response(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                b"<!doctype html><title>Noema MCP OAuth</title><p>Authentication finished, but Noema could not list tools. Return to Noema to retry.</p>",
            )
            .await?;
        }
        Err(_) => {
            write_response(
                stream,
                "400 Bad Request",
                "text/html; charset=utf-8",
                b"<!doctype html><title>Noema MCP OAuth</title><p>Noema could not complete this MCP OAuth setup attempt.</p>",
            )
            .await?;
        }
    }
    Ok(())
}

fn query_value(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _value)| name == key)
        .map(|(_name, value)| value.into_owned())
}

fn callback_url_from_request(request: &HttpRequest) -> Option<String> {
    let host = request.header("host")?;
    let mut url = format!("http://{host}{}", request.path);
    if let Some(query) = &request.query {
        url.push('?');
        url.push_str(query);
    }
    Some(url)
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
    let schema = crate::graphql::build_schema(state.graphql_state().clone());
    let response = schema.execute(graphql_request).await;
    write_json(stream, "200 OK", &response).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

    use crate::provider::adapters::codex_oauth::{CodexOAuthTokens, CodexTokenStore};
    use crate::{
        TurnTranscriptItem,
        provider::auth::{CodexDeviceAuthRequest, ProviderAuthAttemptView},
        {ConversationItemKind, ConversationItemRecord, ConversationItemStatus},
    };
    use serde_json::json;
    use tokio::{
        io::AsyncWriteExt,
        net::{TcpListener, TcpStream},
    };

    use super::{
        graphql_ws::{
            graphql_websocket_upgrade_response, validate_graphql_websocket_upgrade_request,
            websocket_accept_key,
        },
        http::{HttpRequestError, MAX_API_BODY_BYTES},
        provider_auth::{
            CodexDeviceAuthStarter, ProviderAccountStatusStore, ProviderAccountStatusUpdate,
            ProviderAuthAttemptPoller, StartProviderAuthAttemptError,
            persist_provider_auth_attempt_terminal_status,
            provider_account_status_update_from_attempt, start_codex_provider_auth_attempt,
            validate_provider_auth_account,
        },
    };

    #[test]
    fn graphql_endpoint_accepts_post_path() {
        assert!(is_graphql_http_route("POST", "/graphql"));
    }

    #[test]
    fn graphiql_endpoint_accepts_get_path() {
        assert!(is_graphiql_route("GET", "/graphql"));
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
        assert!(is_supported_product_route("GET", "/graphql"));
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
        assert_eq!(request.query.as_deref(), Some("ignore=true"));
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
    fn graphql_websocket_upgrade_accepts_same_origin() {
        let mut request = test_request("GET", "/graphql/ws");
        request
            .headers
            .insert("host".to_string(), "127.0.0.1:8765".to_string());
        request
            .headers
            .insert("origin".to_string(), "http://localhost:8765".to_string());
        request
            .headers
            .insert("upgrade".to_string(), "websocket".to_string());
        request.headers.insert(
            "sec-websocket-key".to_string(),
            "dGhlIHNhbXBsZSBub25jZQ==".to_string(),
        );

        assert!(validate_graphql_websocket_upgrade_request(&request).is_ok());
    }

    #[test]
    fn graphql_websocket_upgrade_rejects_cross_origin() {
        let mut request = test_request("GET", "/graphql/ws");
        request
            .headers
            .insert("host".to_string(), "localhost:8765".to_string());
        request
            .headers
            .insert("origin".to_string(), "https://example.com".to_string());
        request
            .headers
            .insert("upgrade".to_string(), "websocket".to_string());
        request.headers.insert(
            "sec-websocket-key".to_string(),
            "dGhlIHNhbXBsZSBub25jZQ==".to_string(),
        );

        let error = validate_graphql_websocket_upgrade_request(&request)
            .expect_err("cross-origin websocket should be rejected");
        assert_eq!(error.status(), "400 Bad Request");
        assert_eq!(error.message(), "invalid request origin");
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
        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::Completed;
        assert_eq!(
            provider_account_status_update_from_attempt(&attempt),
            Some(ProviderAccountStatusUpdate {
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            })
        );

        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::Failed;
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

        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::WaitingForUser;
        assert_eq!(provider_account_status_update_from_attempt(&attempt), None);
    }

    #[tokio::test]
    async fn start_auth_returned_completed_attempt_persists_authenticated_status() {
        let store = RecordingProviderAccountStatusStore::default();
        let mut attempt = test_provider_auth_attempt();
        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::Completed;
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
        waiting.status = crate::provider::auth::ProviderAuthAttemptStatus::WaitingForUser;
        let mut completed = waiting.clone();
        completed.status = crate::provider::auth::ProviderAuthAttemptStatus::Completed;
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
            query: None,
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
            status: crate::provider::auth::ProviderAuthAttemptStatus::Starting,
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
            sequence_index: 1,
            cursor: "conversation_item:1".to_string(),
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
            sequence_index: 1,
            cursor: "conversation_item:1".to_string(),
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
            sequence_index: 1,
            cursor: "conversation_item:1".to_string(),
            kind: ConversationItemKind::Activity,
            status: ConversationItemStatus::Completed,
            content_text: None,
            payload_json: json!({ "not": "an activity payload" }),
        })
        .expect_err("malformed record should fail");

        assert!(error.to_string().contains("invalid replay payload"));
    }
}
