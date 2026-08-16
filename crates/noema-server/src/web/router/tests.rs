use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::digest;
use serde_json::json;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use tower::ServiceExt;

use super::super::WebAuthMode;

use super::*;

const TEST_AUTHORITY: &str = "localhost:3737";
const TEST_ORIGIN: &str = "http://localhost:3737";
const TEST_RECOVERY_CODE: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

async fn test_store() -> noema_store::NoemaStore {
    let root = tempfile::tempdir().expect("store root").keep();
    noema_store::NoemaStore::open(&noema_store::StoreConfig::new(root.join("noema.sqlite3")))
        .await
        .expect("test store")
}

async fn web_state(sessions: session::SessionSecurity, auth_mode: WebAuthMode) -> WebState {
    WebState::new(
        noema_api::graphql::GraphqlState::for_tests(),
        test_store().await,
        authority::CanonicalAuthority::from_public_origin(TEST_ORIGIN, "localhost")
            .expect("test authority"),
        sessions,
        auth_mode,
        false,
        Some(test_recovery()),
    )
    .expect("web state")
}

fn test_recovery() -> noema_host::RecoveryCodeStore {
    let root = tempfile::tempdir().expect("recovery root").keep();
    let path = root.join("config.yaml");
    std::fs::write(
        &path,
        format!("web:\n  recovery_code: {TEST_RECOVERY_CODE}\n"),
    )
    .expect("recovery config");
    noema_host::RecoveryCodeStore::open(path).expect("recovery store")
}

async fn test_router() -> Router {
    build_router(
        web_state(
            session::SessionSecurity::for_tests("test-capability"),
            WebAuthMode::Required,
        )
        .await,
    )
}

async fn test_router_without_auth() -> Router {
    build_router(
        web_state(
            session::SessionSecurity::for_tests("test-capability"),
            WebAuthMode::DisabledForDevelopment,
        )
        .await,
    )
}

async fn test_setup_router() -> Router {
    build_router(
        web_state(
            session::SessionSecurity::for_tests("setup"),
            WebAuthMode::Required,
        )
        .await,
    )
}

fn empty_request(method: Method, uri: impl AsRef<str>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri.as_ref())
        .body(Body::empty())
        .expect("request")
}

fn graphql_request(query: &'static str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(query))
        .expect("GraphQL request")
}

fn auth_post(path: &str, cookie: Option<&str>, body: Body) -> Request<Body> {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::ORIGIN, TEST_ORIGIN)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    builder.body(body).expect("auth request")
}

async fn request(
    router: Router,
    mut request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, bytes::Bytes) {
    if !request.headers().contains_key(header::HOST) {
        request
            .headers_mut()
            .insert(header::HOST, HeaderValue::from_static(TEST_AUTHORITY));
    }
    if request.method() == Method::POST
        && request.uri().path() == "/graphql"
        && !request.headers().contains_key(header::ORIGIN)
    {
        request
            .headers_mut()
            .insert(header::ORIGIN, HeaderValue::from_static(TEST_ORIGIN));
    }
    raw_request(router, request).await
}

async fn raw_request(
    router: Router,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, bytes::Bytes) {
    let response = router.oneshot(request).await.expect("router response");
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    (status, headers, body)
}

