//! SQLite adapter for the task-oriented MCP repository contract.

use std::{collections::BTreeSet, future::Future, pin::Pin};

use noema_capabilities_mcp::{
    McpConnectionReplacement, McpControlPlaneServer, McpDefinitionRecord, McpDefinitionTarget,
    McpDeleteTicket, McpDiscoveredTool, McpDiscoveryCommit, McpFailureStatus,
    McpInitialDiscoveryCommit, McpInvocationSnapshot, McpProviderPolicyUpdate, McpRepository,
    McpRepositoryError, McpRepositoryErrorKind, McpRepositoryResult, McpResetToolPolicyUpdate,
    McpServerRecord, McpSetToolEnabledUpdate, McpToolPolicyOverrideUpdate, McpToolPolicyRecord,
};
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{Connection, ErrorCode, TransactionBehavior, params};

use crate::{NoemaStore, StoreError, governed_actions::insert_event};

#[path = "repository_calibrations.rs"]
mod policies;
#[path = "repository_rows.rs"]
mod rows;
#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;

const RANDOM_ID_ATTEMPTS: usize = 8;

type RepositoryFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl McpRepository for NoemaStore {
    fn definition(
        &self,
        mcp_definition_id: String,
    ) -> RepositoryFuture<'_, McpRepositoryResult<Option<McpDefinitionRecord>>> {
        Box::pin(with_repository_connection(self, move |connection| {
            rows::definition_on_connection(connection, &mcp_definition_id)
        }))
    }

    fn commit_initial_discovery(
        &self,
        input: McpInitialDiscoveryCommit,
    ) -> RepositoryFuture<'_, McpRepositoryResult<McpControlPlaneServer>> {
        Box::pin(with_repository_connection(self, move |connection| {
            commit_initial_discovery_on_connection(connection, input)
        }))
    }

    fn control_plane_server(
        &self,
        mcp_server_id: String,
    ) -> RepositoryFuture<'_, McpRepositoryResult<Option<McpControlPlaneServer>>> {
        Box::pin(with_repository_connection(self, move |connection| {
            rows::control_plane_server_on_connection(connection, &mcp_server_id)
        }))
    }

    fn control_plane_catalog(
        &self,
    ) -> RepositoryFuture<'_, McpRepositoryResult<Vec<McpControlPlaneServer>>> {
        Box::pin(with_repository_connection(self, |connection| {
            rows::control_plane_catalog_on_connection(connection)
        }))
    }

    fn invocation_snapshot(
        &self,
        mcp_server_id: String,
        mcp_tool_id: String,
    ) -> RepositoryFuture<'_, McpRepositoryResult<Option<McpInvocationSnapshot>>> {
        Box::pin(with_repository_connection(self, move |connection| {
            rows::invocation_snapshot_on_connection(connection, &mcp_server_id, &mcp_tool_id)
        }))
    }

    fn replace_connection(
        &self,
        input: McpConnectionReplacement,
    ) -> RepositoryFuture<'_, McpRepositoryResult<McpServerRecord>> {
        Box::pin(with_repository_connection(self, move |connection| {
            replace_connection_on_connection(connection, input)
        }))
    }

    fn commit_discovery(
        &self,
        input: McpDiscoveryCommit,
    ) -> RepositoryFuture<'_, McpRepositoryResult<McpControlPlaneServer>> {
        Box::pin(with_repository_connection(self, move |connection| {
            commit_discovery_on_connection(connection, input)
        }))
    }

    fn record_failure_status(
        &self,
        input: McpFailureStatus,
    ) -> RepositoryFuture<'_, McpRepositoryResult<bool>> {
        Box::pin(with_repository_connection(self, move |connection| {
            record_failure_status_on_connection(connection, input)
        }))
    }

    fn save_provider_policy(
        &self,
        update: McpProviderPolicyUpdate,
    ) -> RepositoryFuture<'_, McpRepositoryResult<McpServerRecord>> {
        Box::pin(with_repository_connection(self, move |connection| {
            policies::save_provider_policy_on_connection(connection, update)
        }))
    }

    fn save_tool_override(
        &self,
        update: McpToolPolicyOverrideUpdate,
    ) -> RepositoryFuture<'_, McpRepositoryResult<McpToolPolicyRecord>> {
        Box::pin(with_repository_connection(self, move |connection| {
            policies::save_tool_override_on_connection(connection, update)
        }))
    }

    fn reset_tool_policy(
        &self,
        update: McpResetToolPolicyUpdate,
    ) -> RepositoryFuture<'_, McpRepositoryResult<McpToolPolicyRecord>> {
        Box::pin(with_repository_connection(self, move |connection| {
            policies::reset_tool_policy_on_connection(connection, update)
        }))
    }

    fn set_tool_enabled(
        &self,
        update: McpSetToolEnabledUpdate,
    ) -> RepositoryFuture<'_, McpRepositoryResult<McpToolPolicyRecord>> {
        Box::pin(with_repository_connection(self, move |connection| {
            policies::set_tool_enabled_on_connection(connection, update)
        }))
    }

    fn complete_tool_policy(
        &self,
        policy: McpToolPolicyRecord,
    ) -> RepositoryFuture<'_, McpRepositoryResult<Option<McpToolPolicyRecord>>> {
        Box::pin(with_repository_connection(self, move |connection| {
            policies::complete_tool_policy_on_connection(connection, policy)
        }))
    }

    fn begin_delete(
        &self,
        mcp_server_id: String,
    ) -> RepositoryFuture<'_, McpRepositoryResult<Option<McpDeleteTicket>>> {
        Box::pin(with_repository_connection(self, move |connection| {
            begin_delete_on_connection(connection, &mcp_server_id)
        }))
    }

    fn finish_delete(
        &self,
        ticket: McpDeleteTicket,
    ) -> RepositoryFuture<'_, McpRepositoryResult<bool>> {
        Box::pin(with_repository_connection(self, move |connection| {
            finish_delete_on_connection(connection, ticket)
        }))
    }
}

