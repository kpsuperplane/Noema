//! Local MCP control-plane orchestration.

mod orchestration;
mod support;

use crate::{
    AddMcpConnectionCommand, CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand,
    CreateMcpServerCommand, LocalMcpService, McpDeleteServerCommand, McpDeleteServerResult,
    McpListToolsCommand, McpOAuthAttemptContext, McpOAuthSetupAttemptQuery,
    McpOAuthSetupAttemptView, McpOAuthSetupFailure, McpOAuthStartRequest, McpOperationError,
    McpOperationFuture, McpOperationResult, McpOperations, McpProviderPolicyUpdate,
    McpResetToolPolicyCommand, McpSaveConnectionLabelCommand, McpSaveProviderPolicyCommand,
    McpSaveToolOverrideCommand, McpSecretMaterial, McpServerList, McpServerSetupResult,
    McpSetToolEnabledCommand, McpSetupStatus, McpToolClassificationRequest, McpToolList,
    McpToolPolicyRecord, McpToolPolicyStatus, StartMcpOAuthReauthenticationCommand,
    StartMcpOAuthSetupCommand, setup::validate_create_command,
};
use noema_capabilities::{
    apply_tool_classification, apply_tool_safe_defaults, build_tool_classification_prompt,
    parse_tool_classification_response,
};

use support::{streamable_http_url, validate_id};

const CLASSIFICATION_INSTRUCTIONS: &str =
    "Return only the requested MCP behavior hints as one strict JSON object.";

