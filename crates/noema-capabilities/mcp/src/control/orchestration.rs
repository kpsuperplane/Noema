use crate::{
    ContinueMcpServerSetupCommand, CreateMcpServerCommand, LocalMcpService, McpClientError,
    McpConnectionReplacement, McpDiscoveryCommit, McpFailureStatus, McpInitialDiscoveryCommit,
    McpOperationError, McpSecretMaterial, McpServerHealthStatus, McpServerRecord,
    McpServerSetupResult, McpSetupAuthPreference, McpTransportKind, NewMcpServer,
    service::{map_client_operation_error, map_secret_operation_error},
    setup::{
        auth_status_for_secrets, merge_secret_material, preview_server,
        rotate_secret_identity_revision, safe_config_with_secret_refs,
        secret_identity_revision_matches, validate_create_command, validate_discovered_tools,
    },
};

use super::support::{
    SetupFailureProjection, SetupRunError, authentication_available_result,
    client_failure_projection, malformed_projection, setup_failure_result, success_result,
    validate_id,
};

impl LocalMcpService {
    pub(super) async fn create_server_run(
        &self,
        command: CreateMcpServerCommand,
    ) -> Result<McpServerSetupResult, SetupRunError> {
        let setup = validate_create_command(command)
            .map_err(|_| SetupRunError::discovery(McpOperationError::InvalidInput))?;
        let mut server = setup.server;
        let mut secrets = setup.secrets;
        let mut stage = self.stage_secrets(&secrets, None, "create_server")?;
        let context = self
            .inner
            .request_context(self.inner.config.discovery_timeout);
        let preview = preview_server(&server, "pending-setup");
        let preparation = match self
            .inner
            .sessions
            .prepare(&preview, &secrets, &context)
            .await
        {
            Ok(preparation) => preparation,
            Err(error) => {
                self.discard_stage(stage, None, "create_server");
                return self.unpersisted_client_failure(server.transport_kind, &error);
            }
        };
        let (mut session, refreshed) = preparation.into_parts();
        if let Some(credentials) = refreshed {
            secrets.oauth_credentials = Some(credentials);
            server.safe_config = match safe_config_with_secret_refs(server.safe_config, &secrets) {
                Ok(safe_config) => safe_config,
                Err(error) => {
                    self.diagnostic(
                        None,
                        "create_server",
                        "MCP refreshed credential metadata was invalid",
                        &error,
                    );
                    self.inner
                        .close_session(session, None, "create_server_cleanup")
                        .await;
                    self.discard_stage(stage, None, "create_server");
                    return Err(SetupRunError::credentials(McpOperationError::Unavailable));
                }
            };
            self.discard_stage(stage, None, "create_server");
            stage = match self.stage_secrets(&secrets, None, "create_server") {
                Ok(stage) => stage,
                Err(error) => {
                    self.inner
                        .close_session(session, None, "create_server_cleanup")
                        .await;
                    return Err(error);
                }
            };
        }
        let discovered = session.discover_tools(&context).await;
        self.inner
            .close_session(session, None, "create_server_cleanup")
            .await;
        let discovered = match discovered {
            Ok(tools) => tools,
            Err(error) => {
                self.discard_stage(stage, None, "create_server");
                return self.unpersisted_client_failure(server.transport_kind, &error);
            }
        };
        let tools = match validate_discovered_tools(discovered) {
            Ok(tools) => tools,
            Err(error) => {
                self.diagnostic(
                    None,
                    "tools/list",
                    "MCP discovery returned unsupported tool metadata",
                    &error,
                );
                self.discard_stage(stage, None, "create_server");
                return Ok(setup_failure_result(None, malformed_projection()));
            }
        };
        if command_authentication_should_be_offered(setup.auth_preference, &secrets, &server).await
        {
            let tool_count = tools.len();
            self.discard_stage(stage, None, "create_server");
            return Ok(authentication_available_result(tool_count));
        }
        let auth_status = auth_status_for_secrets(&secrets);
        let joined = match self
            .inner
            .repository
            .commit_initial_discovery(McpInitialDiscoveryCommit {
                server,
                tools,
                auth_status,
            })
            .await
        {
            Ok(joined) => joined,
            Err(error) => {
                self.discard_stage(stage, None, "create_server");
                return Err(SetupRunError::discovery(self.repository_error(
                    None,
                    "commit_initial_discovery",
                    &error,
                )));
            }
        };
        let id = joined.server.mcp_server_id.clone();
        let commit = match self.inner.secrets.commit(stage, &id) {
            Ok(commit) => commit,
            Err(error) => {
                self.diagnostic(
                    Some(&id),
                    "create_server",
                    "MCP credentials could not be committed after discovery",
                    &error,
                );
                self.compensate_initial_commit(&id).await;
                return Err(SetupRunError::credentials(map_secret_operation_error(
                    &error,
                )));
            }
        };
        self.finalize_secret_commit(commit, Some(&id), "create_server");
        Ok(success_result(joined.server, joined.tools.len()))
    }

