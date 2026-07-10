use std::time::Duration;

use serde_json::Value;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use crate::daemon::TestDaemonWebServer;

#[tokio::test]
async fn test_server_reports_resolved_loopback_address_and_serves_injected_graphql_state() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("default actors");
    store
        .update_agent_display_name("agent:primary", "Fixture Agent")
        .await
        .expect("primary agent display name");
    let server =
        TestDaemonWebServer::start(crate::graphql::GraphqlState::for_tests_with_store(store))
            .await
            .expect("start test web server");
    let address = server.local_addr();

    assert!(address.ip().is_loopback());
    assert_ne!(address.port(), 0);
    assert_eq!(server.base_url(), format!("http://{address}"));

    let query = serde_json::json!({
        "query": "{ localStatus { localService assistantConnection memoryStorage primaryAgentDisplayName } }"
    });
    let body = serde_json::to_vec(&query).expect("serialize GraphQL request");
    let mut client = TcpStream::connect(address).await.expect("connect client");
    client
        .write_all(
            format!(
                "POST /graphql HTTP/1.1\r\nHost: {address}\r\nOrigin: http://{address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .expect("write GraphQL request headers");
    client
        .write_all(&body)
        .await
        .expect("write GraphQL request body");
    client.flush().await.expect("flush GraphQL request");

    let mut response = Vec::new();
    client
        .read_to_end(&mut response)
        .await
        .expect("read GraphQL response");
    let response = String::from_utf8(response).expect("UTF-8 GraphQL response");
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .expect("HTTP response delimiter");
    assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"), "{headers}");
    let body: Value = serde_json::from_str(body).expect("GraphQL JSON response");
    assert_eq!(
        body,
        serde_json::json!({
            "data": {
                "localStatus": {
                    "localService": "RUNNING",
                    "assistantConnection": "CODEX",
                    "memoryStorage": "READY",
                    "primaryAgentDisplayName": "Fixture Agent"
                }
            }
        })
    );

    timeout(Duration::from_secs(1), server.shutdown())
        .await
        .expect("test server shutdown timed out")
        .expect("test server shutdown");
}

#[tokio::test]
async fn shutdown_owns_and_joins_open_websocket_connection() {
    let server = TestDaemonWebServer::start(crate::graphql::GraphqlState::for_tests())
        .await
        .expect("start test web server");
    let address = server.local_addr();
    let mut client = TcpStream::connect(address).await.expect("connect client");
    client
        .write_all(
            format!(
                "GET /graphql/ws HTTP/1.1\r\nHost: {address}\r\nOrigin: http://{address}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: graphql-transport-ws\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .expect("write WebSocket upgrade request");
    client
        .flush()
        .await
        .expect("flush WebSocket upgrade request");

    let response = read_http_response_head(&mut client).await;
    assert!(
        response.starts_with("HTTP/1.1 101 Switching Protocols\r\n"),
        "{response}"
    );

    timeout(Duration::from_secs(1), server.shutdown())
        .await
        .expect("test server shutdown timed out")
        .expect("test server shutdown");

    let mut byte = [0_u8; 1];
    match timeout(Duration::from_secs(1), client.read(&mut byte))
        .await
        .expect("WebSocket client remained open after shutdown")
    {
        Ok(0) | Err(_) => {}
        Ok(count) => panic!("WebSocket client received {count} unexpected bytes after shutdown"),
    }
    assert!(
        TcpStream::connect(address).await.is_err(),
        "listener still accepted connections after shutdown"
    );
}

async fn read_http_response_head(stream: &mut TcpStream) -> String {
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    while !response.ends_with(b"\r\n\r\n") {
        timeout(Duration::from_secs(1), stream.read_exact(&mut byte))
            .await
            .expect("HTTP response head timed out")
            .expect("read HTTP response head");
        response.push(byte[0]);
    }
    String::from_utf8(response).expect("UTF-8 HTTP response head")
}