impl McpOperations for LocalMcpService {
    fn list_servers(&self) -> McpOperationFuture<'_, McpOperationResult<McpServerList>> {
        Box::pin(self.run_admitted_operation(async move {
            let catalog = self
                .inner
                .repository
                .control_plane_catalog()
                .await
                .map_err(|error| self.repository_error(None, "list_servers", &error))?;
            Ok(McpServerList {
                servers: catalog.into_iter().map(|entry| entry.server).collect(),
            })
        }))
    }

    fn list_tools(
        &self,
        command: McpListToolsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolList>> {
        Box::pin(self.run_admitted_operation(async move {
            let id = validate_id(command.mcp_server_id)?;
            let joined = self
                .inner
                .repository
                .control_plane_server(id.clone())
                .await
                .map_err(|error| self.repository_error(Some(&id), "list_tools", &error))?
                .ok_or(McpOperationError::NotFound)?;
            Ok(McpToolList {
                server: joined.server,
                tools: joined.tools,
            })
        }))
    }

    fn create_server(
        &self,
        command: CreateMcpServerCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>> {
        Box::pin(self.run_admitted_operation(async move {
            let result = self
                .create_server_run(command)
                .await
                .map_err(|error| error.operation)?;
            self.schedule_result_classification(&result).await;
            Ok(result)
        }))
    }

    fn add_connection(
        &self,
        command: AddMcpConnectionCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>> {
        Box::pin(self.run_admitted_operation(async move {
            let result = self
                .add_connection_run(command)
                .await
                .map_err(|error| error.operation)?;
            self.schedule_result_classification(&result).await;
            Ok(result)
        }))
    }

    fn continue_setup(
        &self,
        command: ContinueMcpServerSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>> {
        Box::pin(self.run_admitted_operation(async move {
            let result = self
                .continue_setup_run(command, None, None, None)
                .await
                .map_err(|error| error.operation)?;
            self.schedule_result_classification(&result).await;
            Ok(result)
        }))
    }

    fn start_oauth_setup(
        &self,
        command: StartMcpOAuthSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpOAuthSetupAttemptView>> {
        Box::pin(self.run_admitted_operation(async move {
            validate_create_command(command.setup.clone())
                .map_err(|_| McpOperationError::InvalidInput)?;
            self.inner
                .oauth
                .start_attempt(McpOAuthStartRequest {
                    context: McpOAuthAttemptContext::PendingCreate {
                        owner_human_id: command.owner_human_id,
                        command: Box::new(command.setup),
                    },
                    redirect_uri: command.redirect_uri,
                })
                .await
                .map_err(|error| self.oauth_error(None, "start_oauth_setup", &error))
        }))
    }

    fn start_oauth_reauthentication(
        &self,
        command: StartMcpOAuthReauthenticationCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpOAuthSetupAttemptView>> {
        Box::pin(self.run_admitted_operation(async move {
            let id = validate_id(command.mcp_server_id)?;
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let _lock = self.inner.lock_server(&id, &context).await?;
            let joined = self
                .inner
                .repository
                .control_plane_server(id.clone())
                .await
                .map_err(|error| {
                    self.repository_error(Some(&id), "start_oauth_reauthentication", &error)
                })?
                .ok_or(McpOperationError::NotFound)?;
            let mcp_url = streamable_http_url(&joined.server).ok_or(McpOperationError::Conflict)?;
            self.inner
                .oauth
                .start_attempt(McpOAuthStartRequest {
                    context: McpOAuthAttemptContext::Reauthenticate {
                        mcp_server_id: id.clone(),
                        owner_human_id: command.owner_human_id,
                        expected_authority_generation: joined.server.authority_generation,
                        expected_policy_revision: joined.server.policy_revision,
                        mcp_url,
                    },
                    redirect_uri: command.redirect_uri,
                })
                .await
                .map_err(|error| {
                    self.oauth_error(Some(&id), "start_oauth_reauthentication", &error)
                })
        }))
    }

    fn oauth_setup_attempt(
        &self,
        query: McpOAuthSetupAttemptQuery,
    ) -> McpOperationFuture<'_, McpOperationResult<Option<McpOAuthSetupAttemptView>>> {
        Box::pin(self.run_admitted_operation(async move {
            if query.attempt_id.trim().is_empty()
                || query
                    .owner_human_id
                    .as_deref()
                    .is_some_and(|owner| owner.trim().is_empty())
            {
                return Err(McpOperationError::InvalidInput);
            }
            Ok(match query.owner_human_id.as_deref() {
                Some(owner) => {
                    self.inner
                        .oauth
                        .attempt_for_owner(&query.attempt_id, owner)
                        .await
                }
                None => self.inner.oauth.attempt(&query.attempt_id).await,
            })
        }))
    }

    fn complete_oauth_setup(
        &self,
        command: CompleteMcpOAuthSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpOAuthSetupAttemptView>> {
        Box::pin(self.run_admitted_operation(async move {
            let completion = self
                .inner
                .oauth
                .complete_callback(command)
                .await
                .map_err(|error| self.oauth_error(None, "complete_oauth_setup", &error))?;
            let result = match completion.context.clone() {
                McpOAuthAttemptContext::PendingCreate { command, .. } => {
                    self.inner
                        .oauth
                        .claim_persistence(&completion)
                        .await
                        .map_err(|error| {
                            self.oauth_error(None, "claim_oauth_persistence", &error)
                        })?;
                    let mut command = *command;
                    command.secrets.oauth_credentials = Some(completion.credentials.clone());
                    self.create_server_run(command).await
                }
                McpOAuthAttemptContext::Reauthenticate {
                    mcp_server_id,
                    expected_authority_generation,
                    expected_policy_revision,
                    ..
                } => {
                    self.continue_setup_run(
                        ContinueMcpServerSetupCommand {
                            mcp_server_id,
                            secrets: McpSecretMaterial {
                                oauth_credentials: Some(completion.credentials.clone()),
                                ..McpSecretMaterial::default()
                            },
                        },
                        Some(expected_authority_generation),
                        Some(expected_policy_revision),
                        Some(&completion),
                    )
                    .await
                }
            };
            if let Ok(result) = &result {
                self.schedule_result_classification(result).await;
            }
            let finished = match result {
                Ok(result) if result.setup_status == McpSetupStatus::ReadyForPolicy => {
                    self.inner.oauth.finish_success(&completion, result).await
                }
                Ok(_) => {
                    self.inner
                        .oauth
                        .finish_failure(&completion, McpOAuthSetupFailure::DiscoveryFailed)
                        .await
                }
                Err(error) => {
                    self.inner
                        .oauth
                        .finish_failure(&completion, error.oauth_failure)
                        .await
                }
            };
            finished.map_err(|error| self.oauth_error(None, "finish_oauth_setup", &error))
        }))
    }

    fn save_provider_policy(
        &self,
        command: McpSaveProviderPolicyCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<crate::McpServerRecord>> {
        Box::pin(self.run_admitted_operation(async move {
            let id = validate_id(command.mcp_server_id)?;
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let _policy = self
                .inner
                .lock_policy_write(vec![id.clone()], &context)
                .await?;
            let server = self
                .inner
                .repository
                .save_provider_policy(McpProviderPolicyUpdate {
                    mcp_server_id: id.clone(),
                    data_sharing_policy: command.data_sharing_policy,
                    unsafe_action_policy: command.unsafe_action_policy,
                    expected_policy_revision: command.expected_policy_revision,
                    expected_connection_revision: command.expected_connection_revision,
                })
                .await
                .map_err(|error| {
                    self.repository_error(Some(&id), "save_provider_policy", &error)
                })?;
            Ok(server)
        }))
    }

    fn save_connection_label(
        &self,
        command: McpSaveConnectionLabelCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<crate::McpServerRecord>> {
        Box::pin(self.run_admitted_operation(async move {
            let id = validate_id(command.mcp_server_id)?;
            let connection_label =
                noema_capabilities::normalize_capability_connection_label(command.connection_label)
                    .map_err(|_| McpOperationError::InvalidInput)?;
            let expected_connection_label =
                noema_capabilities::normalize_capability_connection_label(
                    command.expected_connection_label,
                )
                .map_err(|_| McpOperationError::InvalidInput)?;
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let _lock = self.inner.lock_server(&id, &context).await?;
            self.inner
                .repository
                .save_connection_label(crate::McpConnectionLabelUpdate {
                    mcp_server_id: id.clone(),
                    expected_authority_generation: command.expected_connection_revision,
                    expected_connection_label,
                    connection_label,
                })
                .await
                .map_err(|error| self.repository_error(Some(&id), "save_connection_label", &error))
        }))
    }

    fn save_tool_override(
        &self,
        command: McpSaveToolOverrideCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolPolicyRecord>> {
        Box::pin(self.run_admitted_operation(async move {
            let server_id = self.server_id_for_tool(&command.policy.tool_id).await?;
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let _policy = self
                .inner
                .lock_policy_write(vec![server_id.clone()], &context)
                .await?;
            self.inner
                .repository
                .save_tool_override(crate::McpToolPolicyOverrideUpdate {
                    policy: command.policy,
                    expected_policy_revision: command.expected_policy_revision,
                    expected_connection_revision: command.expected_connection_revision,
                })
                .await
                .map_err(|error| {
                    self.repository_error(Some(&server_id), "save_tool_override", &error)
                })
        }))
    }

    fn reset_tool_policy(
        &self,
        command: McpResetToolPolicyCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolPolicyRecord>> {
        Box::pin(self.run_admitted_operation(async move {
            let tool_id = validate_id(command.mcp_tool_id)?;
            let server_id = self.server_id_for_tool(&tool_id).await?;
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let _policy = self
                .inner
                .lock_policy_write(vec![server_id.clone()], &context)
                .await?;
            let policy = self
                .inner
                .repository
                .reset_tool_policy(crate::McpResetToolPolicyUpdate {
                    mcp_tool_id: tool_id,
                    source_revision: command.source_revision,
                    expected_policy_revision: command.expected_policy_revision,
                    expected_connection_revision: command.expected_connection_revision,
                })
                .await
                .map_err(|error| {
                    self.repository_error(Some(&server_id), "reset_tool_policy", &error)
                })?;
            drop(_policy);
            self.schedule_server_classification(&server_id).await;
            Ok(policy)
        }))
    }

    fn set_tool_enabled(
        &self,
        command: McpSetToolEnabledCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpToolPolicyRecord>> {
        Box::pin(self.run_admitted_operation(async move {
            let tool_id = validate_id(command.mcp_tool_id)?;
            let server_id = self.server_id_for_tool(&tool_id).await?;
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let _policy = self
                .inner
                .lock_policy_write(vec![server_id.clone()], &context)
                .await?;
            let policy = self
                .inner
                .repository
                .set_tool_enabled(crate::McpSetToolEnabledUpdate {
                    mcp_tool_id: tool_id,
                    source_revision: command.source_revision,
                    expected_policy_revision: command.expected_policy_revision,
                    expected_connection_revision: command.expected_connection_revision,
                    enabled: command.enabled,
                })
                .await
                .map_err(|error| {
                    self.repository_error(Some(&server_id), "set_tool_enabled", &error)
                })?;
            drop(_policy);
            Ok(policy)
        }))
    }

    fn delete_server(
        &self,
        command: McpDeleteServerCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpDeleteServerResult>> {
        Box::pin(self.run_admitted_operation(async move {
            let id = validate_id(command.mcp_server_id)?;
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let _lock = self.inner.lock_server(&id, &context).await?;
            let Some(ticket) = self
                .inner
                .repository
                .begin_delete(id.clone())
                .await
                .map_err(|error| self.repository_error(Some(&id), "begin_delete", &error))?
            else {
                return Ok(McpDeleteServerResult { deleted: false });
            };
            self.inner.oauth.supersede_server(&id).await;
            if let Err(error) = self.inner.secrets.remove(&id) {
                self.diagnostic(
                    Some(&id),
                    "delete_server",
                    "MCP secret cleanup failed after the server was fenced",
                    &error,
                );
            }
            let deleted = self
                .inner
                .repository
                .finish_delete(ticket)
                .await
                .map_err(|error| self.repository_error(Some(&id), "finish_delete", &error))?;
            Ok(McpDeleteServerResult { deleted })
        }))
    }
}

impl LocalMcpService {
    async fn server_id_for_tool(&self, mcp_tool_id: &str) -> McpOperationResult<String> {
        let catalog = self
            .inner
            .repository
            .control_plane_catalog()
            .await
            .map_err(|error| self.repository_error(None, "resolve_tool", &error))?;
        catalog
            .into_iter()
            .find(|server| {
                server
                    .tools
                    .iter()
                    .any(|tool| tool.tool.mcp_tool_id == mcp_tool_id)
            })
            .map(|server| server.server.mcp_server_id)
            .ok_or(McpOperationError::NotFound)
    }

    async fn schedule_result_classification(&self, result: &McpServerSetupResult) {
        if let Some(server) = result.server.as_ref()
            && result.setup_status == McpSetupStatus::ReadyForPolicy
        {
            self.schedule_server_classification(&server.mcp_server_id)
                .await;
        }
    }

    async fn schedule_server_classification(&self, mcp_server_id: &str) {
        let Ok(Some(server)) = self
            .inner
            .repository
            .control_plane_server(mcp_server_id.to_string())
            .await
        else {
            return;
        };
        for entry in server.tools {
            let Some(policy) = entry.policy else {
                continue;
            };
            if policy.status == McpToolPolicyStatus::Pending {
                self.schedule_tool_classification(entry.tool, policy).await;
            }
        }
    }

    async fn schedule_tool_classification(
        &self,
        tool: crate::McpToolRecord,
        policy: McpToolPolicyRecord,
    ) {
        let key = (
            policy.tool_id.clone(),
            policy.policy_revision,
            policy.source_revision.clone(),
        );
        if !self
            .inner
            .classification_jobs
            .lock()
            .await
            .insert(key.clone())
        {
            return;
        }
        let service = self.clone();
        tokio::spawn(async move {
            let task_service = service.clone();
            let _ = service
                .inner
                .lifecycle
                .run_admitted(task_service.classify_tool(tool, policy))
                .await;
            service.inner.classification_jobs.lock().await.remove(&key);
        });
    }

    async fn classify_tool(&self, tool: crate::McpToolRecord, policy: McpToolPolicyRecord) {
        let Ok(_permit) = self.inner.classification_gate.clone().acquire_owned().await else {
            return;
        };
        let classified = if let Some(completion) = self.inner.completion.as_ref() {
            match completion
                .complete(McpToolClassificationRequest {
                    instructions: CLASSIFICATION_INSTRUCTIONS.to_string(),
                    prompt: build_tool_classification_prompt(
                        &tool.name,
                        tool.description.as_deref(),
                        &tool.input_schema,
                        tool.output_schema.as_ref(),
                        &policy,
                    ),
                })
                .await
            {
                Ok(response) => {
                    match parse_tool_classification_response(&response.assistant_text, &policy) {
                        Ok(completion) => apply_tool_classification(policy, completion),
                        Err(error) => {
                            self.diagnostic(
                                Some(&tool.mcp_server_id),
                                "classify_tool",
                                "MCP tool classification returned invalid output",
                                &error,
                            );
                            apply_tool_safe_defaults(policy)
                        }
                    }
                }
                Err(error) => {
                    self.diagnostic(
                        Some(&tool.mcp_server_id),
                        "classify_tool",
                        "MCP tool classification was unavailable",
                        &error,
                    );
                    apply_tool_safe_defaults(policy)
                }
            }
        } else {
            apply_tool_safe_defaults(policy)
        };
        if let Err(error) = self.inner.repository.complete_tool_policy(classified).await {
            self.diagnostic(
                Some(&tool.mcp_server_id),
                "classify_tool",
                "MCP tool classification could not be committed",
                &error,
            );
        }
    }

    /// Resume genuinely pending per-tool classification after process startup.
    pub async fn resume_pending_tool_classification(&self) {
        let Ok(catalog) = self.inner.repository.control_plane_catalog().await else {
            return;
        };
        for server in catalog {
            self.schedule_server_classification(&server.server.mcp_server_id)
                .await;
        }
    }
}

#[cfg(test)]
mod tests;