#[tokio::test]
async fn authority_session_and_removed_bootstrap_boundary() {
    for (method, uri) in [
        (Method::GET, "/"),
        (Method::GET, "/__noema/bootstrap/test-capability"),
        (Method::POST, "/graphql"),
        (Method::GET, "/mcp/oauth/callback"),
        (Method::GET, "/adapter/oauth/callback"),
    ] {
        for host in [None, Some("attacker.invalid:3737")] {
            let mut builder = Request::builder().method(method.clone()).uri(uri);
            if let Some(host) = host {
                builder = builder.header(header::HOST, host);
            }
            let (status, _, _) = raw_request(
                test_router().await,
                builder.body(Body::empty()).expect("request"),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {uri} {host:?}");
        }
    }

    let router = test_router().await;
    let (setup_blocked, _, _) = request(
        router.clone(),
        graphql_request(r#"{"query":"{ __typename }"}"#),
    )
    .await;
    assert_eq!(setup_blocked, StatusCode::FORBIDDEN);

    let (missing_origin, _, _) = raw_request(
        router.clone(),
        Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::HOST, TEST_AUTHORITY)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("GraphQL request"),
    )
    .await;
    assert_eq!(missing_origin, StatusCode::FORBIDDEN);

    for (method, uri) in [
        (Method::POST, "/graphql"),
        (Method::GET, "/graphql/ws"),
        (Method::POST, "/auth/passkey/login/start"),
    ] {
        for origin in [None, Some("http://attacker.invalid:3737")] {
            let mut builder = Request::builder()
                .method(method.clone())
                .uri(uri)
                .header(header::HOST, TEST_AUTHORITY);
            if let Some(origin) = origin {
                builder = builder.header(header::ORIGIN, origin);
            }
            let (status, _, _) = raw_request(
                test_router().await,
                builder.body(Body::empty()).expect("origin request"),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri} {origin:?}");
        }
    }

    let cookie = authenticate(router.clone()).await;
    let set_cookie = cookie.as_str();
    assert!(set_cookie.starts_with("noema.sid="));

    for capability in ["test-capability", "wrong"] {
        let (status, _, body) = request(
            router.clone(),
            empty_request(Method::GET, format!("/__noema/bootstrap/{capability}")),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, NOT_FOUND);
    }
}

#[tokio::test]
async fn recovery_authorizes_one_setup_session_without_authenticating_it() {
    let router = test_setup_router().await;
    let (forbidden, _, _) = request(
        router.clone(),
        auth_post("/auth/passkey/register/start", None, Body::empty()),
    )
    .await;
    assert_eq!(forbidden, StatusCode::FORBIDDEN);

    let (recovery_status, headers, _) = request(
        router.clone(),
        auth_post(
            "/auth/recovery",
            None,
            Body::from(format!(r#"{{"code":"{TEST_RECOVERY_CODE}"}}"#)),
        ),
    )
    .await;
    assert_eq!(recovery_status, StatusCode::NO_CONTENT);
    let cookie = headers[header::SET_COOKIE]
        .to_str()
        .expect("cookie")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string();

    let (graphql_status, _, _) = request(
        router.clone(),
        Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, &cookie)
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("GraphQL request"),
    )
    .await;
    assert_eq!(graphql_status, StatusCode::FORBIDDEN);

    let (start_status, _, start_body) = request(
        router.clone(),
        auth_post("/auth/passkey/register/start", Some(&cookie), Body::empty()),
    )
    .await;
    assert_eq!(start_status, StatusCode::OK);
    let start: serde_json::Value = serde_json::from_slice(&start_body).expect("start JSON");
    assert!(start["ceremonyId"].is_string());
    assert!(start["options"]["publicKey"]["challenge"].is_string());
}

#[tokio::test]
async fn development_mode_keeps_canonical_host_and_origin_checks() {
    let (status, _, _) = raw_request(
        test_router_without_auth().await,
        Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::HOST, "192.0.2.10:3737")
            .header(header::ORIGIN, "http://192.0.2.10:3737")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("GraphQL request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn development_auth_bypass_allows_graphql_without_bootstrap() {
    let router = test_router_without_auth().await;
    let (status, _, body) = request(
        router.clone(),
        graphql_request(r#"{"query":"{ __typename }"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("GraphQL JSON"),
        json!({"data": {"__typename": "QueryRoot"}})
    );
    let (recovery_status, _, _) = request(
        router,
        auth_post(
            "/auth/recovery",
            None,
            Body::from(format!(r#"{{"code":"{TEST_RECOVERY_CODE}"}}"#)),
        ),
    )
    .await;
    assert_eq!(recovery_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn recovery_keeps_origin_checks_and_rotates_rejected_candidates() {
    let router = test_setup_router().await;
    let (missing_origin, _, _) = raw_request(
        router.clone(),
        Request::builder()
            .method(Method::POST)
            .uri("/auth/recovery")
            .header(header::HOST, TEST_AUTHORITY)
            .header(header::AUTHORIZATION, "Bearer ignored")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(format!(r#"{{"code":"{TEST_RECOVERY_CODE}"}}"#)))
            .expect("recovery request"),
    )
    .await;
    assert_eq!(missing_origin, StatusCode::FORBIDDEN);

    let (wrong, _, _) = request(
        router.clone(),
        auth_post("/auth/recovery", None, Body::from(r#"{"code":"wrong"}"#)),
    )
    .await;
    assert_eq!(wrong, StatusCode::UNAUTHORIZED);

    let (stale, _, _) = request(
        router,
        auth_post(
            "/auth/recovery",
            None,
            Body::from(format!(r#"{{"code":"{TEST_RECOVERY_CODE}"}}"#)),
        ),
    )
    .await;
    assert_eq!(stale, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn router_serves_schema_graphiql_and_spa_fallback() {
    let (disabled_status, _, _) = request(
        test_router_without_auth().await,
        empty_request(Method::GET, "/graphql"),
    )
    .await;
    assert_eq!(disabled_status, StatusCode::NOT_FOUND);

    let mut graphiql_state = web_state(
        session::SessionSecurity::for_tests("test-capability"),
        WebAuthMode::DisabledForDevelopment,
    )
    .await;
    graphiql_state.graphiql_enabled = true;
    for (router, uri, expected_fragment) in [
        (build_router(graphiql_state), "/graphql", "GraphiQL"),
        (
            test_router_without_auth().await,
            "/graphql/schema.graphql",
            "type QueryRoot",
        ),
    ] {
        let (status, headers, body) = request(router, empty_request(Method::GET, uri)).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(
            headers[header::CONTENT_TYPE]
                .to_str()
                .expect("content type")
                .starts_with("text/"),
            "{uri}"
        );
        assert!(
            String::from_utf8_lossy(&body).contains(expected_fragment),
            "{uri}"
        );
    }

    let (status, headers, body) = request(
        test_router_without_auth().await,
        empty_request(Method::GET, "/memory/thread"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.is_empty());
    assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
}

#[tokio::test]
async fn pwa_asset_responses_use_release_safe_headers() {
    for (name, content_type) in [
        ("sw.js", "application/javascript; charset=utf-8"),
        (
            "manifest.webmanifest",
            "application/manifest+json; charset=utf-8",
        ),
        ("pwa-192x192.png", "image/png"),
        ("pwa-512x512.png", "image/png"),
        ("apple-touch-icon.png", "image/png"),
    ] {
        let response = asset_response(super::super::assets::EmbeddedAsset {
            content_type,
            cache_control: "no-cache",
            service_worker_allowed: name == "sw.js",
            body: Cow::Borrowed(b"pwa asset"),
        });
        assert_eq!(response.status(), StatusCode::OK, "{name}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            content_type,
            "{name}"
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "no-cache",
            "{name}"
        );
        assert_eq!(
            response
                .headers()
                .get("service-worker-allowed")
                .and_then(|value| value.to_str().ok()),
            (name == "sw.js").then_some("/"),
            "{name}"
        );
    }
}

#[tokio::test]
async fn private_and_network_endpoints_remain_excluded_from_http_caches() {
    for uri in [
        "/auth/status",
        "/auth/recovery",
        "/artifacts/versions/missing/download",
    ] {
        let (_, headers, _) = request(
            test_router_without_auth().await,
            empty_request(Method::GET, uri),
        )
        .await;
        assert_eq!(headers[header::CACHE_CONTROL], "no-store", "{uri}");
        assert!(!headers.contains_key("service-worker-allowed"), "{uri}");
    }
}

#[tokio::test]
async fn authenticated_http_and_websocket_ignore_client_identity_metadata() {
    use futures_util::{SinkExt, StreamExt};

    let router = test_router().await;
    let cookie = authenticate(router.clone()).await;
    let (status, headers, body) = request(
        router,
        Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::COOKIE, &cookie)
            .body(Body::from(
                r#"{"query":"{ testRequestPrincipal }","extensions":{"principal":{"subjectId":"attacker"}}}"#,
            ))
            .expect("GraphQL principal request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("GraphQL JSON"),
        json!({"data": {"testRequestPrincipal": "human:local"}})
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let authority = authority::CanonicalAuthority::from_public_origin(
        &format!("http://localhost:{}", address.port()),
        "localhost",
    )
    .expect("authority");
    let state = WebState::new(
        noema_api::graphql::GraphqlState::for_tests(),
        test_store().await,
        authority,
        session::SessionSecurity::for_tests("ws-test-capability"),
        WebAuthMode::Required,
        false,
        Some(test_recovery()),
    )
    .expect("web state");
    let server = tokio::spawn(async move {
        axum::serve(listener, build_router(state))
            .await
            .expect("serve test router");
    });

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("HTTP client");
    let bootstrap = client
        .post(format!(
            "http://localhost:{}/__test/authenticate",
            address.port()
        ))
        .send()
        .await
        .expect("test authentication request");
    assert_eq!(bootstrap.status(), reqwest::StatusCode::NO_CONTENT);
    let ws_cookie = bootstrap
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .expect("session cookie")
        .to_str()
        .expect("session cookie text")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string();

    let mut ws_request = format!("ws://localhost:{}/graphql/ws", address.port())
        .into_client_request()
        .expect("WebSocket request");
    ws_request.headers_mut().insert(
        "origin",
        format!("http://localhost:{}", address.port())
            .parse()
            .expect("origin header"),
    );
    ws_request
        .headers_mut()
        .insert("cookie", ws_cookie.parse().expect("cookie header"));
    ws_request.headers_mut().insert(
        "sec-websocket-protocol",
        "graphql-transport-ws".parse().expect("protocol header"),
    );
    let (mut socket, response) = connect_async(ws_request)
        .await
        .expect("WebSocket handshake");
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);

    socket
        .send(Message::Text(
            json!({
                "type": "connection_init",
                "payload": {"principal": {"subjectId": "attacker"}}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("connection init");
    let ack = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("connection ack timeout")
        .expect("connection ack frame")
        .expect("connection ack");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(ack.to_text().expect("ack text"))
            .expect("ack JSON"),
        json!({"type": "connection_ack"})
    );

    socket
        .send(Message::Text(
            json!({
                "id": "principal",
                "type": "subscribe",
                "payload": {
                    "query": "subscription { testRequestPrincipal }",
                    "extensions": {"principal": {"subjectId": "attacker"}}
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("subscription request");
    let next = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("subscription timeout")
        .expect("subscription frame")
        .expect("subscription result");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(next.to_text().expect("result text"))
            .expect("result JSON"),
        json!({
            "id": "principal",
            "type": "next",
            "payload": {"data": {"testRequestPrincipal": "human:local"}}
        })
    );

    tokio::time::timeout(std::time::Duration::from_secs(2), socket.close(None))
        .await
        .expect("WebSocket close timeout")
        .expect("close WebSocket");
    server.abort();
}

#[tokio::test]
async fn client_bearer_authorizes_http_and_ws_without_browser_origin_and_revocation_closes_ws() {
    use futures_util::{SinkExt, StreamExt};

    let store = test_store().await;
    let secret = [6_u8; 32];
    let hash = digest::digest(&digest::SHA256, &secret);
    store
        .insert_client(
            "client-one",
            "human:local",
            "Native client",
            hash.as_ref().try_into().expect("digest length"),
        )
        .await
        .expect("insert client");
    let bearer = format!("Bearer client-one.{}", URL_SAFE_NO_PAD.encode(secret));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let authority = authority::CanonicalAuthority::from_public_origin(
        &format!("http://localhost:{}", address.port()),
        "localhost",
    )
    .expect("authority");
    let state = WebState::new(
        noema_api::graphql::GraphqlState::for_tests(),
        store.clone(),
        authority,
        session::SessionSecurity::for_tests("client-test-capability"),
        WebAuthMode::Required,
        false,
        Some(test_recovery()),
    )
    .expect("web state");
    let server = tokio::spawn(async move {
        axum::serve(listener, build_router(state))
            .await
            .expect("serve test router");
    });
    let client = reqwest::Client::new();
    let url = format!("http://localhost:{}/graphql", address.port());
    let setup_blocked = client
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, &bearer)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(r#"{"query":"{ testRequestPrincipal }"}"#)
        .send()
        .await
        .expect("setup-barrier bearer request");
    assert_eq!(setup_blocked.status(), reqwest::StatusCode::FORBIDDEN);
    store
        .insert_local_human_passkey("test-passkey", r#"{"test":true}"#)
        .await
        .expect("insert test passkey");
    let response = client
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, &bearer)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(r#"{"query":"{ testRequestPrincipal }"}"#)
        .send()
        .await
        .expect("bearer GraphQL request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response
            .json::<serde_json::Value>()
            .await
            .expect("GraphQL JSON"),
        json!({"data": {"testRequestPrincipal": "human:local"}})
    );

    let response = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(r#"{"query":"{ __typename }"}"#)
        .send()
        .await
        .expect("browser-origin request");
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);

    let artifact_response = client
        .get(format!(
            "http://localhost:{}/artifacts/versions/missing/download",
            address.port()
        ))
        .header(reqwest::header::AUTHORIZATION, &bearer)
        .send()
        .await
        .expect("bearer artifact request");
    // The test GraphQL state has no filesystem artifact service, but reaching
    // its internal error proves bearer auth passed the route boundary (an
    // unauthenticated request is rejected before this point).
    assert_eq!(
        artifact_response.status(),
        reqwest::StatusCode::INTERNAL_SERVER_ERROR
    );

    let mut ws_request = format!("ws://localhost:{}/graphql/ws", address.port())
        .into_client_request()
        .expect("WebSocket request");
    ws_request.headers_mut().insert(
        "authorization",
        bearer.parse().expect("authorization header"),
    );
    ws_request.headers_mut().insert(
        "sec-websocket-protocol",
        "graphql-transport-ws".parse().expect("protocol header"),
    );
    let (mut socket, response) = connect_async(ws_request)
        .await
        .expect("WebSocket handshake");
    assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
    socket
        .send(Message::Text(
            json!({"type": "connection_init"}).to_string().into(),
        ))
        .await
        .expect("connection init");
    let _ack = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("connection ack timeout")
        .expect("connection ack frame")
        .expect("connection ack");
    socket
        .send(Message::Text(
            json!({
                "id": "principal",
                "type": "subscribe",
                "payload": {"query": "subscription { testRequestPrincipal }"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("subscription request");
    let _next = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("subscription timeout")
        .expect("subscription frame")
        .expect("subscription result");
    let _complete = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("subscription complete timeout")
        .expect("subscription complete frame")
        .expect("subscription complete");
    store
        .revoke_client("human:local", "client-one")
        .await
        .expect("revoke client");
    let closed = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("revocation close timeout");
    assert!(
        matches!(closed, None | Some(Err(_)) | Some(Ok(Message::Close(_)))),
        "socket remained open: {closed:?}"
    );
    server.abort();
}

async fn authenticate(router: Router) -> String {
    let (status, headers, _) =
        request(router, empty_request(Method::POST, "/__test/authenticate")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    headers[header::SET_COOKIE]
        .to_str()
        .expect("session cookie")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string()
}

#[tokio::test]
async fn router_preserves_oauth_and_plain_text_not_found_responses() {
    let authority = authority::CanonicalAuthority::from_public_origin(TEST_ORIGIN, "localhost")
        .expect("authority");
    let router = test_router().await;
    drop(authenticate(router.clone()).await);
    assert_eq!(
        oauth_callback_url(
            &authority,
            "/mcp/oauth/callback",
            "attemptId=1&host=attacker.invalid"
        ),
        "http://localhost:3737/mcp/oauth/callback?attemptId=1&host=attacker.invalid"
    );
    let (oauth_status, _, oauth_body) = request(
        router.clone(),
        empty_request(Method::GET, "/mcp/oauth/callback"),
    )
    .await;
    assert_eq!(oauth_status, StatusCode::BAD_REQUEST);
    assert_eq!(oauth_body, "missing OAuth callback query");
    let (adapter_status, _, adapter_body) = request(
        router.clone(),
        empty_request(
            Method::GET,
            "/adapter/oauth/callback?state=missing&code=hidden",
        ),
    )
    .await;
    assert_eq!(adapter_status, StatusCode::BAD_REQUEST);
    assert_eq!(
        adapter_body,
        "<!doctype html><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Noema OAuth</title><main><p>Noema could not finish activating this connection. Return to Noema to review its status or try again.</p><p><a href=\"/\">Return to Noema</a></p></main>"
    );
    let oversized = format!(
        "/adapter/oauth/callback?state={}",
        "x".repeat(MAX_OAUTH_QUERY_BYTES + 1)
    );
    let (oversized_status, _, oversized_body) =
        request(router.clone(), empty_request(Method::GET, &oversized)).await;
    assert_eq!(oversized_status, StatusCode::BAD_REQUEST);
    assert_eq!(oversized_body, "invalid OAuth callback query");
    for (method, uri) in [
        (Method::HEAD, "/graphql"),
        (Method::PUT, "/graphql/schema.graphql"),
        (Method::HEAD, "/graphql/ws"),
        (Method::PUT, "/mcp/oauth/callback"),
        (Method::PUT, "/adapter/oauth/callback"),
        (Method::HEAD, "/artifacts/versions/missing/download"),
        (Method::PUT, "/memory"),
    ] {
        let (status, headers, body) =
            request(router.clone(), empty_request(method.clone(), uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
        assert_eq!(
            headers[header::CONTENT_TYPE],
            "text/plain; charset=utf-8",
            "{method} {uri}"
        );
        if method != Method::HEAD {
            assert_eq!(body, NOT_FOUND, "{method} {uri}");
        }
    }
    let (status, _, body) = request(
        router,
        empty_request(Method::GET, "/artifacts/versions/missing/download"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.is_empty());
}

#[tokio::test]
async fn artifact_download_adapter_sanitizes_response_headers() {
    let response = artifact_download_response(noema_api::graphql::AuthorizedArtifactDownload {
        filename: "report\"\r\nx-injected: yes.md".to_owned(),
        media_type: "text/markdown\r\nx-injected: yes".to_owned(),
        bytes: b"report".to_vec(),
    });
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/octet-stream"
    );
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"report\\\"__x-injected: yes.md\""
    );
    assert!(!response.headers().contains_key("x-injected"));
}
