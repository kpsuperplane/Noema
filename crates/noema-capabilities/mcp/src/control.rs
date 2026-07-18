//! Local MCP control-plane orchestration.

mod orchestration;
mod support;

use std::collections::HashMap;

use crate::{
    CompleteMcpOAuthSetupCommand, ContinueMcpServerSetupCommand, CreateMcpServerCommand,
    LocalMcpService, McpAutofillCalibrationsCommand, McpAutofillCalibrationsResult,
    McpAutofillCompletionRequest, McpDeleteServerCommand, McpDeleteServerResult,
    McpListToolsCommand, McpOAuthAttemptContext, McpOAuthSetupAttemptQuery,
    McpOAuthSetupAttemptView, McpOAuthSetupFailure, McpOAuthStartRequest, McpOperationError,
    McpOperationFuture, McpOperationResult, McpOperations, McpSaveCalibrationsCommand,
    McpSaveCalibrationsResult, McpSecretMaterial, McpServerList, McpServerSetupResult,
    McpSetupStatus, McpToolList, StartMcpOAuthReauthenticationCommand, StartMcpOAuthSetupCommand,
    autofill::{build_autofill_prompt, parse_autofill_response},
    setup::validate_create_command,
};

use support::{map_completion_error, streamable_http_url, validate_id};

const AUTOFILL_INSTRUCTIONS: &str =
    "Return only strict JSON matching the requested MCP calibration suggestion schema.";

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
            self.create_server_run(command)
                .await
                .map_err(|error| error.operation)
        }))
    }

    fn continue_setup(
        &self,
        command: ContinueMcpServerSetupCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpServerSetupResult>> {
        Box::pin(self.run_admitted_operation(async move {
            self.continue_setup_run(command, None)
                .await
                .map_err(|error| error.operation)
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
                    context: McpOAuthAttemptContext::PendingCreate(Box::new(command.setup)),
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
                        expected_authority_generation: joined.server.authority_generation,
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
            if query.attempt_id.trim().is_empty() {
                return Err(McpOperationError::InvalidInput);
            }
            Ok(self.inner.oauth.attempt(&query.attempt_id).await)
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
                McpOAuthAttemptContext::PendingCreate(command) => {
                    let mut command = *command;
                    command.secrets.oauth_credentials = Some(completion.credentials.clone());
                    self.create_server_run(command).await
                }
                McpOAuthAttemptContext::Reauthenticate {
                    mcp_server_id,
                    expected_authority_generation,
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
                    )
                    .await
                }
            };
            let finished = match result {
                Ok(result) if result.setup_status == McpSetupStatus::ReadyForCalibration => {
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

    fn autofill_calibrations(
        &self,
        command: McpAutofillCalibrationsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpAutofillCalibrationsResult>> {
        Box::pin(self.run_admitted_operation(async move {
            let id = validate_id(command.mcp_server_id)?;
            let joined = self
                .inner
                .repository
                .control_plane_server(id.clone())
                .await
                .map_err(|error| self.repository_error(Some(&id), "autofill", &error))?
                .ok_or(McpOperationError::NotFound)?;
            let completion = self
                .inner
                .completion
                .as_ref()
                .ok_or(McpOperationError::Unavailable)?;
            let tools = joined
                .tools
                .iter()
                .map(|entry| entry.tool.clone())
                .collect::<Vec<_>>();
            let response = completion
                .complete(McpAutofillCompletionRequest {
                    instructions: AUTOFILL_INSTRUCTIONS.to_string(),
                    prompt: build_autofill_prompt(&joined.server.display_name, &tools),
                })
                .await
                .map_err(map_completion_error)?;
            let suggestions =
                parse_autofill_response(&response.assistant_text, &tools).map_err(|error| {
                    self.diagnostic(
                        Some(&id),
                        "autofill",
                        "MCP calibration autofill returned invalid output",
                        &error,
                    );
                    McpOperationError::MalformedResponse
                })?;
            Ok(McpAutofillCalibrationsResult { suggestions })
        }))
    }

    fn save_calibrations(
        &self,
        command: McpSaveCalibrationsCommand,
    ) -> McpOperationFuture<'_, McpOperationResult<McpSaveCalibrationsResult>> {
        Box::pin(self.run_admitted_operation(async move {
            let context = self
                .inner
                .request_context(self.inner.config.discovery_timeout);
            let catalog = self
                .inner
                .repository
                .control_plane_catalog()
                .await
                .map_err(|error| self.repository_error(None, "save_calibrations", &error))?;
            let tool_servers = catalog
                .iter()
                .flat_map(|entry| {
                    entry.tools.iter().map(|tool| {
                        (
                            tool.tool.mcp_tool_id.as_str(),
                            entry.server.mcp_server_id.as_str(),
                        )
                    })
                })
                .collect::<HashMap<_, _>>();
            let server_ids = command
                .calibrations
                .iter()
                .map(|calibration| {
                    tool_servers
                        .get(calibration.mcp_tool_id.as_str())
                        .map(|server_id| (*server_id).to_string())
                        .ok_or(McpOperationError::NotFound)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let _policy = self.inner.lock_policy_write(server_ids, &context).await?;
            let calibrations = self
                .inner
                .repository
                .save_calibrations(command.calibrations)
                .await
                .map_err(|error| self.repository_error(None, "save_calibrations", &error))?;
            Ok(McpSaveCalibrationsResult { calibrations })
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

#[cfg(test)]
mod tests;