    pub(super) async fn continue_setup_run(
        &self,
        command: ContinueMcpServerSetupCommand,
        expected_generation: Option<String>,
    ) -> Result<McpServerSetupResult, SetupRunError> {
        let id = validate_id(command.mcp_server_id).map_err(SetupRunError::discovery)?;
        let context = self
            .inner
            .request_context(self.inner.config.discovery_timeout);
        let _lock = self
            .inner
            .lock_server(&id, &context)
            .await
            .map_err(SetupRunError::discovery)?;
        let joined = self
            .inner
            .repository
            .control_plane_server(id.clone())
            .await
            .map_err(|error| {
                SetupRunError::discovery(self.repository_error(Some(&id), "continue_setup", &error))
            })?
            .ok_or_else(|| SetupRunError::discovery(McpOperationError::NotFound))?;
        if expected_generation
            .as_deref()
            .is_some_and(|expected| expected != joined.server.authority_generation)
        {
            return Err(SetupRunError::discovery(McpOperationError::Conflict));
        }
        let current_secrets = self.inner.secrets.load(&id).map_err(|error| {
            self.diagnostic(
                Some(&id),
                "continue_setup",
                "MCP credentials could not be read",
                &error,
            );
            SetupRunError::credentials(map_secret_operation_error(&error))
        })?;
        let replacement_supplied = command.secrets.has_secret_material();
        if !secret_identity_revision_matches(&joined.server.safe_config, &current_secrets)
            && !replacement_supplied
        {
            self.diagnostic(
                Some(&id),
                "continue_setup",
                "MCP credential generation did not match the connection fence",
                &"replacement credentials are required",
            );
            return Err(SetupRunError::credentials(McpOperationError::Unavailable));
        }
        let mut secrets = merge_secret_material(current_secrets.clone(), command.secrets);
        let mut safe_config =
            safe_config_with_secret_refs(joined.server.safe_config.clone(), &secrets)
                .map_err(|_| SetupRunError::credentials(McpOperationError::Unavailable))?;
        if replacement_supplied {
            safe_config = rotate_secret_identity_revision(safe_config, &mut secrets)
                .map_err(|_| SetupRunError::credentials(McpOperationError::Unavailable))?;
        }
        let mut server = self
            .replace_connection_and_secrets(joined.server, &secrets, safe_config)
            .await?;
        let preparation = match self
            .inner
            .sessions
            .prepare(&server, &secrets, &context)
            .await
        {
            Ok(preparation) => preparation,
            Err(error) => return self.persist_client_failure(server, &error).await,
        };
        let (mut session, refreshed) = preparation.into_parts();
        if let Some(credentials) = refreshed {
            secrets.oauth_credentials = Some(credentials);
            let safe_config =
                match safe_config_with_secret_refs(server.safe_config.clone(), &secrets) {
                    Ok(safe_config) => safe_config,
                    Err(error) => {
                        self.diagnostic(
                            Some(&id),
                            "continue_setup",
                            "MCP refreshed credential metadata was invalid",
                            &error,
                        );
                        self.inner
                            .close_session(session, Some(&id), "continue_setup_cleanup")
                            .await;
                        return Err(SetupRunError::credentials(McpOperationError::Unavailable));
                    }
                };
            server = match self
                .replace_connection_and_secrets(server, &secrets, safe_config)
                .await
            {
                Ok(server) => server,
                Err(error) => {
                    self.inner
                        .close_session(session, Some(&id), "continue_setup_cleanup")
                        .await;
                    return Err(error);
                }
            };
        }
        let discovered = session.discover_tools(&context).await;
        self.inner
            .close_session(session, Some(&id), "continue_setup_cleanup")
            .await;
        let discovered = match discovered {
            Ok(tools) => tools,
            Err(error) => return self.persist_client_failure(server, &error).await,
        };
        let tools = match validate_discovered_tools(discovered) {
            Ok(tools) => tools,
            Err(error) => {
                self.diagnostic(
                    Some(&id),
                    "tools/list",
                    "MCP discovery returned unsupported tool metadata",
                    &error,
                );
                return self
                    .persist_setup_failure(server, malformed_projection())
                    .await;
            }
        };
        let joined = self
            .inner
            .repository
            .commit_discovery(McpDiscoveryCommit {
                mcp_server_id: id.clone(),
                expected_authority_generation: server.authority_generation,
                tools,
                health_status: McpServerHealthStatus::Healthy,
                auth_status: auth_status_for_secrets(&secrets),
            })
            .await
            .map_err(|error| {
                SetupRunError::discovery(self.repository_error(
                    Some(&id),
                    "commit_discovery",
                    &error,
                ))
            })?;
        Ok(success_result(joined.server, joined.tools.len()))
    }

