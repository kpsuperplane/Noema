//! Run-scoped terminal MCP tools for an ACP Work executor.

use rmcp::{
    ErrorData as McpError, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

#[derive(Clone)]
struct TaskTools;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SubmitResult {
    summary: String,
    result_markdown: String,
    criteria: Vec<CriterionEvidence>,
    artifact_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CriterionEvidence {
    criterion_id: String,
    evidence_markdown: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum GateKind {
    Clarification,
    Approval,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReportBlocked {
    gate_kind: GateKind,
    question: String,
    #[serde(default)]
    context_markdown: String,
    #[serde(default)]
    suggested_answers: Vec<String>,
}

#[tool_router]
impl TaskTools {
    #[tool(
        name = "task.submit_result",
        description = "Submit the complete result and criterion evidence for this Work task."
    )]
    async fn submit_result(
        &self,
        Parameters(input): Parameters<SubmitResult>,
    ) -> Result<CallToolResult, McpError> {
        forward(
            "task.submit_result",
            serde_json::to_value(input).map_err(internal)?,
        )
        .await
    }

    #[tool(
        name = "task.report_blocked",
        description = "Stop this Work task and request a specific human decision or missing input."
    )]
    async fn report_blocked(
        &self,
        Parameters(input): Parameters<ReportBlocked>,
    ) -> Result<CallToolResult, McpError> {
        forward(
            "task.report_blocked",
            serde_json::to_value(input).map_err(internal)?,
        )
        .await
    }
}

#[tool_handler]
impl rmcp::ServerHandler for TaskTools {}

async fn forward(tool: &str, arguments: serde_json::Value) -> Result<CallToolResult, McpError> {
    let address = std::env::var("NOEMA_ACP_TASK_BRIDGE_ADDR")
        .map_err(|_| internal("missing loopback bridge address"))?;
    let token = std::env::var("NOEMA_ACP_TASK_TOKEN").map_err(|_| internal("missing run token"))?;
    let stream = TcpStream::connect(&address).await.map_err(internal)?;
    let (reader, mut writer) = stream.into_split();
    let request = json!({"token": token, "tool": tool, "arguments": arguments});
    writer
        .write_all(
            serde_json::to_string(&request)
                .map_err(internal)?
                .as_bytes(),
        )
        .await
        .map_err(internal)?;
    writer.write_all(b"\n").await.map_err(internal)?;
    let mut response = String::new();
    BufReader::new(reader)
        .read_line(&mut response)
        .await
        .map_err(internal)?;
    let response: serde_json::Value = serde_json::from_str(&response).map_err(internal)?;
    if response.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(internal("runtime rejected the terminal submission"));
    }
    Ok(CallToolResult::success(vec![ContentBlock::text(
        "Noema accepted the terminal submission.",
    )]))
}

fn internal(error: impl std::fmt::Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = TaskTools.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_tool_schema_restricts_gate_kinds() {
        let schema = serde_json::to_value(schemars::schema_for!(ReportBlocked)).unwrap();

        assert_eq!(
            schema.pointer("/$defs/GateKind/enum"),
            Some(&json!(["clarification", "approval"]))
        );
    }
}