async fn with_repository_connection<T>(
    store: &NoemaStore,
    work: impl FnOnce(&mut Connection) -> McpRepositoryResult<T> + Send,
) -> McpRepositoryResult<T>
where
    T: Send,
{
    store
        .with_connection(|connection| Ok::<_, StoreError>(work(connection)))
        .await
        .map_err(map_store_error)?
}

fn commit_initial_discovery_on_connection(
    connection: &mut Connection,
    input: McpInitialDiscoveryCommit,
) -> McpRepositoryResult<McpControlPlaneServer> {
    if input.server.display_name.trim().is_empty() {
        return Err(conflict_error());
    }
    let connection_label = input
        .connection_label
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty());
    let (definition_config, connection_config) = split_safe_config(input.server.safe_config)?;
    let safe_config_json = encode_json(&definition_config)?;
    let connection_config_json = encode_json(&connection_config)?;
    let tools = prepare_discovered_tools(input.tools)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let mcp_definition_id = match input.definition {
        McpDefinitionTarget::New => {
            let mcp_definition_id = allocate_unique_id(
                &transaction,
                "SELECT EXISTS(SELECT 1 FROM mcp_definitions WHERE mcp_definition_id = ?1)",
                "mcp_definition",
            )?;
            let definition_revision = random_id("mcp_definition_revision")?;
            transaction
                .execute(
                    r#"
                    INSERT INTO mcp_definitions (
                      mcp_definition_id, display_name, transport_kind,
                      safe_config_json, definition_revision
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                    params![
                        mcp_definition_id,
                        input.server.display_name,
                        input.server.transport_kind.as_str(),
                        safe_config_json,
                        definition_revision,
                    ],
                )
                .map_err(repo_sql_error)?;
            mcp_definition_id
        }
        McpDefinitionTarget::Existing {
            mcp_definition_id,
            expected_definition_revision,
        } => {
            let definition = rows::definition_on_connection(&transaction, &mcp_definition_id)?
                .ok_or_else(not_found_error)?;
            if definition.definition_revision != expected_definition_revision
                || definition.display_name != input.server.display_name
                || definition.transport_kind != input.server.transport_kind
                || definition.safe_config != definition_config
            {
                return Err(conflict_error());
            }
            mcp_definition_id
        }
    };
    let mcp_server_id = allocate_unique_id(
        &transaction,
        "SELECT EXISTS(SELECT 1 FROM mcp_servers WHERE mcp_server_id = ?1)",
        "mcp_server",
    )?;
    let authority_generation = random_id("mcp_generation")?;
    transaction
        .execute(
            r#"
            INSERT INTO mcp_servers (
              mcp_server_id, mcp_definition_id, connection_config_json, connection_label,
              auth_status, health_status, enabled, metadata_fingerprint, service_description,
              last_discovered_at, updated_at
            )
            VALUES (
              ?1, ?2, ?3, ?4, ?5, 'healthy', 0, ?6, ?7,
              strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
              strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            "#,
            params![
                mcp_server_id,
                mcp_definition_id,
                connection_config_json,
                connection_label,
                input.auth_status.as_str(),
                authority_generation,
                input.service_description,
            ],
        )
        .map_err(repo_sql_error)?;
    for tool in tools {
        insert_new_tool(&transaction, &mcp_server_id, tool)?;
    }
    let server = rows::control_plane_server_on_connection(&transaction, &mcp_server_id)?
        .ok_or_else(invariant_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(server)
}

fn replace_connection_on_connection(
    connection: &mut Connection,
    input: McpConnectionReplacement,
) -> McpRepositoryResult<McpServerRecord> {
    let (definition_config, connection_config) = split_safe_config(input.safe_config)?;
    let connection_config_json = encode_json(&connection_config)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let current = require_generation(
        &transaction,
        &input.mcp_server_id,
        &input.expected_authority_generation,
    )?;
    let (current_definition_config, current_connection_config) =
        split_safe_config(current.safe_config.clone())?;
    if current.transport_kind != input.transport_kind
        || current_definition_config != definition_config
    {
        return Err(conflict_error());
    }
    let identity_changed = current_connection_config != connection_config;
    let authority_generation = if identity_changed {
        fresh_authority_generation(&current.authority_generation)?
    } else {
        current.authority_generation
    };
    let affected = transaction
        .execute(
            r#"
            UPDATE mcp_servers SET
              connection_config_json = ?3,
              metadata_fingerprint = ?4,
              enabled = CASE WHEN ?5 THEN 0 ELSE enabled END,
              health_status = CASE WHEN ?5 THEN 'unknown' ELSE health_status END,
              last_discovered_at = CASE WHEN ?5 THEN NULL ELSE last_discovered_at END,
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_server_id = ?1 AND COALESCE(metadata_fingerprint, '') = ?2
            "#,
            params![
                input.mcp_server_id,
                input.expected_authority_generation,
                connection_config_json,
                authority_generation,
                identity_changed,
            ],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(conflict_error());
    }
    let server = rows::server_record_on_connection(&transaction, &input.mcp_server_id)?
        .ok_or_else(invariant_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(server)
}

fn commit_discovery_on_connection(
    connection: &mut Connection,
    input: McpDiscoveryCommit,
) -> McpRepositoryResult<McpControlPlaneServer> {
    let tools = prepare_discovered_tools(input.tools)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    require_generation(
        &transaction,
        &input.mcp_server_id,
        &input.expected_authority_generation,
    )?;
    let mut existing =
        rows::existing_tools_by_name_on_connection(&transaction, &input.mcp_server_id)?;
    for tool in tools {
        if let Some(previous) = existing.remove(&tool.name) {
            if previous.metadata_fingerprint != tool.metadata_fingerprint {
                update_discovered_tool(&transaction, &previous.mcp_tool_id, tool)?;
                let annotations = transaction
                    .query_row(
                        "SELECT annotations_json FROM mcp_tools WHERE mcp_tool_id = ?1",
                        params![previous.mcp_tool_id],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(repo_sql_error)?;
                let annotations =
                    serde_json::from_str(&annotations).map_err(|_| invariant_error())?;
                policies::seed_tool_policy(&transaction, &previous.mcp_tool_id, &annotations)?;
                continue;
            }
            update_discovered_tool(&transaction, &previous.mcp_tool_id, tool)?;
        } else {
            insert_new_tool(&transaction, &input.mcp_server_id, tool)?;
        }
    }
    for removed in existing.into_values() {
        transaction
            .execute(
                "DELETE FROM mcp_tool_policies WHERE mcp_tool_id = ?1",
                params![removed.mcp_tool_id],
            )
            .map_err(repo_sql_error)?;
        transaction
            .execute(
                "DELETE FROM mcp_tools WHERE mcp_tool_id = ?1",
                params![removed.mcp_tool_id],
            )
            .map_err(repo_sql_error)?;
    }
    let affected = transaction
        .execute(
            r#"
            UPDATE mcp_servers SET
              health_status = ?3,
              auth_status = ?4,
              service_description = COALESCE(?5, service_description),
              last_discovered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_server_id = ?1 AND COALESCE(metadata_fingerprint, '') = ?2
            "#,
            params![
                input.mcp_server_id,
                input.expected_authority_generation,
                input.health_status.as_str(),
                input.auth_status.as_str(),
                input.service_description,
            ],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(conflict_error());
    }
    recompute_server_enabled(&transaction, &input.mcp_server_id)?;
    let server = rows::control_plane_server_on_connection(&transaction, &input.mcp_server_id)?
        .ok_or_else(invariant_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(server)
}

fn record_failure_status_on_connection(
    connection: &Connection,
    input: McpFailureStatus,
) -> McpRepositoryResult<bool> {
    let affected = connection
        .execute(
            r#"
            UPDATE mcp_servers SET
              health_status = ?3,
              auth_status = ?4,
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_server_id = ?1 AND COALESCE(metadata_fingerprint, '') = ?2
            "#,
            params![
                input.mcp_server_id,
                input.expected_authority_generation,
                input.health_status.as_str(),
                input.auth_status.as_str(),
            ],
        )
        .map_err(repo_sql_error)?;
    Ok(affected == 1)
}

fn begin_delete_on_connection(
    connection: &mut Connection,
    mcp_server_id: &str,
) -> McpRepositoryResult<Option<McpDeleteTicket>> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let Some(server) = rows::server_record_on_connection(&transaction, mcp_server_id)? else {
        return Ok(None);
    };
    let deletion_generation = fresh_authority_generation(&server.authority_generation)?;
    let affected = transaction
        .execute(
            r#"
            UPDATE mcp_servers SET
              enabled = 0,
              health_status = 'unavailable',
              metadata_fingerprint = ?3,
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_server_id = ?1 AND COALESCE(metadata_fingerprint, '') = ?2
            "#,
            params![
                mcp_server_id,
                server.authority_generation,
                deletion_generation,
            ],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(conflict_error());
    }
    let auth_actions = {
        let mut statement = transaction
            .prepare(
                r#"
                SELECT actions.action_id, actions.revision
                FROM governed_actions actions
                JOIN capability_auth_requests requests
                  ON requests.governed_action_id = actions.action_id
                 AND requests.governed_action_revision = actions.revision
                WHERE actions.state = 'executing' AND actions.authentication_pending = 1
                  AND requests.mcp_server_id = ?1
                  AND requests.state IN ('awaiting_user', 'authorizing', 'resuming')
                "#,
            )
            .map_err(repo_sql_error)?;
        statement
            .query_map([mcp_server_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(repo_sql_error)?
            .collect::<Result<Vec<(String, u64)>, _>>()
            .map_err(repo_sql_error)?
    };
    transaction
        .execute(
            r#"
            UPDATE governed_actions
            SET state = 'failed', authentication_pending = 0,
                failure_code = 'connection_deleted',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE state = 'executing' AND authentication_pending = 1
              AND EXISTS (
                SELECT 1 FROM capability_auth_requests requests
                WHERE requests.governed_action_id = governed_actions.action_id
                  AND requests.governed_action_revision = governed_actions.revision
                  AND requests.mcp_server_id = ?1
                  AND requests.state IN ('awaiting_user', 'authorizing', 'resuming')
              )
            "#,
            [mcp_server_id],
        )
        .map_err(repo_sql_error)?;
    for (action_id, revision) in auth_actions {
        insert_event(
            &transaction,
            &action_id,
            revision,
            "failed",
            "system:mcp_delete",
            &serde_json::json!({"failure_code": "connection_deleted"}),
        )
        .map_err(map_store_error)?;
    }
    transaction
        .execute(
            r#"
            UPDATE capability_auth_requests
            SET state = 'superseded',
                output_json = '{"success":false,"payload":{"code":"connection_deleted"}}',
                failure_code = 'connection_deleted',
                supersession_reason = 'connection_deleted',
                completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_server_id = ?1
              AND state IN ('awaiting_user', 'authorizing', 'resuming')
            "#,
            [mcp_server_id],
        )
        .map_err(repo_sql_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(Some(McpDeleteTicket {
        mcp_server_id: mcp_server_id.to_string(),
        deletion_generation,
    }))
}

fn finish_delete_on_connection(
    connection: &mut Connection,
    ticket: McpDeleteTicket,
) -> McpRepositoryResult<bool> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let Some(server) = rows::server_record_on_connection(&transaction, &ticket.mcp_server_id)?
    else {
        return Ok(false);
    };
    if server.authority_generation != ticket.deletion_generation {
        return Ok(false);
    }
    transaction
        .execute(
            r#"
            DELETE FROM mcp_tool_policies
            WHERE mcp_tool_id IN (
              SELECT mcp_tool_id FROM mcp_tools WHERE mcp_server_id = ?1
            )
            "#,
            params![ticket.mcp_server_id],
        )
        .map_err(repo_sql_error)?;
    transaction
        .execute(
            "DELETE FROM mcp_tools WHERE mcp_server_id = ?1",
            params![ticket.mcp_server_id],
        )
        .map_err(repo_sql_error)?;
    let affected = transaction
        .execute(
            r#"
            DELETE FROM mcp_servers
            WHERE mcp_server_id = ?1 AND COALESCE(metadata_fingerprint, '') = ?2
            "#,
            params![ticket.mcp_server_id, ticket.deletion_generation],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(conflict_error());
    }
    transaction
        .execute(
            "DELETE FROM mcp_definitions WHERE mcp_definition_id = ?1 AND NOT EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_definition_id = ?1)",
            params![server.mcp_definition_id],
        )
        .map_err(repo_sql_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(true)
}

fn require_generation(
    connection: &Connection,
    mcp_server_id: &str,
    expected_authority_generation: &str,
) -> McpRepositoryResult<McpServerRecord> {
    let server = rows::server_record_on_connection(connection, mcp_server_id)?
        .ok_or_else(not_found_error)?;
    if server.authority_generation != expected_authority_generation {
        return Err(conflict_error());
    }
    Ok(server)
}

struct PreparedDiscoveredTool {
    name: String,
    description: Option<String>,
    input_schema_json: String,
    output_schema_json: Option<String>,
    annotations_json: String,
    metadata_fingerprint: String,
}

fn prepare_discovered_tools(
    tools: Vec<McpDiscoveredTool>,
) -> McpRepositoryResult<Vec<PreparedDiscoveredTool>> {
    let mut names = BTreeSet::new();
    let mut prepared = Vec::with_capacity(tools.len());
    for tool in tools {
        if tool.name.trim().is_empty()
            || tool.metadata_fingerprint.is_empty()
            || !names.insert(tool.name.clone())
        {
            return Err(conflict_error());
        }
        prepared.push(PreparedDiscoveredTool {
            name: tool.name,
            description: tool.description,
            input_schema_json: encode_json(&tool.input_schema)?,
            output_schema_json: tool.output_schema.as_ref().map(encode_json).transpose()?,
            annotations_json: encode_json(&tool.annotations)?,
            metadata_fingerprint: tool.metadata_fingerprint,
        });
    }
    Ok(prepared)
}

fn insert_new_tool(
    connection: &Connection,
    mcp_server_id: &str,
    tool: PreparedDiscoveredTool,
) -> McpRepositoryResult<()> {
    let mcp_tool_id = allocate_unique_id(
        connection,
        "SELECT EXISTS(SELECT 1 FROM mcp_tools WHERE mcp_tool_id = ?1)",
        "mcp_tool",
    )?;
    let annotations =
        serde_json::from_str(&tool.annotations_json).map_err(|_| invariant_error())?;
    connection
        .execute(
            r#"
            INSERT INTO mcp_tools (
              mcp_tool_id, mcp_server_id, name, description,
              input_schema_json, output_schema_json, annotations_json,
              metadata_fingerprint, discovered_at, updated_at
            )
            VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
              strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
              strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            )
            "#,
            params![
                mcp_tool_id,
                mcp_server_id,
                tool.name,
                tool.description,
                tool.input_schema_json,
                tool.output_schema_json,
                tool.annotations_json,
                tool.metadata_fingerprint,
            ],
        )
        .map_err(repo_sql_error)?;
    policies::seed_tool_policy(connection, &mcp_tool_id, &annotations)?;
    Ok(())
}

fn update_discovered_tool(
    connection: &Connection,
    mcp_tool_id: &str,
    tool: PreparedDiscoveredTool,
) -> McpRepositoryResult<()> {
    let affected = connection
        .execute(
            r#"
            UPDATE mcp_tools SET
              name = ?2,
              description = ?3,
              input_schema_json = ?4,
              output_schema_json = ?5,
              annotations_json = ?6,
              metadata_fingerprint = ?7,
              discovered_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_tool_id = ?1
            "#,
            params![
                mcp_tool_id,
                tool.name,
                tool.description,
                tool.input_schema_json,
                tool.output_schema_json,
                tool.annotations_json,
                tool.metadata_fingerprint,
            ],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(invariant_error());
    }
    Ok(())
}

pub(super) fn recompute_server_enabled(
    connection: &Connection,
    mcp_server_id: &str,
) -> McpRepositoryResult<()> {
    let enabled = connection
        .query_row(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM mcp_tools t
              JOIN mcp_tool_policies p ON p.mcp_tool_id = t.mcp_tool_id
              JOIN mcp_servers s ON s.mcp_server_id = t.mcp_server_id
              WHERE t.mcp_server_id = ?1
                AND s.data_sharing_policy IS NOT NULL
                AND s.unsafe_action_policy IS NOT NULL
                AND p.status IN ('ready', 'defaulted')
                AND p.metadata_fingerprint = t.metadata_fingerprint
            )
            "#,
            params![mcp_server_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(repo_sql_error)?
        != 0;
    let affected = connection
        .execute(
            r#"
            UPDATE mcp_servers SET
              enabled = ?2,
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_server_id = ?1
            "#,
            params![mcp_server_id, enabled],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(invariant_error());
    }
    Ok(())
}

fn allocate_unique_id(
    connection: &Connection,
    exists_sql: &str,
    prefix: &str,
) -> McpRepositoryResult<String> {
    for _ in 0..RANDOM_ID_ATTEMPTS {
        let candidate = random_id(prefix)?;
        let exists = connection
            .query_row(exists_sql, params![candidate], |row| row.get::<_, i64>(0))
            .map_err(repo_sql_error)?
            != 0;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(unavailable_error())
}

fn random_id(prefix: &str) -> McpRepositoryResult<String> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| unavailable_error())?;
    let mut id = String::with_capacity(prefix.len() + 1 + bytes.len() * 2);
    id.push_str(prefix);
    id.push(':');
    for byte in bytes {
        use std::fmt::Write as _;
        write!(id, "{byte:02x}").map_err(|_| invariant_error())?;
    }
    Ok(id)
}

fn fresh_authority_generation(previous: &str) -> McpRepositoryResult<String> {
    for _ in 0..RANDOM_ID_ATTEMPTS {
        let generation = random_id("mcp_generation")?;
        if generation != previous {
            return Ok(generation);
        }
    }
    Err(unavailable_error())
}

fn encode_json(value: &serde_json::Value) -> McpRepositoryResult<String> {
    serde_json::to_string(value).map_err(|_| conflict_error())
}

fn split_safe_config(
    mut safe_config: serde_json::Value,
) -> McpRepositoryResult<(serde_json::Value, serde_json::Value)> {
    let object = safe_config.as_object_mut().ok_or_else(conflict_error)?;
    let mut connection = serde_json::Map::new();
    for key in ["secret_refs", "secret_identity_revision"] {
        if let Some(value) = object.remove(key) {
            connection.insert(key.to_string(), value);
        }
    }
    Ok((safe_config, serde_json::Value::Object(connection)))
}

pub(super) fn repo_sql_error(error: rusqlite::Error) -> McpRepositoryError {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::ConstraintViolation =>
        {
            conflict_error()
        }
        rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::IntegralValueOutOfRange(..)
        | rusqlite::Error::InvalidColumnType(..)
        | rusqlite::Error::InvalidColumnIndex(..)
        | rusqlite::Error::InvalidColumnName(..) => invariant_error(),
        _ => unavailable_error(),
    }
}

fn map_store_error(_error: StoreError) -> McpRepositoryError {
    unavailable_error()
}

pub(super) fn not_found_error() -> McpRepositoryError {
    McpRepositoryError::new(
        McpRepositoryErrorKind::NotFound,
        "MCP repository record was not found",
    )
}

pub(super) fn conflict_error() -> McpRepositoryError {
    McpRepositoryError::new(
        McpRepositoryErrorKind::Conflict,
        "MCP repository state changed or rejected the request",
    )
}

pub(super) fn invariant_error() -> McpRepositoryError {
    McpRepositoryError::new(
        McpRepositoryErrorKind::Invariant,
        "MCP repository state is invalid",
    )
}

fn unavailable_error() -> McpRepositoryError {
    McpRepositoryError::new(
        McpRepositoryErrorKind::Unavailable,
        "MCP repository is unavailable",
    )
}
