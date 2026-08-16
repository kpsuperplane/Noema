use std::os::unix::fs::PermissionsExt;

use axum::{body::Body, http::Request};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tower::ServiceExt as _;

use super::*;

#[tokio::test]
async fn socket_serves_local_authority_with_private_permissions_and_cleanup() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("run/graphql.sock");
    let server = LocalGraphqlServer::bind(
        path.clone(),
        noema_api::graphql::build_schema(noema_api::graphql::GraphqlState::for_tests()),
    )
    .await
    .expect("bind local GraphQL");

    assert_eq!(
        std::fs::metadata(path.parent().expect("parent"))
            .expect("parent metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&path)
            .expect("socket metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let task = tokio::spawn(server.serve(shutdown_receiver));
    let mut stream = tokio::net::UnixStream::connect(&path)
        .await
        .expect("connect local GraphQL");
    let body = r#"{"query":"{ testRequestPrincipal }"}"#;
    stream
        .write_all(
            format!(
                "POST /graphql HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .expect("write local GraphQL request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .expect("read local GraphQL response");
    let response = String::from_utf8(response).expect("response text");
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("human:local"));

    shutdown_sender.send(true).expect("shutdown");
    task.await.expect("server task").expect("server shutdown");
    assert!(!path.exists());
}

#[tokio::test]
async fn router_has_local_authority_and_exposes_only_bounded_graphql_post() {
    let router = local_router(noema_api::graphql::build_schema(
        noema_api::graphql::GraphqlState::for_tests(),
    ));
    let response = router
        .clone()
        .oneshot(
            Request::post("/graphql")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"query":"{ testRequestPrincipal }"}"#))
                .expect("GraphQL request"),
        )
        .await
        .expect("GraphQL response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

    for request in [Request::get("/graphql"), Request::post("/auth/recovery")] {
        let response = router
            .clone()
            .oneshot(request.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    let response = router
        .oneshot(
            Request::post("/graphql")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONTENT_LENGTH, MAX_GRAPHQL_BODY_BYTES + 1)
                .body(Body::from(vec![b'x'; MAX_GRAPHQL_BODY_BYTES + 1]))
                .expect("oversized request"),
        )
        .await
        .expect("oversized response");
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
