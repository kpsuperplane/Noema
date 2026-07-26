//! Resumption of a background run after an MCP authentication interruption.

use noema_tasks::WorkDomainError;

use super::{
    WorkCommandService,
    governed_action_resume::{InterventionRunResult, resume_waiting_run_tx},
};
use crate::{McpAuthenticationRequestState, StoreError};

impl WorkCommandService {
    /// Complete a waiting parent and queue one pinned child after authentication.
    pub async fn resume_after_mcp_authentication(
        &self,
        request_id: &str,
        revision: u64,
        actor_id: &str,
    ) -> Result<Option<String>, StoreError> {
        let request = self
            .store
            .get_mcp_authentication_request(request_id, revision)
            .await?
            .ok_or(StoreError::Work(WorkDomainError::WorkUnavailable))?;
        if !matches!(
            request.state,
            McpAuthenticationRequestState::Completed
                | McpAuthenticationRequestState::Cancelled
                | McpAuthenticationRequestState::Superseded
        ) {
            return Err(StoreError::Work(WorkDomainError::InvalidTransition));
        }
        let (Some(task_id), Some(run_id)) = (request.task_id.as_deref(), request.run_id.as_deref())
        else {
            return Ok(None);
        };
        let success = request
            .output
            .as_ref()
            .and_then(|output| output.get("success"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let status = if success {
            "completed"
        } else if request.state == McpAuthenticationRequestState::Cancelled
            || request.state == McpAuthenticationRequestState::Superseded
        {
            "skipped"
        } else {
            "failed"
        };
        let payload = serde_json::to_string(&serde_json::json!({
            "mcp_authentication": {
                "request_id": request.request_id,
                "revision": request.revision,
                "capability_name": request.capability_name,
                "state": request.state.as_str(),
                "output": request.output,
                "failure_code": request.failure_code,
            }
        }))?;
        self.store
            .with_immediate_transaction_retry(|transaction| {
                resume_waiting_run_tx(
                    transaction,
                    self.provider_registry.as_ref(),
                    task_id,
                    run_id,
                    actor_id,
                    InterventionRunResult {
                        item_id: format!("run_item:mcp_authentication:{}", request.request_id),
                        status,
                        correlation_id: request.request_id.clone(),
                        content: format!(
                            "MCP authentication {} {}",
                            request.capability_name,
                            request.state.as_str()
                        ),
                        payload: payload.clone(),
                    },
                )
            })
            .await
    }
}
