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
use super::super::WebFiles;

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

async fn web_state(auth_mode: WebAuthMode) -> WebState {
    web_state_with_store(test_store().await, auth_mode)
}

fn web_state_with_store(store: noema_store::NoemaStore, auth_mode: WebAuthMode) -> WebState {
    let sessions = session::SessionSecurity::for_tests(store.clone());
    WebState::new(
        noema_api::graphql::GraphqlState::for_tests(),
        store,
        authority::CanonicalAuthority::from_public_origin(TEST_ORIGIN, "localhost")
            .expect("test authority"),
        sessions,
        auth_mode,
        false,
        WebFiles::new(Some(test_recovery()), test_paths()),
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

fn test_paths() -> noema_home::NoemaPaths {
    noema_home::NoemaPaths::from_noema_home(tempfile::tempdir().expect("Noema home").keep())
        .expect("Noema paths")
}

async fn test_router() -> Router {
    build_router(web_state(WebAuthMode::Required).await)
}

async fn test_router_without_auth() -> Router {
    build_router(web_state(WebAuthMode::DisabledForDevelopment).await)
}

async fn test_setup_router() -> Router {
    build_router(web_state(WebAuthMode::Required).await)
}

#[tokio::test]
async fn favicon_route_requires_authentication_and_serves_cached_images() {
    let state = web_state(WebAuthMode::Required).await;
    super::super::favicons::seed_icon(&state.favicons, "example.com", b"png");
    let router = build_router(state);
    let cookie = authenticate(router.clone()).await;

    let (status, _, _) = request(
        router.clone(),
        empty_request(Method::GET, "/favicons/example.com"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let favicon_request = || {
        Request::get("/favicons/example.com")
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("favicon request")
    };
    let (status, headers, body) = request(router.clone(), favicon_request()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(headers[header::CACHE_CONTROL], "private, max-age=86400");
    assert_eq!(body.as_ref(), b"png");

    let etag = headers[header::ETAG].clone();
    let mut conditional = favicon_request();
    conditional
        .headers_mut()
        .insert(header::IF_NONE_MATCH, etag);
    let (status, _, body) = request(router.clone(), conditional).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());

    let mut invalid = empty_request(Method::GET, "/favicons/127.0.0.1");
    invalid.headers_mut().insert(
        header::COOKIE,
        cookie.parse().expect("session cookie header"),
    );
    let (status, _, _) = request(router, invalid).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
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

async fn passkey_remove_status(
    router: Router,
    cookie: Option<&str>,
    credential_id: &str,
) -> StatusCode {
    request(
        router.clone(),
        auth_post(
            "/auth/passkey/remove",
            cookie,
            Body::from(format!(r#"{{"credentialId":"{credential_id}"}}"#)),
        ),
    )
    .await
    .0
}

async fn authenticated_graphql_status(router: Router, cookie: &str) -> StatusCode {
    request(
        router.clone(),
        Request::post("/graphql")
            .header(header::COOKIE, cookie)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"query":"{ __typename }"}"#))
            .expect("authenticated GraphQL request"),
    )
    .await
    .0
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

    let secure_store = test_store().await;
    let secure_state = WebState::new(
        noema_api::graphql::GraphqlState::for_tests(),
        secure_store.clone(),
        authority::CanonicalAuthority::from_public_origin("https://noema.example", "noema.example")
            .expect("secure authority"),
        session::SessionSecurity::for_tests(secure_store),
        WebAuthMode::Required,
        false,
        WebFiles::new(Some(test_recovery()), test_paths()),
    )
    .expect("secure state");
    let (_, secure_headers, _) = raw_request(
        build_router(secure_state),
        Request::post("/__test/authenticate")
            .header(header::HOST, "noema.example")
            .body(Body::empty())
            .expect("secure authentication"),
    )
    .await;
    assert!(
        secure_headers[header::SET_COOKIE]
            .to_str()
            .expect("secure cookie")
            .starts_with("__Host-noema.sid=")
    );

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
    let (initial_status, _, initial_body) = request(
        router.clone(),
        auth_post("/auth/passkey/register/start", None, Body::empty()),
    )
    .await;
    assert_eq!(initial_status, StatusCode::OK);
    let initial: serde_json::Value =
        serde_json::from_slice(&initial_body).expect("initial start JSON");
    assert!(initial["ceremonyId"].is_string());

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
    let router = test_router_without_auth().await;
    let (status, _, _) = raw_request(
        router.clone(),
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

    let (approval_status, _, _) = raw_request(
        router.clone(),
        Request::builder()
            .method(Method::POST)
            .uri("/oauth/authorize")
            .header(header::HOST, TEST_AUTHORITY)
            .header(header::AUTHORIZATION, "Bearer ignored")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from("csrf=ignored&decision=approve"))
            .expect("OAuth approval"),
    )
    .await;
    assert_eq!(approval_status, StatusCode::BAD_REQUEST);

    let (logout_status, _, _) = raw_request(
        router,
        Request::builder()
            .method(Method::POST)
            .uri("/auth/logout")
            .header(header::HOST, TEST_AUTHORITY)
            .header(header::AUTHORIZATION, "Bearer ignored")
            .body(Body::empty())
            .expect("logout request"),
    )
    .await;
    assert_eq!(logout_status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn browser_authentication_precedes_graphql_parsing() {
    let store = test_store().await;
    store
        .insert_local_human_passkey("test-passkey", r#"{"test":true}"#)
        .await
        .expect("insert passkey");
    let router = build_router(web_state_with_store(store, WebAuthMode::Required));
    assert_eq!(
        request(
            router.clone(),
            auth_post("/auth/passkey/register/start", None, Body::empty()),
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let malformed_request = Request::builder()
        .method(Method::POST)
        .uri("/graphql")
        .header(header::ORIGIN, TEST_ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"))
        .expect("malformed GraphQL request");

    assert_eq!(
        request(router, malformed_request).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn native_authorization_resumes_after_recent_passkey_authentication() {
    let router = test_router().await;
    let _ = authenticate(router.clone()).await;
    let authorization = "/oauth/authorize?client_id=noema-desktop%3Aabcdefghijklmnop&redirect_uri=http%3A%2F%2F127.0.0.1%3A49152%2Foauth%2Fcallback&response_type=code&state=ssssssssssssssssssssssssssssssss&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256";
    let (redirect, headers, _) =
        request(router.clone(), empty_request(Method::GET, authorization)).await;
    assert_eq!(redirect, StatusCode::SEE_OTHER);
    assert_eq!(
        headers[header::LOCATION].to_str().expect("resume location"),
        "/?native_authorization=resume"
    );
    let pending_cookie = headers[header::SET_COOKIE]
        .to_str()
        .expect("pending cookie")
        .split(';')
        .next()
        .expect("pending cookie pair")
        .to_string();
    let (authenticated, headers, _) = request(
        router.clone(),
        Request::post("/__test/authenticate")
            .header(header::COOKIE, pending_cookie)
            .body(Body::empty())
            .expect("authenticate request"),
    )
    .await;
    assert_eq!(authenticated, StatusCode::NO_CONTENT);
    let recent_cookie = headers[header::SET_COOKIE]
        .to_str()
        .expect("recent cookie")
        .split(';')
        .next()
        .expect("recent cookie pair")
        .to_string();
    let (resumed, _, body) = request(
        router,
        Request::get("/oauth/authorize")
            .header(header::COOKIE, recent_cookie)
            .body(Body::empty())
            .expect("resume request"),
    )
    .await;
    assert_eq!(resumed, StatusCode::OK);
    assert!(
        String::from_utf8_lossy(&body).contains("<title>Connect Noema Desktop · Noema</title>")
    );
}

#[tokio::test]
async fn ios_authorization_page_alone_permits_native_form_navigation() {
    let router = test_router().await;
    let cookie = authenticate(router.clone()).await;
    let authorization = "/oauth/authorize?client_id=noema-ios%3Aabcdefghijklmnop&redirect_uri=noema%3A%2F%2Foauth%2Fcallback&response_type=code&state=ssssssssssssssssssssssssssssssss&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256";
    let (status, headers, _) = request(
        router.clone(),
        Request::get(authorization)
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("iOS authorization request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_SECURITY_POLICY], IOS_OAUTH_CSP);

    let (_, default_headers, _) = request(
        router,
        Request::get("/auth/status")
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .expect("default CSP request"),
    )
    .await;
    assert_eq!(
        default_headers[header::CONTENT_SECURITY_POLICY],
        DEFAULT_CSP
    );
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
async fn passkey_management_requires_auth_and_revokes_affected_sessions() {
    let store = test_store().await;
    store
        .insert_local_human_passkey("second-passkey", r#"{"test":true}"#)
        .await
        .expect("second passkey");
    let router = build_router(web_state_with_store(store, WebAuthMode::Required));
    assert_eq!(
        passkey_remove_status(router.clone(), None, "second-passkey").await,
        StatusCode::FORBIDDEN
    );
    let cookie = authenticate(router.clone()).await;

    let (list_status, _, list_body) = request(
        router.clone(),
        Request::get("/auth/passkeys")
            .header(header::COOKIE, &cookie)
            .body(Body::empty())
            .expect("list passkeys"),
    )
    .await;
    assert_eq!(list_status, StatusCode::OK);
    let list: serde_json::Value = serde_json::from_slice(&list_body).expect("passkey list");
    assert_eq!(
        list,
        json!([
            {"credentialId": "second-passkey", "current": false},
            {"credentialId": "test-passkey", "current": true}
        ])
    );
    assert_eq!(
        passkey_remove_status(router.clone(), Some(&cookie), "test-passkey").await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        authenticated_graphql_status(router.clone(), &cookie).await,
        StatusCode::UNAUTHORIZED
    );

    let first = authenticate(router.clone()).await;
    let second = authenticate(router.clone()).await;
    let (logout, _, _) = request(
        router.clone(),
        auth_post("/auth/logout/all", Some(&first), Body::empty()),
    )
    .await;
    assert_eq!(logout, StatusCode::NO_CONTENT);
    assert_eq!(
        authenticated_graphql_status(router, &second).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn router_serves_schema_graphiql_and_spa_fallback() {
    let (disabled_status, _, _) = request(
        test_router_without_auth().await,
        empty_request(Method::GET, "/graphql"),
    )
    .await;
    assert_eq!(disabled_status, StatusCode::NOT_FOUND);

    let mut graphiql_state = web_state(WebAuthMode::DisabledForDevelopment).await;
    graphiql_state.graphiql_enabled = true;
    let graphiql_router = build_router(graphiql_state);
    let (graphiql_status, graphiql_headers, graphiql_body) = request(
        graphiql_router.clone(),
        empty_request(Method::GET, "/graphql"),
    )
    .await;
    assert_eq!(graphiql_status, StatusCode::OK);
    assert_eq!(
        graphiql_headers[header::CONTENT_SECURITY_POLICY],
        GRAPHIQL_CSP
    );
    let graphiql_body = String::from_utf8(graphiql_body.to_vec()).expect("GraphiQL HTML");
    assert!(graphiql_body.contains("Noema GraphiQL"));
    assert!(graphiql_body.contains("/assets/"));
    assert!(!graphiql_body.contains("https://"));
    assert_eq!(
        request(
            graphiql_router,
            empty_request(Method::GET, "/assets/graphiql.html")
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    for (router, uri, expected_fragment) in [(
        test_router_without_auth().await,
        "/graphql/schema.graphql",
        "type QueryRoot",
    )] {
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
        "/artifacts/versions/missing/preview",
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
                r#"{"query":"{ task(taskId: \"task:transport\") { taskId } }","extensions":{"principal":{"subjectId":"attacker"}}}"#,
            ))
            .expect("GraphQL principal request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let body = serde_json::from_slice::<serde_json::Value>(&body).expect("GraphQL JSON");
    assert_eq!(
        body.pointer("/errors/0/message")
            .and_then(|value| value.as_str()),
        Some("Noema store is unavailable")
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
    let store = test_store().await;
    let state = WebState::new(
        noema_api::graphql::GraphqlState::for_tests(),
        store.clone(),
        authority,
        session::SessionSecurity::for_tests(store),
        WebAuthMode::Required,
        false,
        WebFiles::new(Some(test_recovery()), test_paths()),
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
                    "query": "subscription { tasksEvents(workspaceId: \"workspace:personal\") { cursor } }",
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
    let next = serde_json::from_str::<serde_json::Value>(next.to_text().expect("result text"))
        .expect("result JSON");
    assert_eq!(
        next.pointer("/payload/errors/0/message")
            .and_then(|value| value.as_str()),
        Some("Noema store is unavailable")
    );
    let _complete = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("subscription complete timeout")
        .expect("subscription complete frame")
        .expect("subscription complete");

    let logout = client
        .post(format!("http://localhost:{}/auth/logout", address.port()))
        .header(
            reqwest::header::ORIGIN,
            format!("http://localhost:{}", address.port()),
        )
        .header(reqwest::header::COOKIE, &ws_cookie)
        .send()
        .await
        .expect("logout request");
    assert_eq!(logout.status(), reqwest::StatusCode::NO_CONTENT);
    let closed = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("revocation close timeout");
    assert!(matches!(
        closed,
        None | Some(Err(_)) | Some(Ok(Message::Close(_)))
    ));
    server.abort();
}

#[tokio::test]
async fn client_bearer_authorizes_http_and_ws_without_browser_origin_and_revocation_closes_ws() {
    use futures_util::{SinkExt, StreamExt};

    let store = test_store().await;
    let client_id = "noema-desktop:abcdefghijklmnop";
    let access = URL_SAFE_NO_PAD.encode([6_u8; 32]);
    let pkce_value = "p".repeat(44);
    let access_digest = digest::digest(&digest::SHA256, access.as_bytes());
    let now: i64 = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .expect("system time")
        .as_secs()
        .try_into()
        .expect("timestamp");
    store
        .insert_native_oauth_code(
            [7_u8; 32],
            noema_store::NewNativeOAuthCode {
                client_id,
                display_name: "Native client",
                redirect_uri: "http://127.0.0.1:49152/oauth/callback",
                pkce_value: &pkce_value,
                expires_at: now + 300,
            },
            now,
        )
        .await
        .expect("insert client");
    store
        .insert_native_oauth_family(noema_store::NewNativeOAuthFamily {
            family_id: "0123456789abcdef0123456789abcdef",
            client_id,
            access_hash: access_digest.as_ref().try_into().expect("access digest"),
            refresh_hash: [8_u8; 32],
            issued_at: now,
            access_expires_at: now + 900,
            idle_expires_at: now + 30 * 24 * 60 * 60,
            absolute_expires_at: now + 180 * 24 * 60 * 60,
        })
        .await
        .expect("insert family");
    let bearer = format!("Bearer {access}");
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
        session::SessionSecurity::for_tests(store.clone()),
        WebAuthMode::Required,
        false,
        WebFiles::new(Some(test_recovery()), test_paths()),
    )
    .expect("web state");
    super::super::favicons::seed_icon(&state.favicons, "example.com", b"png");
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
        .body(r#"{"query":"{ task(taskId: \"task:transport\") { taskId } }"}"#)
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
        .body(r#"{"query":"{ task(taskId: \"task:transport\") { taskId } }"}"#)
        .send()
        .await
        .expect("bearer GraphQL request");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let response = response
        .json::<serde_json::Value>()
        .await
        .expect("GraphQL JSON");
    assert_eq!(
        response
            .pointer("/errors/0/message")
            .and_then(|value| value.as_str()),
        Some("Noema store is unavailable")
    );

    let favicon_response = client
        .get(format!(
            "http://localhost:{}/favicons/example.com",
            address.port()
        ))
        .header(reqwest::header::AUTHORIZATION, &bearer)
        .send()
        .await
        .expect("bearer favicon request");
    assert_eq!(favicon_response.status(), reqwest::StatusCode::OK);
    let favicon_body = favicon_response.bytes().await.expect("favicon body");
    assert_eq!(favicon_body.as_ref(), b"png");

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
                "payload": {"query": "subscription { tasksEvents(workspaceId: \"workspace:personal\") { cursor } }"}
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

    let expiring_access = URL_SAFE_NO_PAD.encode([16_u8; 32]);
    let expiring_digest = digest::digest(&digest::SHA256, expiring_access.as_bytes());
    let expiring_now: i64 = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .expect("system time")
        .as_secs()
        .try_into()
        .expect("timestamp");
    store
        .insert_native_oauth_family(noema_store::NewNativeOAuthFamily {
            family_id: "1123456789abcdef0123456789abcdef",
            client_id,
            access_hash: expiring_digest.as_ref().try_into().expect("access digest"),
            refresh_hash: [18_u8; 32],
            issued_at: expiring_now,
            access_expires_at: expiring_now + 3,
            idle_expires_at: expiring_now + 30 * 24 * 60 * 60,
            absolute_expires_at: expiring_now + 180 * 24 * 60 * 60,
        })
        .await
        .expect("insert expiring family");
    let mut expiring_request = format!("ws://localhost:{}/graphql/ws", address.port())
        .into_client_request()
        .expect("expiring WebSocket request");
    expiring_request.headers_mut().insert(
        "authorization",
        format!("Bearer {expiring_access}")
            .parse()
            .expect("authorization header"),
    );
    expiring_request.headers_mut().insert(
        "sec-websocket-protocol",
        "graphql-transport-ws".parse().expect("protocol header"),
    );
    let (mut expiring_socket, _) = connect_async(expiring_request)
        .await
        .expect("expiring WebSocket handshake");
    expiring_socket
        .send(Message::Text(
            json!({"type": "connection_init"}).to_string().into(),
        ))
        .await
        .expect("expiring connection init");
    let _ack = expiring_socket
        .next()
        .await
        .expect("expiring connection ack frame")
        .expect("expiring connection ack");
    let expired = tokio::time::timeout(std::time::Duration::from_secs(5), expiring_socket.next())
        .await
        .expect("access expiry close timeout");
    assert!(matches!(
        expired,
        None | Some(Err(_)) | Some(Ok(Message::Close(_)))
    ));

    store
        .revoke_client("human:local", client_id)
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
        (Method::HEAD, "/artifacts/versions/missing/preview"),
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
async fn artifact_preview_adapter_allows_only_inert_browser_formats() {
    let response = artifact_preview_response(noema_api::graphql::AuthorizedArtifactDownload {
        filename: "label.png".to_owned(),
        media_type: "image/png".to_owned(),
        bytes: b"png".to_vec(),
    });
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        "inline; filename=\"label.png\""
    );
    assert_eq!(
        response.headers()[header::CONTENT_SECURITY_POLICY],
        "default-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'none'"
    );

    for media_type in ["image/svg+xml", "text/html"] {
        let response = artifact_preview_response(noema_api::graphql::AuthorizedArtifactDownload {
            filename: "unsafe".to_owned(),
            media_type: media_type.to_owned(),
            bytes: Vec::new(),
        });
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
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
