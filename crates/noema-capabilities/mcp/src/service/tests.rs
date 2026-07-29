use noema_capabilities::{
    CapabilityBindingSource, CapabilityError, CapabilityInvocation, CapabilityInvoker,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{McpOperationError, McpOperations};

use super::test_support::TestHarness;

#[tokio::test]
async fn shutdown_cancels_admitted_transport_work_and_rejects_new_work() {
    let harness = TestHarness::new();
    let invoking = harness.start_blocked_invocation().await;

    assert!(harness.service.shutdown().await);
    assert_eq!(
        invoking.await.expect("invoke task"),
        Err(CapabilityError::Unavailable)
    );
    assert_eq!(
        McpOperations::list_servers(&harness.service).await,
        Err(McpOperationError::ShuttingDown)
    );
    assert!(
        CapabilityBindingSource::catalog(&harness.service)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn chat_service_discovery_dispatches_verified_card_into_existing_setup() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let origin = format!("http://{}/", listener.local_addr().expect("address"));
    let endpoint = format!("{origin}mcp");
    let card = serde_json::to_vec(&json!({
        "serverInfo": {"name": "docs", "title": "Docs", "version": "1"},
        "transport": {"type": "streamable-http", "endpoint": endpoint},
        "description": "Official docs tools"
    }))
    .expect("server card");
    let responder = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.expect("connection");
            let mut request = vec![0_u8; 4_096];
            let read = socket.read(&mut request).await.expect("request");
            let is_card =
                String::from_utf8_lossy(&request[..read]).starts_with("GET /.well-known/mcp.json ");
            let (status, body) = if is_card {
                ("200 OK", card.as_slice())
            } else {
                ("404 Not Found", &[][..])
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response headers");
            socket.write_all(body).await.expect("response body");
        }
    });

    let harness = TestHarness::new();
    let catalog = CapabilityBindingSource::catalog(&harness.service)
        .await
        .expect("catalog");
    let binding = catalog
        .snapshot
        .resolve(crate::catalog::CONNECT_SERVICE_TOOL)
        .expect("connect service binding");
    let output = CapabilityInvoker::invoke(
        &harness.service,
        CapabilityInvocation {
            operation: binding.spec().name.clone(),
            operation_token: binding.target().operation_token().clone(),
            arguments: json!({"service_url": origin}),
            reviewed_authorization: None,
        },
    )
    .await
    .expect("setup output");
    responder.abort();

    assert_eq!(output.payload["status"], "ready_for_policy");
    assert_eq!(output.payload["display_name"], "Docs");
    assert_eq!(output.payload["endpoint_url"], endpoint);
    assert_eq!(
        output.payload["setup_result"]["server"]["mcp_server_id"],
        "mcp:created"
    );
    assert_eq!(
        output.payload["setup_input"]["http"]["secretHeaders"],
        json!({})
    );
}
