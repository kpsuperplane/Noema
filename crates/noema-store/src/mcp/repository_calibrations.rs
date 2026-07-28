use noema_capabilities_mcp::{
    McpProviderPolicyUpdate, McpRepositoryResult, McpResetToolPolicyUpdate, McpServerRecord,
    McpSetToolEnabledUpdate, McpToolHint, McpToolHintSource, McpToolPolicyOverrideUpdate,
    McpToolPolicyRecord, McpToolPolicyStatus, validate_provider_policy,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;

use super::{
    conflict_error, invariant_error, not_found_error, recompute_server_enabled, repo_sql_error,
    rows::{server_record_on_connection, tool_policy_on_connection},
};

pub(super) fn save_provider_policy_on_connection(
    connection: &mut Connection,
    update: McpProviderPolicyUpdate,
) -> McpRepositoryResult<McpServerRecord> {
    validate_provider_policy(update.data_sharing_policy, update.unsafe_action_policy)
        .map_err(|_| conflict_error())?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let affected = transaction
        .execute(
            "UPDATE mcp_servers SET data_sharing_policy = ?2, unsafe_action_policy = ?3, policy_revision = policy_revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE mcp_server_id = ?1 AND policy_revision = ?4 AND COALESCE(metadata_fingerprint, '') = ?5",
            params![
                update.mcp_server_id,
                update.data_sharing_policy.as_str(),
                update.unsafe_action_policy.as_str(),
                update.expected_policy_revision,
                update.expected_connection_revision,
            ],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(conflict_error());
    }
    recompute_server_enabled(&transaction, &update.mcp_server_id)?;
    let server = server_record_on_connection(&transaction, &update.mcp_server_id)?
        .ok_or_else(invariant_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(server)
}

pub(super) fn save_tool_override_on_connection(
    connection: &mut Connection,
    update: McpToolPolicyOverrideUpdate,
) -> McpRepositoryResult<McpToolPolicyRecord> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let (server_id, fingerprint, authority_generation) =
        tool_identity(&transaction, &update.policy.tool_id)?;
    if fingerprint != update.policy.source_revision
        || authority_generation != update.expected_connection_revision
    {
        return Err(conflict_error());
    }
    let current = tool_policy_on_connection(&transaction, &update.policy.tool_id)?
        .ok_or_else(invariant_error)?;
    if current.policy_revision != update.expected_policy_revision {
        return Err(conflict_error());
    }
    write_complete_policy(
        &transaction,
        &update.policy.tool_id,
        current.policy_revision + 1,
        &fingerprint,
        [
            update.policy.read_only,
            update.policy.idempotent,
            update.policy.destructive,
            update.policy.open_world,
        ],
        McpToolHintSource::Human,
        McpToolPolicyStatus::Ready,
    )?;
    recompute_server_enabled(&transaction, &server_id)?;
    let saved = tool_policy_on_connection(&transaction, &update.policy.tool_id)?
        .ok_or_else(invariant_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(saved)
}

pub(super) fn reset_tool_policy_on_connection(
    connection: &mut Connection,
    update: McpResetToolPolicyUpdate,
) -> McpRepositoryResult<McpToolPolicyRecord> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let annotations = transaction
        .query_row(
            "SELECT t.annotations_json, t.metadata_fingerprint, COALESCE(s.metadata_fingerprint, '') FROM mcp_tools t JOIN mcp_servers s ON s.mcp_server_id = t.mcp_server_id WHERE t.mcp_tool_id = ?1",
            params![update.mcp_tool_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(repo_sql_error)?
        .ok_or_else(not_found_error)?;
    if annotations.1 != update.source_revision
        || annotations.2 != update.expected_connection_revision
    {
        return Err(conflict_error());
    }
    let current = tool_policy_on_connection(&transaction, &update.mcp_tool_id)?
        .ok_or_else(invariant_error)?;
    if current.policy_revision != update.expected_policy_revision {
        return Err(conflict_error());
    }
    let annotations = serde_json::from_str(&annotations.0).map_err(|_| invariant_error())?;
    seed_tool_policy(&transaction, &update.mcp_tool_id, &annotations)?;
    let policy = tool_policy_on_connection(&transaction, &update.mcp_tool_id)?
        .ok_or_else(invariant_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(policy)
}

pub(super) fn set_tool_enabled_on_connection(
    connection: &mut Connection,
    update: McpSetToolEnabledUpdate,
) -> McpRepositoryResult<McpToolPolicyRecord> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(repo_sql_error)?;
    let (server_id, fingerprint, authority_generation) =
        tool_identity(&transaction, &update.mcp_tool_id)?;
    let current = tool_policy_on_connection(&transaction, &update.mcp_tool_id)?
        .ok_or_else(invariant_error)?;
    if fingerprint != update.source_revision
        || authority_generation != update.expected_connection_revision
        || current.policy_revision != update.expected_policy_revision
    {
        return Err(conflict_error());
    }
    let affected = if update.enabled {
        transaction
            .execute(
                r#"
                UPDATE mcp_tool_policies SET
                  read_only_source = CASE WHEN read_only IS NULL THEN 'safe_default' ELSE read_only_source END,
                  read_only = COALESCE(read_only, 0),
                  idempotent_source = CASE WHEN idempotent IS NULL THEN 'safe_default' ELSE idempotent_source END,
                  idempotent = COALESCE(idempotent, 0),
                  destructive_source = CASE WHEN destructive IS NULL THEN 'safe_default' ELSE destructive_source END,
                  destructive = COALESCE(destructive, 1),
                  open_world_source = CASE WHEN open_world IS NULL THEN 'safe_default' ELSE open_world_source END,
                  open_world = COALESCE(open_world, 1),
                  status = CASE
                    WHEN read_only IS NULL OR idempotent IS NULL OR destructive IS NULL OR open_world IS NULL
                      OR read_only_source = 'safe_default' OR idempotent_source = 'safe_default'
                      OR destructive_source = 'safe_default' OR open_world_source = 'safe_default'
                    THEN 'defaulted'
                    ELSE 'ready'
                  END,
                  policy_revision = policy_revision + 1,
                  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE mcp_tool_id = ?1 AND policy_revision = ?2
                "#,
                params![update.mcp_tool_id, update.expected_policy_revision],
            )
            .map_err(repo_sql_error)?
    } else {
        transaction
            .execute(
                "UPDATE mcp_tool_policies SET status = 'disabled', policy_revision = policy_revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE mcp_tool_id = ?1 AND policy_revision = ?2",
                params![update.mcp_tool_id, update.expected_policy_revision],
            )
            .map_err(repo_sql_error)?
    };
    if affected != 1 {
        return Err(conflict_error());
    }
    recompute_server_enabled(&transaction, &server_id)?;
    let policy = tool_policy_on_connection(&transaction, &update.mcp_tool_id)?
        .ok_or_else(invariant_error)?;
    transaction.commit().map_err(repo_sql_error)?;
    Ok(policy)
}

pub(super) fn complete_tool_policy_on_connection(
    connection: &mut Connection,
    policy: McpToolPolicyRecord,
) -> McpRepositoryResult<Option<McpToolPolicyRecord>> {
    if !matches!(
        policy.status,
        McpToolPolicyStatus::Ready | McpToolPolicyStatus::Defaulted
    ) || !policy.is_callable()
    {
        return Err(conflict_error());
    }
    let source = |hint: &McpToolHint| hint.source.map(|value| value.as_str());
    let affected = connection
        .execute(
            r#"
            UPDATE mcp_tool_policies SET
              read_only = ?4, read_only_source = ?5,
              idempotent = ?6, idempotent_source = ?7,
              destructive = ?8, destructive_source = ?9,
              open_world = ?10, open_world_source = ?11,
              status = ?12, policy_revision = policy_revision + 1,
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE mcp_tool_id = ?1 AND policy_revision = ?2
              AND metadata_fingerprint = ?3 AND status = 'pending'
            "#,
            params![
                policy.tool_id,
                policy.policy_revision,
                policy.source_revision,
                policy.read_only.value,
                source(&policy.read_only),
                policy.idempotent.value,
                source(&policy.idempotent),
                policy.destructive.value,
                source(&policy.destructive),
                policy.open_world.value,
                source(&policy.open_world),
                policy.status.as_str(),
            ],
        )
        .map_err(repo_sql_error)?;
    if affected == 0 {
        return Ok(None);
    }
    let server_id = connection
        .query_row(
            "SELECT mcp_server_id FROM mcp_tools WHERE mcp_tool_id = ?1",
            params![policy.tool_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(repo_sql_error)?;
    recompute_server_enabled(connection, &server_id)?;
    tool_policy_on_connection(connection, &policy.tool_id)
}

pub(super) fn seed_tool_policy(
    connection: &Connection,
    mcp_tool_id: &str,
    annotations: &Value,
) -> McpRepositoryResult<()> {
    let (_, fingerprint, _) = tool_identity(connection, mcp_tool_id)?;
    let hint = |name: &str| {
        annotations
            .get(name)
            .and_then(Value::as_bool)
            .map(|value| (value, McpToolHintSource::Annotation.as_str()))
    };
    let values = [
        hint("readOnlyHint"),
        hint("idempotentHint"),
        hint("destructiveHint"),
        hint("openWorldHint"),
    ];
    let status = if values.iter().all(Option::is_some) {
        McpToolPolicyStatus::Ready
    } else {
        McpToolPolicyStatus::Pending
    };
    connection
        .execute(
            r#"
            INSERT INTO mcp_tool_policies (
              mcp_tool_id, read_only, read_only_source, idempotent, idempotent_source,
              destructive, destructive_source, open_world, open_world_source,
              status, policy_revision, metadata_fingerprint
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)
            ON CONFLICT(mcp_tool_id) DO UPDATE SET
              read_only = excluded.read_only, read_only_source = excluded.read_only_source,
              idempotent = excluded.idempotent, idempotent_source = excluded.idempotent_source,
              destructive = excluded.destructive, destructive_source = excluded.destructive_source,
              open_world = excluded.open_world, open_world_source = excluded.open_world_source,
              status = excluded.status, policy_revision = mcp_tool_policies.policy_revision + 1,
              metadata_fingerprint = excluded.metadata_fingerprint,
              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            "#,
            params![
                mcp_tool_id,
                values[0].map(|value| value.0),
                values[0].map(|value| value.1),
                values[1].map(|value| value.0),
                values[1].map(|value| value.1),
                values[2].map(|value| value.0),
                values[2].map(|value| value.1),
                values[3].map(|value| value.0),
                values[3].map(|value| value.1),
                status.as_str(),
                fingerprint,
            ],
        )
        .map_err(repo_sql_error)?;
    Ok(())
}

fn tool_identity(
    connection: &Connection,
    mcp_tool_id: &str,
) -> McpRepositoryResult<(String, String, String)> {
    connection
        .query_row(
            "SELECT t.mcp_server_id, t.metadata_fingerprint, COALESCE(s.metadata_fingerprint, '') FROM mcp_tools t JOIN mcp_servers s ON s.mcp_server_id = t.mcp_server_id WHERE t.mcp_tool_id = ?1",
            params![mcp_tool_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(repo_sql_error)?
        .ok_or_else(not_found_error)
}

fn write_complete_policy(
    connection: &Connection,
    mcp_tool_id: &str,
    revision: u64,
    fingerprint: &str,
    values: [bool; 4],
    source: McpToolHintSource,
    status: McpToolPolicyStatus,
) -> McpRepositoryResult<()> {
    let affected = connection
        .execute(
            "UPDATE mcp_tool_policies SET read_only = ?2, read_only_source = ?6, idempotent = ?3, idempotent_source = ?6, destructive = ?4, destructive_source = ?6, open_world = ?5, open_world_source = ?6, status = ?7, policy_revision = ?8, metadata_fingerprint = ?9, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE mcp_tool_id = ?1",
            params![mcp_tool_id, values[0], values[1], values[2], values[3], source.as_str(), status.as_str(), revision, fingerprint],
        )
        .map_err(repo_sql_error)?;
    if affected != 1 {
        return Err(invariant_error());
    }
    Ok(())
}
