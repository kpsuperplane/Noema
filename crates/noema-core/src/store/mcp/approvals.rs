use rusqlite::{OptionalExtension, params};

use super::{McpApprovalRequestRecord, NewMcpApprovalRequest, rows::mcp_approval_request_from_row};
use crate::store::{NoemaStore, StoreError, sqlite::now_timestamp_sql};

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
        if approval.mcp_server_id.is_some() != approval.mcp_tool_id.is_some() {
            return Err(StoreError::InvariantViolation {
                message: "MCP approval server and tool references must both be present or absent"
                    .to_string(),
            });
        }
        self.ensure_default_actors().await?;
        let approval_id = approval.approval_id.clone();
        self.with_connection(|conn| {
            conn.execute(
                format!(
                    r#"
                    INSERT INTO approval_requests (
                      approval_id, action_summary, tool_invocation_id, mcp_server_id,
                      mcp_tool_id, requester_human_id, requester_agent_id,
                      owner_human_id, owner_agent_id, active_human_id, active_agent_id,
                      destination_summary, data_source_summary, source_owner_identity,
                      source_owner_trust, destination_owner_identity, destination_owner_trust,
                      export_summary, redacted_review_json, attempt_fingerprint, status,
                      decision_human_id, decision_agent_id, decision_comment, decided_at, updated_at
                    )
                    VALUES (
                      ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                      ?14, ?15, ?16, ?17, ?18, ?19, ?3, 'pending', NULL, NULL, NULL, NULL, {}
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
                    concrete_human(&approval.requester_actor_id),
                    concrete_agent(&approval.requester_actor_id),
                    concrete_human(&approval.owner_scope_id),
                    concrete_agent(&approval.owner_scope_id),
                    concrete_human(&approval.active_scope_id),
                    concrete_agent(&approval.active_scope_id),
                    approval.destination_summary,
                    approval.data_source_summary,
                    approval.source_owner_identity,
                    approval.source_owner_trust,
                    approval.destination_owner_identity,
                    approval.destination_owner_trust,
                    approval.export_summary,
                    "{}",
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
  mcp_tool_id, COALESCE(requester_human_id, requester_agent_id),
  COALESCE(owner_human_id, owner_agent_id), COALESCE(active_human_id, active_agent_id),
  destination_summary, data_source_summary, source_owner_identity,
  source_owner_trust, destination_owner_identity, destination_owner_trust,
  export_summary, redacted_review_json, status, COALESCE(decision_human_id, decision_agent_id),
  decision_comment, decided_at
FROM approval_requests
"#;

fn concrete_human(value: &str) -> Option<&str> {
    value.starts_with("human:").then_some(value)
}

fn concrete_agent(value: &str) -> Option<&str> {
    (!value.starts_with("human:")).then_some(value)
}
