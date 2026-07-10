//! Axum route composition and HTTP response adaptation for the daemon web UI.

use std::borrow::Cow;

use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::{
    Router,
    body::Body,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
    routing::{get, get_service},
};
use tower_http::{limit::RequestBodyLimitLayer, set_header::SetResponseHeaderLayer};

use super::{WebState, assets::embedded_asset};

const MAX_GRAPHQL_BODY_BYTES: usize = 64 * 1024;
const NOT_FOUND: &str = "not found";

/// Build the single application router served by the daemon.
pub(crate) fn build_router(state: WebState) -> Router {
    let graphql_subscription = GraphQLSubscription::new(state.graphql_schema().clone());

    Router::new()
        .route(
            "/graphql",
            get(graphiql)
                .post(graphql)
                .head(method_not_found)
                .fallback(method_not_found)
                .layer(RequestBodyLimitLayer::new(MAX_GRAPHQL_BODY_BYTES)),
        )
        .route(
            "/graphql/schema.graphql",
            get(graphql_schema)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/graphql/ws",
            get_service(graphql_subscription)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/mcp/oauth/callback",
            get(mcp_oauth_callback)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/artifacts/versions/{artifact_version_slug}/download",
            get(download_artifact_slug)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .route(
            "/artifacts/{artifact_version_id}/download",
            get(download_artifact_id)
                .head(method_not_found)
                .fallback(method_not_found),
        )
        .fallback(asset_or_not_found)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .with_state(state)
}

async fn graphql(State(state): State<WebState>, request: GraphQLRequest) -> GraphQLResponse {
    state
        .graphql_schema()
        .execute(request.into_inner())
        .await
        .into()
}

async fn graphiql() -> Html<String> {
    Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql/ws")
            .finish(),
    )
}

async fn graphql_schema(State(state): State<WebState>) -> Response {
    plain_response(StatusCode::OK, state.graphql_schema().sdl())
}

async fn mcp_oauth_callback(
    State(state): State<WebState>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let Some(query) = query else {
        return plain_response(StatusCode::BAD_REQUEST, "missing OAuth callback query");
    };
    let Some(attempt_id) = query_value(&query, "attemptId") else {
        return plain_response(StatusCode::BAD_REQUEST, "missing MCP OAuth attempt id");
    };
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return plain_response(StatusCode::BAD_REQUEST, "invalid MCP OAuth callback");
    };
    let callback_url = format!("http://{host}/mcp/oauth/callback?{query}");

    match crate::graphql::complete_mcp_server_oauth_setup(
        state.graphql_state(),
        &attempt_id,
        &callback_url,
    )
    .await
    {
        Ok(attempt) if attempt.status == "completed" => Html(
            "<!doctype html><title>Noema MCP OAuth</title><p>Authentication completed. You can return to Noema.</p>",
        )
        .into_response(),
        Ok(_) => Html(
            "<!doctype html><title>Noema MCP OAuth</title><p>Authentication finished, but Noema could not list tools. Return to Noema to retry.</p>",
        )
        .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Html("<!doctype html><title>Noema MCP OAuth</title><p>Noema could not complete this MCP OAuth setup attempt.</p>"),
        )
            .into_response(),
    }
}

fn query_value(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

async fn download_artifact_slug(
    State(state): State<WebState>,
    Path(artifact_version_slug): Path<String>,
) -> Response {
    let Some(artifact_version_id) =
        crate::artifact_version_id_from_download_slug(&artifact_version_slug)
    else {
        return not_found();
    };
    download_artifact(&state, &artifact_version_id).await
}

async fn download_artifact_id(
    State(state): State<WebState>,
    Path(artifact_version_id): Path<String>,
) -> Response {
    download_artifact(&state, &artifact_version_id).await
}

async fn download_artifact(state: &WebState, artifact_version_id: &str) -> Response {
    let store = match state.graphql_state().store() {
        Ok(store) => store,
        Err(_) => return internal_error(state, "store_state"),
    };
    let paths = match state.graphql_state().paths() {
        Ok(paths) => paths,
        Err(_) => return internal_error(state, "path_state"),
    };
    let version = match store.get_artifact_version(artifact_version_id).await {
        Ok(Some(version)) => version,
        Ok(None) => return not_found(),
        Err(_) => return internal_error(state, "version_query"),
    };
    let artifact = match store.get_artifact(&version.artifact_id).await {
        Ok(Some(artifact)) => artifact,
        Ok(None) => return not_found(),
        Err(_) => return internal_error(state, "artifact_query"),
    };
    if !matches!(
        version.storage,
        crate::ArtifactVersionStorage::LocalFile { .. }
    ) {
        return not_found();
    }
    let Ok((absolute_path, bytes)) =
        crate::artifacts::read_validated_local_artifact_file(paths, &artifact.artifact, &version)
    else {
        return not_found();
    };
    let Some(filename) = absolute_path.file_name().and_then(|value| value.to_str()) else {
        return not_found();
    };
    let content_type = safe_header_value(
        version
            .media_type
            .as_deref()
            .unwrap_or("application/octet-stream"),
    );
    let content_disposition = safe_header_value(&attachment_content_disposition(filename));
    let mut response = Body::from(bytes).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    response
        .headers_mut()
        .insert(header::CONTENT_DISPOSITION, content_disposition);
    response
}

async fn asset_or_not_found(method: Method, uri: Uri) -> Response {
    if method != Method::GET {
        return not_found();
    }
    embedded_asset(uri.path()).map_or_else(not_found, asset_response)
}

fn asset_response(asset: super::assets::EmbeddedAsset) -> Response {
    let mut response = Body::from(asset.body).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, safe_header_value(asset.content_type));
    response
}

