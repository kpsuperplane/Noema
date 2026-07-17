use super::*;

pub(super) async fn test_store_with_mcp_tool() -> crate::NoemaStore {
    let store = test_store().await;
    store
        .create_mcp_server(google_server())
        .await
        .expect("create server");
    upsert_google_tool(
        &store,
        "read_doc",
        json!({"readOnlyHint": true}),
        "fingerprint_1",
    )
    .await;
    store
}

pub(super) fn google_server() -> NewMcpServer {
    NewMcpServer {
        mcp_server_id: "mcp_server:google".to_string(),
        display_name: "Google".to_string(),
        transport_kind: McpTransportKind::Stdio,
        safe_config: json!({}),
    }
}

pub(super) fn google_tool(
    name: &str,
    annotations: serde_json::Value,
    fingerprint: &str,
) -> NewMcpTool {
    NewMcpTool {
        mcp_tool_id: format!("mcp_tool:google:{name}"),
        mcp_server_id: "mcp_server:google".to_string(),
        name: name.to_string(),
        description: Some(name.replace('_', " ")),
        input_schema: json!({"type": "object"}),
        output_schema: None,
        annotations,
        metadata_fingerprint: fingerprint.to_string(),
    }
}

pub(super) async fn upsert_google_tool(
    store: &crate::NoemaStore,
    name: &str,
    annotations: serde_json::Value,
    fingerprint: &str,
) {
    store
        .upsert_discovered_mcp_tool(google_tool(name, annotations, fingerprint))
        .await
        .expect("upsert tool");
}

pub(super) fn ready_google_read_calibration(
    reviewed_metadata_fingerprint: &str,
) -> NewToolCalibration {
    google_read_calibration(
        McpTrustClassification::Trusted,
        McpTrustClassification::None,
        McpCalibrationStatus::Ready,
        reviewed_metadata_fingerprint,
    )
}

pub(super) fn ready_google_calibration(
    calibration_name: &str,
    tool_name: &str,
    read_classification: McpTrustClassification,
    write_classification: McpTrustClassification,
    reviewed_metadata_fingerprint: &str,
) -> NewToolCalibration {
    NewToolCalibration {
        calibration_id: format!("tool_calibration:{calibration_name}"),
        mcp_tool_id: format!("mcp_tool:google:{tool_name}"),
        read_classification,
        write_classification,
        export_classification: McpTrustClassification::None,
        status: McpCalibrationStatus::Ready,
        reviewed_by: Some("human:local".to_string()),
        reviewed_metadata_fingerprint: Some(reviewed_metadata_fingerprint.to_string()),
    }
}

pub(super) fn google_read_calibration(
    read_classification: McpTrustClassification,
    write_classification: McpTrustClassification,
    status: McpCalibrationStatus,
    reviewed_metadata_fingerprint: &str,
) -> NewToolCalibration {
    NewToolCalibration {
        calibration_id: "tool_calibration:read_doc".to_string(),
        mcp_tool_id: "mcp_tool:google:read_doc".to_string(),
        read_classification,
        write_classification,
        export_classification: McpTrustClassification::None,
        status,
        reviewed_by: Some("human:local".to_string()),
        reviewed_metadata_fingerprint: Some(reviewed_metadata_fingerprint.to_string()),
    }
}
