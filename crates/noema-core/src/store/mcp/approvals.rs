use serde_json::{Map, Value};

use super::{
    McpApprovalRequestRecord, NewMcpApprovalRequest, mcp_record_fragment,
    rows::{McpApprovalRequestRow, mcp_approval_request_from_row},
};
use crate::store::{NoemaStore, StoreError};

impl NoemaStore {
    /// Create one durable MCP approval request.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn create_mcp_approval_request(
        &self,
        approval: NewMcpApprovalRequest,
    ) -> Result<McpApprovalRequestRecord, StoreError> {
        let payload_preview = sanitize_approval_payload_preview(approval.payload_preview);
        self.db
            .query(
                r#"
                CREATE type::record('approval_requests', $record_id) SET
                  approval_id = $approval_id,
                  action_summary = $action_summary,
                  tool_invocation_id = $tool_invocation_id,
                  mcp_server_id = $mcp_server_id,
                  mcp_tool_id = $mcp_tool_id,
                  requester_actor_id = $requester_actor_id,
                  owner_scope_id = $owner_scope_id,
                  active_scope_id = $active_scope_id,
                  destination_summary = $destination_summary,
                  data_source_summary = $data_source_summary,
                  source_owner_identity = $source_owner_identity,
                  source_owner_trust = $source_owner_trust,
                  destination_owner_identity = $destination_owner_identity,
                  destination_owner_trust = $destination_owner_trust,
                  export_summary = $export_summary,
                  payload_preview = $payload_preview,
                  status = 'pending',
                  decision_actor_id = NONE,
                  decision_comment = NONE,
                  decided_at = NONE,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", mcp_record_fragment(&approval.approval_id)))
            .bind(("approval_id", approval.approval_id.clone()))
            .bind(("action_summary", approval.action_summary))
            .bind(("tool_invocation_id", approval.tool_invocation_id))
            .bind(("mcp_server_id", approval.mcp_server_id))
            .bind(("mcp_tool_id", approval.mcp_tool_id))
            .bind(("requester_actor_id", approval.requester_actor_id))
            .bind(("owner_scope_id", approval.owner_scope_id))
            .bind(("active_scope_id", approval.active_scope_id))
            .bind(("destination_summary", approval.destination_summary))
            .bind(("data_source_summary", approval.data_source_summary))
            .bind(("source_owner_identity", approval.source_owner_identity))
            .bind(("source_owner_trust", approval.source_owner_trust))
            .bind((
                "destination_owner_identity",
                approval.destination_owner_identity,
            ))
            .bind(("destination_owner_trust", approval.destination_owner_trust))
            .bind(("export_summary", approval.export_summary))
            .bind(("payload_preview", payload_preview))
            .await?
            .check()?;
        self.get_mcp_approval_request(&approval.approval_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing MCP approval request after create: {}",
                    approval.approval_id
                ))
            })
    }

    /// List durable MCP approval requests, optionally filtered by status.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn list_mcp_approval_requests(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<McpApprovalRequestRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT approval_id, action_summary, tool_invocation_id, mcp_server_id,
                  mcp_tool_id, requester_actor_id, owner_scope_id, active_scope_id,
                  destination_summary, data_source_summary, source_owner_identity,
                  source_owner_trust, destination_owner_identity, destination_owner_trust,
                  export_summary, payload_preview, status, decision_actor_id,
                  decision_comment, decided_at
                FROM approval_requests
                WHERE $status = NONE OR status = $status;
                "#,
            )
            .bind(("status", status.map(str::to_string)))
            .await?;
        let rows: Vec<McpApprovalRequestRow> = response.take(0)?;
        let mut approvals: Vec<McpApprovalRequestRecord> = rows
            .into_iter()
            .map(mcp_approval_request_from_row)
            .collect();
        approvals.sort_by(|left, right| left.approval_id.cmp(&right.approval_id));
        Ok(approvals)
    }

    async fn get_mcp_approval_request(
        &self,
        approval_id: &str,
    ) -> Result<Option<McpApprovalRequestRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT approval_id, action_summary, tool_invocation_id, mcp_server_id,
                  mcp_tool_id, requester_actor_id, owner_scope_id, active_scope_id,
                  destination_summary, data_source_summary, source_owner_identity,
                  source_owner_trust, destination_owner_identity, destination_owner_trust,
                  export_summary, payload_preview, status, decision_actor_id,
                  decision_comment, decided_at
                FROM approval_requests
                WHERE approval_id = $approval_id
                LIMIT 1;
                "#,
            )
            .bind(("approval_id", approval_id.to_string()))
            .await?;
        let rows: Vec<McpApprovalRequestRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(mcp_approval_request_from_row))
    }
}

fn sanitize_approval_payload_preview(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(sanitize_preview_object(object)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .take(10)
                .map(sanitize_approval_payload_preview)
                .collect(),
        ),
        Value::String(value) => Value::String(truncate_preview_string(value)),
        other => other,
    }
}

fn sanitize_preview_object(object: Map<String, Value>) -> Map<String, Value> {
    object
        .into_iter()
        .take(20)
        .map(|(key, value)| {
            let sanitized_value = if approval_preview_key_is_sensitive(&key) {
                Value::String("[redacted]".to_string())
            } else {
                sanitize_approval_payload_preview(value)
            };
            (key, sanitized_value)
        })
        .collect()
}

fn approval_preview_key_is_sensitive(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "secret",
        "token",
        "password",
        "credential",
        "api_key",
        "apikey",
        "private_key",
        "authorization",
        "auth",
        "cookie",
        "session",
        "set-cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn truncate_preview_string(value: String) -> String {
    const MAX_PREVIEW_CHARS: usize = 240;
    if value.chars().count() <= MAX_PREVIEW_CHARS {
        return value;
    }

    let mut truncated = value.chars().take(MAX_PREVIEW_CHARS).collect::<String>();
    truncated.push_str("...");
    truncated
}