fn plain_response(status: StatusCode, body: impl Into<Cow<'static, str>>) -> Response {
    let mut response = (status, body.into()).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn not_found() -> Response {
    plain_response(StatusCode::NOT_FOUND, NOT_FOUND)
}

async fn method_not_found() -> Response {
    not_found()
}

fn internal_error(state: &WebState, operation: &'static str) -> Response {
    state.record_artifact_failure(operation);
    plain_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}

fn safe_header_value(value: &str) -> HeaderValue {
    HeaderValue::from_str(value)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"))
}

fn attachment_content_disposition(filename: &str) -> String {
    let escaped = filename
        .chars()
        .map(|character| match character {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            _ if character.is_control() => "_".to_string(),
            _ => character.to_string(),
        })
        .collect::<String>();
    format!("attachment; filename=\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, StatusCode, header},
    };
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower::ServiceExt;

    use super::*;

    fn test_router() -> Router {
        build_router(WebState::new(crate::graphql::GraphqlState::for_tests()))
    }

    async fn artifact_fixture() -> (
        tempfile::TempDir,
        crate::NoemaPaths,
        crate::NoemaStore,
        crate::ArtifactWithVersions,
        Router,
    ) {
        let home = tempfile::tempdir().expect("temp dir");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
            .await
            .expect("open store");
        let conversation = store
            .create_conversation(crate::NewConversation::local_chat(None, None))
            .await
            .expect("conversation");
        let artifact = crate::create_conversation_local_file_artifact(
            &store,
            &paths,
            crate::NewConversationLocalFileArtifact {
                conversation_id: conversation.conversation_id.clone(),
                title: "Downloadable report".to_string(),
                description: None,
                artifact_kind: "document".to_string(),
                filename: "report.md".to_string(),
                bytes: b"hello download".to_vec(),
                media_type: Some("text/markdown".to_string()),
                created_by_actor_id: "agent:primary".to_string(),
                source: crate::ArtifactSource {
                    conversation_id: Some(conversation.conversation_id),
                    turn_id: None,
                    item_id: None,
                },
                metadata: json!({}),
            },
        )
        .await
        .expect("local artifact");
        let router = build_router(WebState::new(
            crate::graphql::GraphqlState::for_tests_with_store_and_paths(
                store.clone(),
                paths.clone(),
            ),
        ));
        (home, paths, store, artifact, router)
    }

    async fn request(
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
    async fn router_serves_graphql_post_from_startup_schema() {
        let (status, headers, body) = request(
            test_router(),
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
        let (schema_status, _, schema) = request(
            test_router(),
            Request::builder()
                .uri("/graphql/schema.graphql")
                .body(Body::empty())
                .expect("schema request"),
        )
        .await;
        assert_eq!(schema_status, StatusCode::OK);
        assert!(String::from_utf8_lossy(&schema).contains("type QueryRoot"));

        let (graphiql_status, headers, graphiql) = request(
            test_router(),
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
                .header("host", "localhost:8765")
                .body(Body::empty())
                .expect("OAuth request"),
        )
        .await;
        assert_eq!(oauth_status, StatusCode::BAD_REQUEST);
        assert_eq!(oauth_body, "missing OAuth callback query");

        let (_home, _paths, _store, _artifact, artifact_router) = artifact_fixture().await;
        let (artifact_status, _, artifact_body) = request(
            artifact_router,
            Request::builder()
                .uri("/artifacts/versions/missing/download")
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
    async fn official_graphql_subscription_service_accepts_websocket_upgrade() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            axum::serve(listener, test_router())
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("serve router");
        });
        let mut client = tokio::net::TcpStream::connect(address)
            .await
            .expect("connect client");
        client
            .write_all(
                b"GET /graphql/ws HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: graphql-transport-ws\r\n\r\n",
            )
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
    async fn artifact_download_routes_serve_bytes_and_safe_headers() {
        let (_home, _paths, _store, artifact, router) = artifact_fixture().await;
        for uri in [
            crate::artifact_download_url(&artifact.current_version.artifact_version_id),
            format!(
                "/artifacts/{}/download",
                artifact.current_version.artifact_version_id
            ),
        ] {
            let (status, headers, body) = request(
                router.clone(),
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("artifact request"),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(headers[header::CONTENT_TYPE], "text/markdown");
            assert_eq!(
                headers[header::CONTENT_DISPOSITION],
                "attachment; filename=\"report.md\""
            );
            assert_eq!(body, "hello download");
        }
    }

    #[tokio::test]
    async fn artifact_download_refuses_traversal_and_symlinks() {
        let (_home, paths, store, artifact, router) = artifact_fixture().await;
        let version_id = &artifact.current_version.artifact_version_id;
        store
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE artifact_versions SET local_relative_path = ?1 WHERE artifact_version_id = ?2",
                    rusqlite::params!["providers/secret.txt", version_id],
                )
                .map(|_| ())
                .map_err(crate::StoreError::Sqlite)
            })
            .await
            .expect("forge traversal");
        std::fs::create_dir_all(paths.providers_dir()).expect("providers dir");
        std::fs::write(paths.providers_dir().join("secret.txt"), b"top secret").expect("secret");
        let (status, _, body) = request(
            router,
            Request::builder()
                .uri(crate::artifact_download_url(version_id))
                .body(Body::empty())
                .expect("traversal request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, NOT_FOUND);
        assert!(!String::from_utf8_lossy(&body).contains("top secret"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn artifact_download_refuses_symlinked_file() {
        let (_home, paths, _store, artifact, router) = artifact_fixture().await;
        let crate::ArtifactVersionStorage::LocalFile { relative_path } =
            &artifact.current_version.storage
        else {
            panic!("expected local file");
        };
        let artifact_path = paths.root().join(relative_path);
        std::fs::remove_file(&artifact_path).expect("remove artifact file");
        let secret_path = paths.root().join("secret.txt");
        std::fs::write(&secret_path, b"top secret").expect("write secret");
        std::os::unix::fs::symlink(secret_path, artifact_path).expect("symlink artifact file");
        let (status, _, body) = request(
            router,
            Request::builder()
                .uri(crate::artifact_download_url(
                    &artifact.current_version.artifact_version_id,
                ))
                .body(Body::empty())
                .expect("artifact request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, NOT_FOUND);
        assert!(!String::from_utf8_lossy(&body).contains("top secret"));
    }

    #[tokio::test]
    async fn artifact_download_refuses_injected_filename_header() {
        let (_home, paths, store, artifact, router) = artifact_fixture().await;
        let version_id = &artifact.current_version.artifact_version_id;
        let forged_path =
            format!("conversations/x/artifacts/x/versions/1/report\"\r\nx-injected: yes.md");
        std::fs::create_dir_all(paths.root().join("conversations/x/artifacts/x/versions/1"))
            .expect("artifact dirs");
        std::fs::write(paths.root().join(&forged_path), b"hello download").expect("forged file");
        store
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE artifact_versions SET local_relative_path = ?1 WHERE artifact_version_id = ?2",
                    rusqlite::params![forged_path, version_id],
                )
                .map(|_| ())
                .map_err(crate::StoreError::Sqlite)
            })
            .await
            .expect("forge filename");
        let (status, headers, body) = request(
            router,
            Request::builder()
                .uri(crate::artifact_download_url(version_id))
                .body(Body::empty())
                .expect("artifact request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(!headers.contains_key("x-injected"));
        assert_eq!(body, NOT_FOUND);
    }

    #[tokio::test]
    async fn artifact_download_sanitizes_unsafe_media_type() {
        let (_home, _paths, store, artifact, router) = artifact_fixture().await;
        let version_id = &artifact.current_version.artifact_version_id;
        store
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE artifact_versions SET media_type = ?1 WHERE artifact_version_id = ?2",
                    rusqlite::params!["text/markdown\r\nX-Injected: yes", version_id],
                )
                .map(|_| ())
                .map_err(crate::StoreError::Sqlite)
            })
            .await
            .expect("forge media type");
        let (status, headers, body) = request(
            router,
            Request::builder()
                .uri(crate::artifact_download_url(version_id))
                .body(Body::empty())
                .expect("artifact request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
        assert!(!headers.contains_key("x-injected"));
        assert_eq!(body, "hello download");
    }

    #[tokio::test]
    async fn artifact_store_failure_is_safe_500_with_bounded_diagnostic() {
        let (_home, paths, store, artifact, router) = artifact_fixture().await;
        store
            .with_connection(|conn| {
                conn.execute("DROP TABLE artifact_versions", [])
                    .map(|_| ())
                    .map_err(crate::StoreError::Sqlite)
            })
            .await
            .expect("break artifact query");
        let (status, _, body) = request(
            router,
            Request::builder()
                .uri(crate::artifact_download_url(
                    &artifact.current_version.artifact_version_id,
                ))
                .body(Body::empty())
                .expect("artifact request"),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, "internal server error");
        let diagnostics = std::fs::read_to_string(paths.errors_log_path()).expect("diagnostic log");
        assert!(diagnostics.contains("artifact_download_failure"));
        assert!(diagnostics.contains("version_query"));
        assert!(!diagnostics.contains("report.md"));
        assert!(!diagnostics.contains("hello download"));
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
}