    async fn replace_connection_and_secrets(
        &self,
        server: McpServerRecord,
        secrets: &McpSecretMaterial,
        safe_config: serde_json::Value,
    ) -> Result<McpServerRecord, SetupRunError> {
        let id = server.mcp_server_id.clone();
        let stage = self.stage_secrets(secrets, Some(&id), "continue_setup")?;
        let replacement = self
            .inner
            .repository
            .replace_connection(McpConnectionReplacement {
                mcp_server_id: id.clone(),
                expected_authority_generation: server.authority_generation,
                transport_kind: server.transport_kind,
                safe_config,
            })
            .await;
        let replacement = match replacement {
            Ok(server) => server,
            Err(error) => {
                self.discard_stage(stage, Some(&id), "continue_setup");
                return Err(SetupRunError::discovery(self.repository_error(
                    Some(&id),
                    "replace_connection",
                    &error,
                )));
            }
        };
        let commit = self.inner.secrets.commit(stage, &id).map_err(|error| {
            self.diagnostic(
                Some(&id),
                "continue_setup",
                "MCP credentials could not be committed after the connection was fenced",
                &error,
            );
            SetupRunError::credentials(map_secret_operation_error(&error))
        })?;
        self.finalize_secret_commit(commit, Some(&id), "continue_setup");
        Ok(replacement)
    }

    async fn persist_client_failure(
        &self,
        server: McpServerRecord,
        error: &McpClientError,
    ) -> Result<McpServerSetupResult, SetupRunError> {
        self.diagnostic(
            Some(&server.mcp_server_id),
            "tools/list",
            "MCP setup discovery failed",
            error,
        );
        let projection = client_failure_projection(server.transport_kind, error);
        let terminal_error = match error {
            McpClientError::Timeout { .. } | McpClientError::Cancelled { .. } => {
                Some(map_client_operation_error(error))
            }
            _ => None,
        };
        let result = self.persist_setup_failure(server, projection).await?;
        terminal_error.map_or(Ok(result), |error| Err(SetupRunError::discovery(error)))
    }

    async fn persist_setup_failure(
        &self,
        mut server: McpServerRecord,
        projection: SetupFailureProjection,
    ) -> Result<McpServerSetupResult, SetupRunError> {
        let id = server.mcp_server_id.clone();
        let updated = self
            .inner
            .repository
            .record_failure_status(McpFailureStatus {
                mcp_server_id: id.clone(),
                expected_authority_generation: server.authority_generation.clone(),
                health_status: projection.health_status,
                auth_status: projection.auth_status,
            })
            .await
            .map_err(|error| {
                SetupRunError::discovery(self.repository_error(
                    Some(&id),
                    "record_failure_status",
                    &error,
                ))
            })?;
        if !updated {
            return Err(SetupRunError::discovery(McpOperationError::Conflict));
        }
        server.health_status = projection.health_status;
        server.auth_status = projection.auth_status;
        Ok(setup_failure_result(Some(server), projection))
    }

    fn unpersisted_client_failure(
        &self,
        transport_kind: McpTransportKind,
        error: &McpClientError,
    ) -> Result<McpServerSetupResult, SetupRunError> {
        self.diagnostic(None, "tools/list", "MCP setup discovery failed", error);
        if matches!(
            error,
            McpClientError::Timeout { .. } | McpClientError::Cancelled { .. }
        ) {
            return Err(SetupRunError::discovery(map_client_operation_error(error)));
        }
        Ok(setup_failure_result(
            None,
            client_failure_projection(transport_kind, error),
        ))
    }
}

async fn command_authentication_should_be_offered(
    preference: McpSetupAuthPreference,
    secrets: &McpSecretMaterial,
    server: &NewMcpServer,
) -> bool {
    if preference == McpSetupAuthPreference::UseAnonymous
        || secrets.oauth_client_credentials.is_some()
        || secrets.oauth_credentials.is_some()
    {
        return false;
    }
    if server.transport_kind != McpTransportKind::StreamableHttp {
        return false;
    }
    let Some(endpoint) = server
        .safe_config
        .get("url")
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    crate::oauth::resolved_resource(endpoint).await.is_some()
}
