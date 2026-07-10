use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Value};

use super::{McpApprovalRequestRecord, NewMcpApprovalRequest, rows::mcp_approval_request_from_row};
use crate::store::{
    NoemaStore, StoreError,
    sqlite::{json_to_string, now_timestamp_sql},
};

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
        let approval_id = approval.approval_id.clone();
        let payload_preview_json =
            json_to_string(&sanitize_approval_payload_preview(approval.payload_preview))?;
        self.with_connection(|conn| {
            conn.execute(
                format!(
                    r#"
                    INSERT INTO approval_requests (
                      approval_id, action_summary, tool_invocation_id, mcp_server_id,
                      mcp_tool_id, requester_actor_id, owner_scope_id, active_scope_id,
                      destination_summary, data_source_summary, source_owner_identity,
                      source_owner_trust, destination_owner_identity, destination_owner_trust,
                      export_summary, payload_preview_json, status, decision_actor_id,
                      decision_comment, decided_at, updated_at
                    )
                    VALUES (
                      ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                      ?14, ?15, ?16, 'pending', NULL, NULL, NULL, {}
                    )
                    "#,
                    now_timestamp_sql()
                )
                .as_str(),
                params![
                    approval.approval_id,
                    approval.action_summary,
                    approval.tool_invocation_id,
                    approval.mcp_server_id,
                    approval.mcp_tool_id,
                    approval.requester_actor_id,
                    approval.owner_scope_id,
                    approval.active_scope_id,
                    approval.destination_summary,
                    approval.data_source_summary,
                    approval.source_owner_identity,
                    approval.source_owner_trust,
                    approval.destination_owner_identity,
                    approval.destination_owner_trust,
                    approval.export_summary,
                    payload_preview_json,
                ],
            )?;
            Ok(())
        })
        .await?;
        self.get_mcp_approval_request(&approval_id)
            .await?
            .ok_or_else(|| {
                StoreError::Schema(format!(
                    "missing MCP approval request after create: {}",
                    approval_id
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
        self.with_connection(|conn| {
            let mut approvals = Vec::new();
            if let Some(status) = status {
                let mut statement = conn.prepare(
                    format!("{MCP_APPROVAL_REQUEST_SELECT} WHERE status = ?1 ORDER BY approval_id")
                        .as_str(),
                )?;
                let rows = statement.query_map(params![status], mcp_approval_request_from_row)?;
                for row in rows {
                    approvals.push(row?);
                }
            } else {
                let mut statement = conn.prepare(
                    format!("{MCP_APPROVAL_REQUEST_SELECT} ORDER BY approval_id").as_str(),
                )?;
                let rows = statement.query_map([], mcp_approval_request_from_row)?;
                for row in rows {
                    approvals.push(row?);
                }
            }
            Ok(approvals)
        })
        .await
    }

    async fn get_mcp_approval_request(
        &self,
        approval_id: &str,
    ) -> Result<Option<McpApprovalRequestRecord>, StoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                format!("{MCP_APPROVAL_REQUEST_SELECT} WHERE approval_id = ?1 LIMIT 1").as_str(),
                params![approval_id],
                mcp_approval_request_from_row,
            )
            .optional()
            .map_err(StoreError::Sqlite)
        })
        .await
    }
}

const MCP_APPROVAL_REQUEST_SELECT: &str = r#"
SELECT approval_id, action_summary, tool_invocation_id, mcp_server_id,
  mcp_tool_id, requester_actor_id, owner_scope_id, active_scope_id,
  destination_summary, data_source_summary, source_owner_identity,
  source_owner_trust, destination_owner_identity, destination_owner_trust,
  export_summary, payload_preview_json, status, decision_actor_id,
  decision_comment, decided_at
FROM approval_requests
"#;

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
