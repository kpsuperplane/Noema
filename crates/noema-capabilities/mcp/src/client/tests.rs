use std::any::TypeId;

use rmcp::transport::streamable_http_client::AuthRequiredError;

use super::*;

#[tokio::test]
async fn request_context_transport_and_redaction_contracts() {
    // Case: request_context_distinguishes_cancellation_and_timeout.
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let cancelled_context = McpRequestContext::with_timeout(Duration::from_secs(1), cancelled);
    let error = run_with_context(&cancelled_context, "tools/list", async { Ok(()) })
        .await
        .expect_err("cancelled");
    assert_eq!(
        error,
        McpClientError::Cancelled {
            operation: "tools/list"
        }
    );

    let expired_context = McpRequestContext::new(
        Instant::now() - Duration::from_millis(1),
        CancellationToken::new(),
    );
    let error = run_with_context(&expired_context, "tools/call", async { Ok(()) })
        .await
        .expect_err("timed out");
    assert_eq!(
        error,
        McpClientError::Timeout {
            operation: "tools/call"
        }
    );
    // Case: tool_declared_error_remains_a_successful_transport_output.
    let mut result = CallToolResult::default();
    result.structured_content = Some(json!({"reason": "denied"}));
    result.is_error = Some(true);
    let output = tool_call_output_from_rmcp(result).expect("transport output");

    assert!(output.is_error);
    assert_eq!(output.result["isError"], true);
    assert_eq!(
        output.result["structuredContent"],
        json!({"reason": "denied"})
    );
    // Case: oversized_tool_result_is_rejected_before_capability_exposure.
    let mut result = CallToolResult::default();
    result.structured_content = Some(json!({"content": "x".repeat(MAX_TOOL_RESULT_BYTES)}));

    assert!(matches!(
        tool_call_output_from_rmcp(result),
        Err(McpClientError::Malformed(_))
    ));
    // Case: call_arguments_fail_before_protocol_work_when_not_an_object.
    let error =
        call_tool_params("read", json!(["not", "an", "object"])).expect_err("invalid arguments");
    assert!(matches!(error, McpClientError::Malformed(_)));
    // Case: metadata_discovery_returns_auth_required_from_initialize.
    let error = DynamicTransportError::from_parts(
        "test-http",
        TypeId::of::<()>(),
        Box::new(StreamableHttpError::<reqwest::Error>::AuthRequired(
            AuthRequiredError::new(
                r#"Bearer resource_metadata="/.well-known/oauth-protected-resource""#.to_string(),
            ),
        )),
    );

    assert!(matches!(
        dynamic_transport_error(error),
        McpClientError::AuthenticationRequired(_)
    ));
    // Case: preparation_debug_redacts_refreshed_credentials.
    struct FakeSession;

    impl McpPreparedSession for FakeSession {
        fn discover_tools<'a>(
            &'a mut self,
            _context: &'a McpRequestContext,
        ) -> McpClientFuture<'a, Vec<McpDiscoveredTool>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn call_tool<'a>(
            &'a mut self,
            _tool_name: &'a str,
            _arguments: Value,
            _context: &'a McpRequestContext,
        ) -> McpClientFuture<'a, McpToolCallOutput> {
            Box::pin(async {
                Ok(McpToolCallOutput {
                    result: json!({}),
                    is_error: false,
                })
            })
        }

        fn close(self: Box<Self>) -> McpClientFuture<'static, ()> {
            Box::pin(async { Ok(()) })
        }
    }

    let preparation = McpSessionPreparation::new(
        Box::new(FakeSession),
        Some(McpOAuthStoredCredentials {
            client_id: "secret-client".to_string(),
            token_response: json!({"access_token": "secret-token"}),
            token_received_at: Some(42),
        }),
    );
    let debug = format!("{preparation:?}");

    assert!(!debug.contains("secret-client"));
    assert!(!debug.contains("secret-token"));
    assert!(debug.contains("[REDACTED]"));
}
