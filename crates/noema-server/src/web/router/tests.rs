use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use tower::ServiceExt;

use super::super::WebAuthMode;

use super::*;

const TEST_AUTHORITY: &str = "127.0.0.1:3737";

fn web_state(graphql_state: noema_core::graphql::GraphqlState) -> WebState {
    WebState::new(
        graphql_state,
        authority::CanonicalAuthority::from_socket_addr(
            TEST_AUTHORITY.parse().expect("test authority"),
        ),
        session::SessionSecurity::for_tests("test-capability"),
        WebAuthMode::Required,
    )
}

fn test_router() -> Router {
    build_router(web_state(noema_core::graphql::GraphqlState::for_tests()))
}

fn test_router_without_auth() -> Router {
    build_router(WebState::new(
        noema_core::graphql::GraphqlState::for_tests(),
        authority::CanonicalAuthority::from_socket_addr(
            TEST_AUTHORITY.parse().expect("test authority"),
        ),
        session::SessionSecurity::for_tests("test-capability"),
        WebAuthMode::DisabledForDevelopment,
    ))
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
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:3737"),
        );
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
async fn authority_session_and_bootstrap_boundary() {
    for (method, uri) in [
        (Method::GET, "/"),
        (Method::GET, "/__noema/bootstrap/test-capability"),
        (Method::POST, "/graphql"),
        (Method::GET, "/mcp/oauth/callback"),
    ] {
        for host in [None, Some("attacker.invalid:3737")] {
            let mut builder = Request::builder().method(method.clone()).uri(uri);
            if let Some(host) = host {
                builder = builder.header(header::HOST, host);
            }
            let (status, _, _) =
                raw_request(test_router(), builder.body(Body::empty()).expect("request")).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {uri} {host:?}");
        }
    }

    let router = test_router();
    let (unauthorized, _, _) = request(
        router.clone(),
        Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("GraphQL request"),
    )
    .await;
    assert_eq!(unauthorized, StatusCode::UNAUTHORIZED);

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

    for (method, uri) in [(Method::POST, "/graphql"), (Method::GET, "/graphql/ws")] {
        for origin in [None, Some("http://attacker.invalid:3737")] {
            let mut builder = Request::builder()
                .method(method.clone())
                .uri(uri)
                .header(header::HOST, TEST_AUTHORITY);
            if let Some(origin) = origin {
                builder = builder.header(header::ORIGIN, origin);
            }
            let (status, _, _) = raw_request(
                test_router(),
                builder.body(Body::empty()).expect("origin request"),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri} {origin:?}");
        }
    }

    let (bootstrap_status, headers, _) = request(
        router.clone(),
        Request::builder()
            .uri("/__noema/bootstrap/test-capability")
            .body(Body::empty())
            .expect("bootstrap request"),
    )
    .await;
    assert_eq!(bootstrap_status, StatusCode::SEE_OTHER);
    assert_eq!(headers[header::LOCATION], "/");
    let set_cookie = headers[header::SET_COOKIE].to_str().expect("cookie");
    assert!(set_cookie.starts_with("noema.sid="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
    assert!(set_cookie.contains("Path=/"));

    for capability in ["test-capability", "wrong"] {
        let (status, _, body) = request(
            router.clone(),
            Request::builder()
                .uri(format!("/__noema/bootstrap/{capability}"))
                .body(Body::empty())
                .expect("bootstrap replay"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, NOT_FOUND);
    }
}

#[tokio::test]
async fn development_auth_bypass_allows_graphql_without_bootstrap() {
    let (status, _, body) = request(
        test_router_without_auth(),
        Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("GraphQL request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("GraphQL JSON"),
        json!({"data": {"__typename": "QueryRoot"}})
    );
}

#[tokio::test]
async fn authenticated_http_and_websocket_ignore_client_identity_metadata() {
    use futures_util::{SinkExt, StreamExt};

    let router = test_router();
    let cookie = authenticate(router.clone()).await;
    let (status, _, body) = authenticated_request(
        router,
        &cookie,
        Request::builder()
            .method(Method::POST)
            .uri("/graphql")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("GraphQL principal request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("GraphQL JSON"),
        json!({"data": {"__typename": "QueryRoot"}})
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let address = listener.local_addr().expect("test server address");
    let authority = authority::CanonicalAuthority::from_socket_addr(address);
    let state = WebState::new(
        noema_core::graphql::GraphqlState::for_tests(),
        authority,
        session::SessionSecurity::for_tests("ws-test-capability"),
        WebAuthMode::Required,
    );
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
        .get(format!(
            "http://{address}/__noema/bootstrap/ws-test-capability"
        ))
        .send()
        .await
        .expect("bootstrap request");
    assert_eq!(bootstrap.status(), reqwest::StatusCode::SEE_OTHER);
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

    let mut ws_request = format!("ws://{address}/graphql/ws")
        .into_client_request()
        .expect("WebSocket request");
    ws_request.headers_mut().insert(
        "origin",
        format!("http://{address}").parse().expect("origin header"),
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
                "payload": {"query": "{ __typename }"}
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
            "payload": {"data": {"__typename": "QueryRoot"}}
        })
    );

    tokio::time::timeout(std::time::Duration::from_secs(2), socket.close(None))
        .await
        .expect("WebSocket close timeout")
        .expect("close WebSocket");
    server.abort();
}

#[test]
fn oauth_callback_url_uses_canonical_authority() {
    let authority =
        authority::CanonicalAuthority::from_socket_addr(TEST_AUTHORITY.parse().expect("authority"));
    assert_eq!(
        oauth_callback_url(&authority, "attemptId=1&host=attacker.invalid"),
        "http://127.0.0.1:3737/mcp/oauth/callback?attemptId=1&host=attacker.invalid"
    );
}

async fn authenticate(router: Router) -> String {
    let (status, headers, _) = request(
        router,
        Request::builder()
            .uri("/__noema/bootstrap/test-capability")
            .body(Body::empty())
            .expect("bootstrap request"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    headers[header::SET_COOKIE]
        .to_str()
        .expect("session cookie")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string()
}

async fn authenticated_request(
    router: Router,
    cookie: &str,
    mut request_value: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, bytes::Bytes) {
    request_value.headers_mut().insert(
        header::COOKIE,
        HeaderValue::from_str(cookie).expect("cookie header"),
    );
    request(router, request_value).await
}

#[tokio::test]
async fn router_serves_graphql_post_from_startup_schema() {
    let router = test_router();
    let cookie = authenticate(router.clone()).await;
    let (status, headers, body) = authenticated_request(
        router,
        &cookie,
        Request::builder()
            .method("POST")
            .uri("/graphql")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("GraphQL request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("GraphQL JSON"),
        serde_json::json!({"data": {"__typename": "QueryRoot"}})
    );
}

#[tokio::test]
async fn router_serves_schema_and_graphiql() {
    let schema_router = test_router();
    let schema_cookie = authenticate(schema_router.clone()).await;
    let (schema_status, _, schema) = authenticated_request(
        schema_router,
        &schema_cookie,
        Request::builder()
            .uri("/graphql/schema.graphql")
            .body(Body::empty())
            .expect("schema request"),
    )
    .await;
    assert_eq!(schema_status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&schema).contains("type QueryRoot"));

    let graphiql_router = test_router();
    let graphiql_cookie = authenticate(graphiql_router.clone()).await;
    let (graphiql_status, headers, graphiql) = authenticated_request(
        graphiql_router,
        &graphiql_cookie,
        Request::builder()
            .uri("/graphql")
            .body(Body::empty())
            .expect("GraphiQL request"),
    )
    .await;
    assert_eq!(graphiql_status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
    assert!(String::from_utf8_lossy(&graphiql).contains("/graphql/ws"));
}

#[tokio::test]
async fn router_preserves_oauth_and_not_found_responses() {
    let (oauth_status, _, oauth_body) = request(
        test_router(),
        Request::builder()
            .uri("/mcp/oauth/callback")
            .body(Body::empty())
            .expect("OAuth request"),
    )
    .await;
    assert_eq!(oauth_status, StatusCode::BAD_REQUEST);
    assert_eq!(oauth_body, "missing OAuth callback query");

    let artifact_router = test_router();
    let cookie = authenticate(artifact_router.clone()).await;
    let (artifact_status, _, artifact_body) = authenticated_request(
        artifact_router,
        &cookie,
        Request::builder()
            .uri("/artifacts/versions/invalid:slug/download")
            .body(Body::empty())
            .expect("artifact request"),
    )
    .await;
    assert_eq!(artifact_status, StatusCode::NOT_FOUND);
    assert_eq!(artifact_body, NOT_FOUND);

    let (missing_status, _, missing_body) = request(
        test_router(),
        Request::builder()
            .uri("/assets/missing.css")
            .body(Body::empty())
            .expect("missing request"),
    )
    .await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    assert_eq!(missing_body, NOT_FOUND);
}

#[tokio::test]
async fn unsupported_methods_return_plain_text_not_found() {
    for (method, uri) in [
        (Method::HEAD, "/graphql"),
        (Method::PUT, "/graphql/schema.graphql"),
        (Method::HEAD, "/graphql/ws"),
        (Method::PUT, "/mcp/oauth/callback"),
        (Method::HEAD, "/artifacts/versions/missing/download"),
        (Method::PUT, "/memory"),
    ] {
        let (status, headers, body) = request(
            test_router(),
            Request::builder()
                .method(method.clone())
                .uri(uri)
                .body(Body::empty())
                .expect("unsupported method request"),
        )
        .await;
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
}

#[tokio::test]
async fn artifact_download_rejects_missing_session() {
    let (status, _, body) = request(
        test_router(),
        Request::builder()
            .uri("/artifacts/artifact_version:missing/download")
            .body(Body::empty())
            .expect("artifact request"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body.is_empty());
}

#[tokio::test]
async fn artifact_download_adapter_sanitizes_response_headers() {
    let response = artifact_download_response(noema_core::graphql::AuthorizedArtifactDownload {
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

#[tokio::test]
async fn official_graphql_subscription_service_accepts_websocket_upgrade() {
    let router = test_router();
    let cookie = authenticate(router.clone()).await;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("serve router");
    });
    let mut client = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect client");
    let request = format!(
        "GET /graphql/ws HTTP/1.1\r\nHost: {TEST_AUTHORITY}\r\nOrigin: http://{TEST_AUTHORITY}\r\nCookie: {cookie}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: graphql-transport-ws\r\n\r\n"
    );
    client
        .write_all(request.as_bytes())
        .await
        .expect("write upgrade request");
    let mut response = [0_u8; 1024];
    let read = client
        .read(&mut response)
        .await
        .expect("read upgrade response");
    let response = String::from_utf8_lossy(&response[..read]);
    assert!(response.starts_with("HTTP/1.1 101 Switching Protocols\r\n"));
    assert!(
        response
            .to_ascii_lowercase()
            .contains("sec-websocket-protocol: graphql-transport-ws")
    );
    drop(client);
    let _ = shutdown_tx.send(());
    server.await.expect("server task");
}

#[tokio::test]
async fn router_serves_known_asset_and_spa_fallback() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web-assets");
    std::fs::create_dir_all(&dir).expect("create asset dir");
    let index_path = dir.join("index.html");
    let created_index = !index_path.exists();
    std::fs::write(dir.join("__noema_router_test.js"), b"export {};").expect("write asset");
    if created_index {
        std::fs::write(&index_path, b"<!doctype html><title>Noema</title>").expect("write SPA");
    }

    let (asset_status, _, asset_body) = request(
        test_router(),
        Request::builder()
            .uri("/assets/__noema_router_test.js")
            .body(Body::empty())
            .expect("asset request"),
    )
    .await;
    assert_eq!(asset_status, StatusCode::OK);
    assert_eq!(asset_body, "export {};");

    let (spa_status, _, spa_body) = request(
        test_router(),
        Request::builder()
            .uri("/memory")
            .body(Body::empty())
            .expect("SPA request"),
    )
    .await;
    assert_eq!(spa_status, StatusCode::OK);
    assert!(!spa_body.is_empty());

    std::fs::remove_file(dir.join("__noema_router_test.js")).expect("remove asset");
    if created_index {
        std::fs::remove_file(index_path).expect("remove SPA entry");
    }
}
