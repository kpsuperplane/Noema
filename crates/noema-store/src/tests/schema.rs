use std::fs;

use rusqlite::{Connection, params};
use tempfile::TempDir;

use super::{schema_support::*, support::store_config};
use crate::{
    NoemaStore, SchemaIncompatibility, StoreConfig, StoreError,
    runtime::{inspect_empty_schema_for_test, run_migration_for_test},
    schema::{LEGACY_V9_SCHEMA_SQL, STORE_SCHEMA_VERSION, store_migrations},
};

#[tokio::test]
async fn v61_upgrade_recovers_complete_capability_authentication_identity() {
    let home = TempDir::new().expect("store root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut connection = Connection::open(&config.path).expect("v61 database");
    store_migrations()
        .to_version(&mut connection, 61)
        .expect("construct v61 schema");
    insert_migration_conversation(&connection, "conversation:auth-v61", "turn:auth-v61");
    connection
        .execute_batch(&format!(
            r#"
            INSERT INTO mcp_definitions (
              mcp_definition_id, display_name, transport_kind, safe_config_json,
              definition_revision
            ) VALUES (
              'mcp_definition:auth-v61', 'MCP', 'streamable_http', '{{}}',
              'mcp_definition_revision:auth-v61'
            );
            INSERT INTO mcp_servers (
              mcp_server_id, mcp_definition_id, connection_config_json,
              auth_status, health_status, enabled
            ) VALUES (
              'mcp:auth-v61', 'mcp_definition:auth-v61', '{{}}', 'needs_auth', 'healthy', 1
            );
            INSERT INTO adapter_oauth_profiles (
              profile_digest, profile_id, display_name, grant_audience,
              descriptor_relative_path
            ) VALUES (
              '{profile}', 'profile:test', 'Provider', 'audience:test',
              'adapters/oauth-profiles/{profile}/profile.json'
            );
            INSERT INTO adapter_oauth_applications (
              application_id, profile_digest, callback_mode, client_id, status,
              revision, credential_generation, descriptor_relative_path
            ) VALUES (
              '{application}', '{profile}', 'hosted', 'client', 'active', 1,
              '{generation}', 'adapters/oauth-applications/{application}/application.json'
            );
            INSERT INTO adapter_oauth_grants (
              grant_id, application_id, audience, desired_scopes_json,
              granted_scopes_json, authority_revision, token_revision, status,
              descriptor_relative_path
            ) VALUES
              ('{grant}', '{application}', 'audience:test', '[]', '[]', 1, 1,
               'authentication_required', 'adapters/oauth-grants/{grant}/grant.json'),
              ('{terminal_grant}', '{application}', 'audience:terminal', '[]', '[]', 1, 1,
               'authentication_required', 'adapters/oauth-grants/{terminal_grant}/grant.json');
            "#,
            profile = "1".repeat(64),
            application = "2".repeat(32),
            generation = "3".repeat(32),
            grant = "b".repeat(32),
            terminal_grant = "f".repeat(32),
        ))
        .expect("OAuth authorities");
    connection
        .execute_batch(&format!(r#"
          WITH fixtures(request_id, mcp_id, adapter_id, context, state, output_index) AS (
            VALUES
              ('cap_auth:mcp', 'mcp:auth-v61', NULL, '{{}}', 'awaiting_user', 0),
              ('cap_auth:connection', NULL, '{a}', '{{"destination":{{"connection_id":"{a}"}}}}', 'awaiting_user', 1),
              ('cap_auth:grant', NULL, '{b}', '{{"destination":{{"connection_id":"{c}"}}}}', 'authorizing', 2),
              ('cap_auth:historical-grant', NULL, '{d}', '{{"destination":{{"connection_id":"{e}"}}}}', 'completed', 3),
              ('cap_auth:terminal-fallback', NULL, '{f}', '{{}}', 'cancelled', 4)
          )
          INSERT INTO capability_auth_requests (
            request_id, owner_human_id, conversation_id, turn_id, requesting_agent_id,
            mcp_server_id, adapter_connection_id, challenge_kind, authority_revision,
            capability_name, operation_token, input_schema_json, protected_arguments_ref,
            arguments_sha256, provider_selection_digest, output_index, result_context_json, state
          ) SELECT request_id, 'human:local', 'conversation:auth-v61', 'turn:auth-v61',
            'agent:primary', mcp_id, adapter_id, 'reauthenticate', 'revision:1',
            'fixture.call', 'operation', '{{}}', '{protected}', '{digest}', '{digest}',
            output_index, context, state FROM fixtures;
          "#,
          a = "a".repeat(32), b = "b".repeat(32), c = "c".repeat(32),
          d = "d".repeat(32), e = "e".repeat(32), f = "f".repeat(32),
          protected = "4".repeat(32), digest = "5".repeat(64),
        ))
        .expect("authentication requests");
    drop(connection);

    drop(NoemaStore::open(&config).await.expect("upgrade store"));
    let connection = Connection::open(&config.path).expect("upgraded database");
    assert_eq!(
        connection
            .query_row(
                r#"SELECT COUNT(*) FROM capability_auth_requests WHERE
                  (request_id = 'cap_auth:mcp' AND authority_kind = 'mcp_server' AND authority_id = 'mcp:auth-v61' AND destination_id = authority_id) OR
                  (request_id = 'cap_auth:connection' AND authority_kind = 'adapter_connection' AND authority_id = destination_id) OR
                  (request_id = 'cap_auth:grant' AND authority_kind = 'adapter_grant' AND authority_id <> destination_id) OR
                  (request_id = 'cap_auth:historical-grant' AND authority_kind = 'adapter_grant' AND authority_id <> destination_id) OR
                  (request_id = 'cap_auth:terminal-fallback' AND authority_kind = 'adapter_connection' AND authority_id = destination_id)"#,
                [],
                |row| row.get::<_, usize>(0),
            )
            .expect("recovered identities"),
        5
    );

    let fresh_home = TempDir::new().expect("fresh root");
    let fresh_config = store_config(fresh_home.path());
    drop(NoemaStore::open(&fresh_config).await.expect("fresh store"));
    assert_eq!(
        database_snapshot(&config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );

    let invalid_home = TempDir::new().expect("invalid root");
    let invalid_config = store_config(invalid_home.path());
    fs::create_dir_all(invalid_config.path.parent().expect("database parent"))
        .expect("invalid database parent");
    let mut invalid = Connection::open(&invalid_config.path).expect("invalid database");
    store_migrations()
        .to_version(&mut invalid, 61)
        .expect("construct invalid v61 schema");
    invalid
        .execute("INSERT INTO adapter_oauth_profiles (profile_digest, profile_id, display_name, grant_audience, descriptor_relative_path) VALUES (?1, 'profile:invalid', 'Provider', 'audience:invalid', ?2)", params!["6".repeat(64), format!("adapters/oauth-profiles/{}/profile.json", "6".repeat(64))])
        .expect("invalid profile");
    invalid
        .execute("INSERT INTO adapter_oauth_applications (application_id, profile_digest, callback_mode, client_id, status, revision, credential_generation, descriptor_relative_path) VALUES (?1, ?2, 'hosted', 'client', 'active', 1, ?3, ?4)", params!["7".repeat(32), "6".repeat(64), "8".repeat(32), format!("adapters/oauth-applications/{}/application.json", "7".repeat(32))])
        .expect("invalid application");
    invalid
        .execute("INSERT INTO adapter_oauth_grants (grant_id, application_id, audience, desired_scopes_json, granted_scopes_json, authority_revision, token_revision, status, descriptor_relative_path) VALUES (?1, ?2, 'audience:invalid', '[]', '[]', 1, 1, 'authentication_required', ?3)", params!["9".repeat(32), "7".repeat(32), format!("adapters/oauth-grants/{}/grant.json", "9".repeat(32))])
        .expect("invalid grant");
    insert_migration_conversation(&invalid, "conversation:invalid-auth", "turn:invalid-auth");
    invalid
        .execute("INSERT INTO capability_auth_requests (request_id, owner_human_id, conversation_id, turn_id, requesting_agent_id, adapter_connection_id, challenge_kind, authority_revision, capability_name, operation_token, input_schema_json, protected_arguments_ref, arguments_sha256, provider_selection_digest, output_index, result_context_json, state) VALUES ('cap_auth:invalid-grant', 'human:local', 'conversation:invalid-auth', 'turn:invalid-auth', 'agent:primary', ?1, 'reauthenticate', 'revision:1', 'fixture.call', 'operation', '{}', ?2, ?3, ?3, 0, '{}', 'awaiting_user')", params!["9".repeat(32), "a".repeat(32), "b".repeat(64)])
        .expect("invalid active request");
    invalid
        .execute("INSERT INTO capability_auth_requests (request_id, owner_human_id, conversation_id, turn_id, requesting_agent_id, adapter_connection_id, challenge_kind, authority_revision, capability_name, operation_token, input_schema_json, protected_arguments_ref, arguments_sha256, provider_selection_digest, output_index, result_context_json, state) VALUES ('cap_auth:ambiguous-connection', 'human:local', 'conversation:invalid-auth', 'turn:invalid-auth', 'agent:primary', ?1, 'reauthenticate', 'revision:1', 'fixture.call', 'operation', '{}', ?2, ?3, ?3, 1, '{}', 'awaiting_user')", params!["c".repeat(32), "d".repeat(32), "e".repeat(64)])
        .expect("ambiguous active request");
    store_migrations()
        .to_version(&mut invalid, 62)
        .expect("apply version 62 boundary");
    assert!(store_migrations().to_version(&mut invalid, 63).is_err());
    invalid
        .execute(
            "DELETE FROM capability_auth_requests WHERE request_id = 'cap_auth:invalid-grant'",
            [],
        )
        .expect("remove version 63 defect fixture");
    store_migrations()
        .to_version(&mut invalid, 63)
        .expect("apply version 63 repair");
    assert!(store_migrations().to_latest(&mut invalid).is_err());
}

#[tokio::test]
async fn v58_upgrade_adds_optional_provider_conversation_text() {
    let home = TempDir::new().expect("store root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut connection = Connection::open(&config.path).expect("v58 database");
    store_migrations()
        .to_version(&mut connection, 58)
        .expect("construct v58 schema");
    connection
        .execute_batch(
            r#"
            INSERT INTO conversations (
              conversation_id, owner_object_type, owner_object_id, provider
            ) VALUES ('conversation:provider-text', 'human', 'human:local', 'codex');
            INSERT INTO conversation_items (
              item_id, conversation_id, sequence_index, kind, status,
              author_actor_id, content_text
            ) VALUES (
              'item:existing-text', 'conversation:provider-text', 1,
              'assistant_text', 'completed', 'agent:primary', 'Existing text'
            );
            "#,
        )
        .expect("v58 data");
    drop(connection);

    drop(NoemaStore::open(&config).await.expect("upgrade store"));
    let connection = Connection::open(&config.path).expect("upgraded database");
    assert_eq!(
        connection
            .query_row(
                "SELECT provider_content_text FROM conversation_items WHERE item_id = 'item:existing-text'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("provider text"),
        None
    );
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))
            .expect("schema version"),
        STORE_SCHEMA_VERSION
    );
}

#[test]
fn v50_task_file_conversion_preserves_existing_task_document_and_retries() {
    let home = TempDir::new().expect("temp store root");
    let database = home.path().join("db/noema.sqlite3");
    fs::create_dir_all(database.parent().expect("database parent")).expect("database directory");
    let mut connection = Connection::open(&database).expect("open database");
    store_migrations()
        .to_version(&mut connection, 49)
        .expect("migrate to v49");
    connection
        .execute(
            "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, description_markdown, executor_agent_id, task_directory, source_kind, created_by_actor_id) VALUES ('task:file-conversion', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Converted Task', 'Original request.', 'agent:system:task-executor', 'converted-task', 'system', 'actor:system')",
            [],
        )
        .expect("insert Task");
    connection
        .execute(
            "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, description_markdown, executor_agent_id, cwd_override, task_directory, source_kind, created_by_actor_id) VALUES ('task:explicit-conversion', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Explicit Task', 'Explicit request.', 'agent:system:task-executor', ?1, 'explicit-task', 'system', 'actor:system')",
            [home.path().to_string_lossy().as_ref()],
        )
        .expect("insert explicit Task");
    let task_root = home.path().join("tasks/converted-task");
    fs::create_dir_all(&task_root).expect("Task directory");
    fs::write(task_root.join("TASK.md"), "Current mutable content.\n").expect("current Task file");

    {
        let transaction = connection.transaction().expect("conversion transaction");
        crate::task_file_migration::convert_legacy_task_files(&transaction)
            .expect("first conversion");
        crate::task_file_migration::convert_legacy_task_files(&transaction)
            .expect("retry conversion");
        transaction.commit().expect("commit conversion");
    }
    store_migrations()
        .to_latest(&mut connection)
        .expect("apply v50");

    assert_eq!(
        fs::read_to_string(task_root.join("TASK.md")).expect("Task file"),
        "Current mutable content.\n"
    );
    let legacy = fs::read_to_string(task_root.join("legacy-task.md")).expect("legacy Task file");
    assert!(legacy.contains("# Converted Task"));
    assert!(legacy.contains("Original request."));
    let explicit =
        fs::read_to_string(home.path().join("explicit-task/TASK.md")).expect("explicit Task file");
    assert!(explicit.contains("# Explicit Task"));
    assert!(explicit.contains("Explicit request."));
}

#[test]
fn v56_result_migration_copies_only_submitted_tasks_and_preserves_results() {
    let home = TempDir::new().expect("temp store root");
    let database = home.path().join("db/noema.sqlite3");
    fs::create_dir_all(database.parent().expect("database parent")).expect("database directory");
    let mut connection = Connection::open(&database).expect("open database");
    store_migrations()
        .to_version(&mut connection, 55)
        .expect("migrate to v55");
    for (id, directory, stage) in [
        ("task:submitted", "submitted", "stage:personal:done"),
        ("task:pending", "pending", "stage:personal:queue"),
        ("task:preserved", "preserved", "stage:personal:done"),
    ] {
        connection
            .execute(
                "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, description_markdown, executor_agent_id, task_directory, source_kind, created_by_actor_id) VALUES (?1, 'workspace:personal', 'workflow:personal:default', ?2, ?1, '', 'agent:system:task-executor', ?3, 'system', 'actor:system')",
                params![id, stage, directory],
            )
            .expect("insert Task");
        let root = home.path().join("tasks").join(directory);
        fs::create_dir_all(&root).expect("Task directory");
        fs::write(root.join("TASK.md"), format!("Current {id}\n")).expect("Task file");
    }
    fs::write(
        home.path().join("tasks/preserved/RESULT.md"),
        "Existing result\n",
    )
    .expect("existing result");

    store_migrations()
        .to_version(&mut connection, 56)
        .expect("migrate to v56");

    assert_eq!(
        fs::read_to_string(home.path().join("tasks/submitted/RESULT.md")).expect("copied result"),
        "Current task:submitted\n"
    );
    assert!(!home.path().join("tasks/pending/RESULT.md").exists());
    assert_eq!(
        fs::read_to_string(home.path().join("tasks/preserved/RESULT.md"))
            .expect("preserved result"),
        "Existing result\n"
    );
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))
            .expect("schema version"),
        56
    );
}

#[tokio::test]
async fn v57_moves_recurrence_prose_preserves_documents_on_retry_and_converges() {
    let upgrade_home = TempDir::new().expect("upgrade root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database directory");
    let mut connection = Connection::open(&upgrade_config.path).expect("open database");
    store_migrations()
        .to_version(&mut connection, 56)
        .expect("migrate to v56");
    let authorization = serde_json::json!({
        "kind": "manual_task_body",
        "title": "Recurring title",
        "description_markdown": "Stored authorization"
    })
    .to_string();
    connection
        .execute(
            "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, description_markdown, authorization_context_json, executor_agent_id, task_directory, source_kind, created_by_actor_id) VALUES ('task:v57', 'workspace:personal', 'workflow:personal:default', 'stage:personal:inbox', 'Existing Task', 'Stale database prose', ?1, 'agent:system:task-executor', 'v57-task', 'work_ui', 'actor:human:local')",
            [&authorization],
        )
        .expect("insert Task");
    connection
        .execute(
            "INSERT INTO task_recurrences (recurrence_id, workspace_id, title, description_markdown, authorization_context_json, starts_at, cron_expression, time_zone, missed_run_policy, overlap_policy, lifecycle, next_run_at) VALUES ('recurrence:v57', 'workspace:personal', 'Recurring title', 'Stored template', ?1, 1, '0 8 * * *', 'UTC', 'run_once', 'skip', 'active', 1)",
            [&authorization],
        )
        .expect("insert recurrence");
    let task_path = upgrade_home.path().join("tasks/v57-task/TASK.md");
    fs::create_dir_all(task_path.parent().expect("Task directory")).expect("Task directory");
    fs::write(&task_path, "Exact existing Task\n").expect("Task document");

    let transaction = connection.transaction().expect("migration transaction");
    crate::task_file_migration::move_task_prose_to_files(&transaction)
        .expect("interrupted migration");
    transaction.rollback().expect("interrupt migration");
    let recurrence_path = upgrade_home.path().join("recurrences/v57/TASK.md");
    fs::write(&recurrence_path, "Interrupted template edit\n").expect("template edit");
    drop(connection);

    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("retry migration"),
    );
    assert_eq!(
        fs::read_to_string(task_path).expect("Task document"),
        "Exact existing Task\n"
    );
    assert_eq!(
        fs::read_to_string(recurrence_path).expect("recurrence document"),
        "Interrupted template edit\n"
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    for table in ["tasks", "task_recurrences"] {
        assert_eq!(
            connection
                .query_row(
                    &format!(
                        "SELECT count(*) FROM pragma_table_info('{table}') WHERE name = 'description_markdown'"
                    ),
                    [],
                    |row| row.get::<_, usize>(0),
                )
                .expect("description column count"),
            0
        );
    }
    let renamed: String = connection
        .query_row(
            "SELECT authorization_context_json FROM task_recurrences WHERE recurrence_id = 'recurrence:v57'",
            [],
            |row| row.get(0),
        )
        .expect("authorization context");
    assert!(renamed.contains("task_document_markdown"));
    assert!(!renamed.contains("description_markdown"));
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh root");
    let fresh_config = store_config(fresh_home.path());
    drop(NoemaStore::open(&fresh_config).await.expect("fresh schema"));
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn fresh_migrations_are_exact_idempotent_and_enforce_foreign_keys() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    let store = NoemaStore::open(&config)
        .await
        .expect("migrate fresh store");

    store
        .with_connection(|conn| {
            assert_eq!(conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))?, 1);
            assert_eq!(conn.query_row("PRAGMA busy_timeout", [], |row| row.get::<_, i64>(0))?, 5_000);
            assert_eq!(count_where(conn, "humans", "human_id = 'human:local'")?, 1);
            assert_eq!(count_where(conn, "workspaces", "workspace_id = 'workspace:personal' AND name = 'Personal' AND description = '' AND is_personal = 1 AND archived_at IS NULL AND revision = 1")?, 1);
            assert_eq!(count_where(conn, "workspace_memberships", "workspace_id = 'workspace:personal' AND human_id = 'human:local' AND role = 'owner'")?, 1);
            assert_eq!(count_where(conn, "projects", "1 = 1")?, 0);

            let policy = conn.query_row(
                "SELECT max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, max_automatic_retries, max_review_rounds FROM task_execution_policy WHERE policy_id = 'default'",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?)),
            )?;
            assert_eq!(policy, (80, 400, 120, 20, 3, 3));
            assert_eq!(conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?, STORE_SCHEMA_VERSION);
            assert!(!schema_object_exists(conn, "table", "schema_state")?);
            assert!(schema_object_exists(conn, "table", "clients")?);
            assert_eq!(count_where(conn, "auxiliary_model_preferences", "task_id = 'action_reviewer'")?, 0);
            Ok(())
        })
        .await
        .expect("inspect migrated schema");

    drop(store);
    let reopened = NoemaStore::open(&config)
        .await
        .expect("second bootstrap/open is idempotent");
    reopened
        .with_connection(|conn| {
            for (table, predicate, expected) in [
                ("workspaces", "is_personal = 1", 1),
                (
                    "workspace_memberships",
                    "workspace_id = 'workspace:personal'",
                    1,
                ),
                ("task_execution_policy", "policy_id = 'default'", 1),
            ] {
                assert_eq!(count_where(conn, table, predicate)?, expected);
            }
            Ok(())
        })
        .await
        .expect("inspect idempotent reopen");
}
#[tokio::test]
async fn v31_upgrade_and_fresh_schema_converge_on_web_browse_contract() {
    let upgrade_home = TempDir::new().expect("v31 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v31 database");
    store_migrations()
        .to_version(&mut connection, 31)
        .expect("construct v31 schema");
    connection
        .execute(
            "INSERT INTO observed_urls (normalized_url, source_kind, source_event_reference) VALUES ('https://example.com/', 'search_result', 'fixture')",
            [],
        )
        .expect("v31 observation");
    drop(connection);
    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("upgrade v31"),
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    connection
        .execute(
            "INSERT INTO provider_capability_bindings (binding_id, tool_name, capability_id, provider_account_id) VALUES ('binding:web.browse:web.browse', 'web.browse', 'web.browse', 'provider_account:obscura:system')",
            [],
        )
        .expect("browse binding accepted");
    connection
        .execute(
            "INSERT INTO observed_urls (normalized_url, source_kind, source_event_reference) VALUES ('https://example.com/browse', 'browser_link', 'fixture')",
            [],
        )
        .expect("browser observation accepted");
    assert_eq!(
        count_where(
            &connection,
            "observed_urls",
            "source_event_reference = 'fixture'"
        )
        .expect("observations"),
        2
    );

    let fresh_home = TempDir::new().expect("fresh root");
    let fresh_config = store_config(fresh_home.path());
    drop(NoemaStore::open(&fresh_config).await.expect("fresh schema"));
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn browser_provider_route_migration_preserves_v59_assignment_at_position_zero() {
    let home = TempDir::new().expect("v59 route root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut connection = Connection::open(&config.path).expect("v59 database");
    store_migrations()
        .to_version(&mut connection, 59)
        .expect("construct v59 schema");
    connection
        .execute(
            "INSERT INTO provider_capability_bindings (binding_id, tool_name, capability_id, provider_account_id) VALUES ('provider_capability_binding:web.browse:web.browse', 'web.browse', 'web.browse', 'provider_account:obscura:system')",
            [],
        )
        .expect("save v59 browse assignment");
    drop(connection);

    drop(NoemaStore::open(&config).await.expect("upgrade v59 route"));
    let connection = Connection::open(&config.path).expect("upgraded route database");
    assert_eq!(
        connection
            .query_row(
                "SELECT route_position FROM provider_capability_bindings WHERE tool_name = 'web.browse'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("read migrated route position"),
        0
    );
}

#[tokio::test]
async fn v32_upgrade_persists_system_provider_accounts_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v32 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v32 database");
    store_migrations()
        .to_version(&mut connection, 32)
        .expect("construct v32 schema");
    connection
        .execute(
            "INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:exa:existing', 'exa', 'existing', 'Existing Exa', 'secret_input', 1, 0, 'authenticated')",
            [],
        )
        .expect("existing account");
    drop(connection);

    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("upgrade v32"),
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    assert_eq!(
        count_where(
            &connection,
            "provider_accounts",
            "provider_account_id IN ('provider_account:duckduckgo_public:system', 'provider_account:direct_http:system', 'provider_account:obscura:system') AND account_key = 'system' AND auth_method = 'none' AND is_active = 1 AND is_default = 1 AND status = 'authenticated'"
        )
        .expect("system accounts"),
        3
    );
    assert_eq!(
        count_where(
            &connection,
            "provider_accounts",
            "provider_account_id = 'provider_account:exa:existing'"
        )
        .expect("existing account"),
        1
    );
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh root");
    let fresh_config = store_config(fresh_home.path());
    drop(NoemaStore::open(&fresh_config).await.expect("fresh schema"));
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn v58_upgrade_adds_kernel_provider_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v57 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v57 database");
    store_migrations()
        .to_version(&mut connection, 57)
        .expect("construct v57 schema");
    connection
        .execute(
            "INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:exa:existing', 'exa', 'existing', 'Existing Exa', 'secret_input', 1, 0, 'authenticated')",
            [],
        )
        .expect("existing account");
    drop(connection);

    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("upgrade v57"),
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    connection
        .execute(
            "INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:kernel:existing', 'kernel', 'existing', 'Existing Kernel', 'secret_input', 1, 0, 'authenticated')",
            [],
        )
        .expect("Kernel account");
    assert_eq!(
        count_where(
            &connection,
            "provider_accounts",
            "provider_account_id IN ('provider_account:exa:existing', 'provider_account:kernel:existing')"
        )
        .expect("preserved accounts"),
        2
    );
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))
            .expect("schema version"),
        STORE_SCHEMA_VERSION
    );
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh root");
    let fresh_config = store_config(fresh_home.path());
    drop(NoemaStore::open(&fresh_config).await.expect("fresh schema"));
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn v34_upgrade_links_saved_action_request_items_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v34 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v34 database");
    store_migrations()
        .to_version(&mut connection, 34)
        .expect("construct v34 schema");
    connection
        .execute_batch(
            r#"
            INSERT INTO conversations (
              conversation_id, owner_object_type, owner_object_id, provider
            ) VALUES ('conversation:action-source', 'human', 'human:local', 'codex');
            INSERT INTO conversation_turns (turn_id, conversation_id, status)
            VALUES ('turn:action-source', 'conversation:action-source', 'waiting_for_tool');
            INSERT INTO conversation_items (
              item_id, conversation_id, turn_id, sequence_index, kind, status,
              author_actor_id, payload_json, deleted_at
            ) VALUES (
              'item:action-source', 'conversation:action-source', 'turn:action-source', 1,
              'approval_request', 'completed', 'agent:primary',
              '{"metadata":{"action":{"id":"action:source","payload":{"provider_call_id":"call:ordinary"}}}}',
              strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            );
            INSERT INTO governed_actions (
              action_id, revision, owner_human_id, conversation_id, turn_id,
              requesting_agent_id, capability_name, operation_token, review_route,
              read_only, idempotent, destructive, open_world, arguments_json,
              arguments_sha256, input_schema_json, authorization_context_json,
              safe_summary, state
            ) VALUES (
              'action:source', 1, 'human:local', 'conversation:action-source',
              'turn:action-source', 'agent:primary', 'fixture.write', 'token:source',
              'human_review', 0, 0, 0, 1, '{}',
              'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
              '{"type":"object"}', '{}', 'Fixture action', 'awaiting_approval'
            );
            INSERT INTO governed_actions (
              action_id, revision, owner_human_id, conversation_id, turn_id,
              requesting_agent_id, capability_name, operation_token, review_route,
              read_only, idempotent, destructive, open_world, arguments_json,
              arguments_sha256, input_schema_json, authorization_context_json,
              safe_summary, state
            ) SELECT
              'action:unlinked', revision, owner_human_id, conversation_id, turn_id,
              requesting_agent_id, capability_name, operation_token, review_route,
              read_only, idempotent, destructive, open_world, arguments_json,
              arguments_sha256, input_schema_json, authorization_context_json,
              safe_summary, 'succeeded'
            FROM governed_actions WHERE action_id = 'action:source';
            "#,
        )
        .expect("v34 action source");
    drop(connection);

    let store = NoemaStore::open(&upgrade_config)
        .await
        .expect("upgrade v34");
    let source = store
        .get_action_request_source("action:source", 1)
        .await
        .expect("load hidden source")
        .expect("saved source");
    assert_eq!(source.item_id, "item:action-source");
    assert_eq!(
        source
            .payload_json
            .pointer("/metadata/action/payload/provider_call_id"),
        Some(&serde_json::json!("call:ordinary"))
    );
    assert!(
        store
            .list_interrupted_governed_action_resumptions()
            .await
            .expect("recoverable actions")
            .is_empty()
    );
    drop(store);

    let fresh_home = TempDir::new().expect("fresh root");
    let fresh_config = store_config(fresh_home.path());
    drop(NoemaStore::open(&fresh_config).await.expect("fresh schema"));
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn clients_migration_upgrades_an_existing_v25_database() {
    let home = TempDir::new().expect("client migration root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    let mut conn = Connection::open(&config.path).expect("open version 25 database");
    store_migrations()
        .to_version(&mut conn, 25)
        .expect("migrate through version 25");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("upgrade client schema");
    store
        .with_connection(|conn| {
            assert!(schema_object_exists(conn, "table", "clients")?);
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .expect("inspect client schema");
}

#[tokio::test]
async fn notification_migration_upgrades_v33_projection_state_and_registrations() {
    let home = TempDir::new().expect("Web Push migration root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    let mut conn = Connection::open(&config.path).expect("open version 33 database");
    store_migrations()
        .to_version(&mut conn, 33)
        .expect("migrate through version 33");
    conn.execute(
        "INSERT INTO web_push_identity (identity_id, private_key, public_key, primary_sequence, attention_seeded) VALUES (1, ?1, ?2, 7, 1)",
        rusqlite::params![vec![1_u8; 32], vec![2_u8; 65]],
    )
    .expect("seed legacy projection state");
    conn.execute(
        "INSERT INTO web_push_attention_seen (attention_key) VALUES ('attention:preserved')",
        [],
    )
    .expect("seed legacy attention");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("upgrade Web Push schema");
    store
        .with_connection(|conn| {
            for table in [
                "web_push_identity",
                "web_push_subscriptions",
                "web_push_deliveries",
                "notification_projection_state",
                "notification_attention_seen",
                "client_notification_registrations",
                "apns_deliveries",
            ] {
                assert!(schema_object_exists(conn, "table", table)?);
            }
            let identity_columns = conn
                .prepare("PRAGMA table_info(web_push_identity)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            assert!(!identity_columns.iter().any(|column| {
                matches!(
                    column.as_str(),
                    "primary_conversation_id" | "primary_sequence" | "attention_seeded"
                )
            }));
            assert_eq!(
                conn.query_row(
                    "SELECT primary_sequence, attention_seeded FROM notification_projection_state WHERE state_id = 1",
                    [],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, bool>(1)?)),
                )?,
                (7, true)
            );
            assert_eq!(
                conn.query_row(
                    "SELECT count(*) FROM notification_attention_seen WHERE attention_key = 'attention:preserved'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                1
            );
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .expect("inspect Web Push schema");
}

#[tokio::test]
async fn live_activity_migration_upgrades_v35_and_converges_with_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v35 migration root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v35 database");
    store_migrations()
        .to_version(&mut connection, 35)
        .expect("construct v35 schema");
    connection
        .execute(
            "INSERT INTO work_events (event_id, event_kind, workspace_id, actor_id, correlation_id) VALUES ('event:v36-seed', 'task.waiting', 'workspace:personal', 'actor:system', 'correlation:v36')",
            [],
        )
        .expect("seed work event");
    connection
        .execute(
            "INSERT INTO work_notification_outbox (notification_id, event_sequence, destination_kind, destination_id, notification_kind, payload_json) VALUES ('notification:v36-seed', 1, 'human_primary_conversation', 'human:local', 'task_waiting', '{\"task_id\":\"task:v36\"}')",
            [],
        )
        .expect("seed task notification");
    drop(connection);

    let upgraded = NoemaStore::open(&upgrade_config)
        .await
        .expect("upgrade v35 database");
    upgraded
        .with_connection(|connection| {
            assert_eq!(
                connection.query_row(
                    "SELECT task_notification_sequence FROM notification_projection_state WHERE state_id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                1
            );
            assert!(schema_object_exists(
                connection,
                "table",
                "client_live_activity_registrations"
            )?);
            let route_columns = connection
                .prepare("PRAGMA table_info(apns_deliveries)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            assert!(route_columns.iter().any(|column| column == "route"));
            assert!(route_columns.iter().any(|column| column == "task_id"));
            assert_eq!(
                connection.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .expect("inspect upgraded v36 schema");
    drop(upgraded);
    let fresh_home = TempDir::new().expect("fresh v36 root");
    let fresh_config = store_config(fresh_home.path());
    let fresh = NoemaStore::open(&fresh_config)
        .await
        .expect("create fresh v36 schema");
    drop(fresh);
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn live_activity_diagnostics_upgrade_v40_and_converge_with_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v40 migration root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v40 database");
    store_migrations()
        .to_version(&mut connection, 40)
        .expect("construct v40 schema");
    drop(connection);

    let upgraded = NoemaStore::open(&upgrade_config)
        .await
        .expect("upgrade v40 database");
    upgraded
        .with_connection(|connection| {
            let delivery_columns = connection
                .prepare("PRAGMA table_info(live_activity_deliveries)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            assert!(delivery_columns.iter().any(|column| column == "apns_id"));
            assert!(schema_object_exists(
                connection,
                "table",
                "live_activity_observations"
            )?);
            assert_eq!(
                connection.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .expect("inspect upgraded v42 schema");
    drop(upgraded);

    let fresh_home = TempDir::new().expect("fresh v42 root");
    let fresh_config = store_config(fresh_home.path());
    drop(
        NoemaStore::open(&fresh_config)
            .await
            .expect("create fresh v42 schema"),
    );
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn live_activity_observation_rename_preserves_v41_rows() {
    let home = TempDir::new().expect("v41 rename root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut connection = Connection::open(&config.path).expect("v41 database");
    store_migrations()
        .to_version(&mut connection, 41)
        .expect("construct v41 schema");
    connection
        .execute(
            "INSERT INTO clients (client_id, owner_human_id, display_name, token_hash) VALUES ('client:rename', 'human:local', 'Phone', zeroblob(32))",
            [],
        )
        .expect("insert client");
    connection
        .execute(
            "INSERT INTO client_live_activity_observations (observation_id, client_id, event) VALUES ('live_activity_observation:rename', 'client:rename', 'snapshot')",
            [],
        )
        .expect("insert observation");
    drop(connection);

    let store = NoemaStore::open(&config)
        .await
        .expect("rename observation table");
    store
        .with_connection(|connection| {
            assert!(!schema_object_exists(
                connection,
                "table",
                "client_live_activity_observations"
            )?);
            assert_eq!(
                count_where(
                    connection,
                    "live_activity_observations",
                    "client_id = 'client:rename'"
                )?,
                1
            );
            Ok(())
        })
        .await
        .expect("inspect renamed observation");
}

#[tokio::test]
async fn task_schedules_upgrade_v27_without_losing_tasks_and_match_fresh_schema() {
    let upgrade_home = TempDir::new().expect("schedule upgrade root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().unwrap()).unwrap();
    let mut conn = Connection::open(&upgrade_config.path).unwrap();
    store_migrations().to_version(&mut conn, 27).unwrap();
    conn.execute(
        "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:schedule-upgrade', 'workspace:personal', 'workflow:personal:default', 'stage:personal:inbox', 'Preserved', 'system', 'actor:system')",
        [],
    ).unwrap();
    drop(conn);
    let upgraded = NoemaStore::open(&upgrade_config).await.unwrap();

    let fresh_home = TempDir::new().expect("fresh schedule root");
    let fresh = NoemaStore::open(&store_config(fresh_home.path()))
        .await
        .unwrap();
    let snapshot = |connection: &mut Connection| -> Result<(String, String, String), StoreError> {
        let objects = connection.query_row(
            "SELECT group_concat(name || ':' || sql, '|') FROM sqlite_master WHERE name IN ('task_recurrences', 'task_recurrence_occurrences', 'tasks_next_scheduled', 'task_recurrences_next_due', 'task_recurrence_occurrences_history') ORDER BY name",
            [], |row| row.get(0),
        )?;
        let columns = connection.query_row(
            "SELECT group_concat(name, ',') FROM pragma_table_info('tasks') WHERE name LIKE 'schedule%' OR name LIKE 'recurrence%' ORDER BY cid",
            [], |row| row.get(0),
        )?;
        let occurrence_columns = connection.query_row(
            "SELECT group_concat(name, ',') FROM pragma_table_info('task_recurrence_occurrences') ORDER BY cid",
            [], |row| row.get(0),
        )?;
        Ok((objects, columns, occurrence_columns))
    };
    let upgraded_schema = upgraded.with_connection(snapshot).await.unwrap();
    let fresh_schema = fresh.with_connection(snapshot).await.unwrap();
    assert_eq!(upgraded_schema, fresh_schema);
    upgraded.with_connection(|connection| {
        assert_eq!(connection.query_row(
            "SELECT title FROM tasks WHERE task_id = 'task:schedule-upgrade' AND scheduled_for IS NULL AND recurrence_id IS NULL",
            [], |row| row.get::<_, String>(0),
        )?, "Preserved");
        assert!(connection.execute(
            "INSERT INTO task_recurrences (recurrence_id, workspace_id, title, authorization_context_json, starts_at, cron_expression, time_zone, missed_run_policy, overlap_policy, lifecycle) VALUES ('recurrence:invalid', 'workspace:personal', 'Invalid', '{}', 0, '* * * * *', 'UTC', 'run_once', 'skip', 'active')",
            [],
        ).is_err());
        Ok(())
    }).await.unwrap();
}

#[tokio::test]
async fn recurrence_history_index_repairs_an_already_applied_v30() {
    let home = TempDir::new().expect("v30 repair root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().unwrap()).unwrap();
    let mut connection = Connection::open(&config.path).unwrap();
    store_migrations().to_version(&mut connection, 30).unwrap();
    connection
        .execute_batch(
            "DROP INDEX task_recurrence_occurrences_history;
             CREATE INDEX task_recurrence_occurrences_history
             ON task_recurrence_occurrences(recurrence_id, scheduled_for DESC, occurrence_id DESC);",
        )
        .unwrap();
    drop(connection);

    let store = NoemaStore::open(&config).await.unwrap();
    store
        .with_connection(|connection| {
            let index_sql: String = connection.query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'task_recurrence_occurrences_history'",
                [],
                |row| row.get(0),
            )?;
            assert!(index_sql.contains("created_at DESC"));
            assert_eq!(
                connection.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn acp_executors_upgrade_v28_preserves_provider_history_and_converges() {
    let upgrade_home = TempDir::new().expect("ACP upgrade root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().unwrap()).unwrap();
    let mut connection = Connection::open(&upgrade_config.path).unwrap();
    store_migrations().to_version(&mut connection, 28).unwrap();
    connection.execute(
        "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Preserved provider task', 'system', 'actor:system')",
        [],
    ).unwrap();
    insert_legacy_delegated_contract(&connection).unwrap();
    connection
        .execute(
            "UPDATE tasks SET current_contract_id = 'contract:one' WHERE task_id = 'task:valid'",
            [],
        )
        .unwrap();
    connection.execute(
        r#"INSERT INTO agent_runs (
          run_id, instance_name, task_id, task_generation, contract_id, run_kind, agent_id,
          provider_kind, provider_account_id, provider_instance_key, selection_mode, model_profile,
          max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval,
          max_automatic_retries, max_review_rounds, status
        ) VALUES (
          'run:provider-history', 'Provider history', 'task:valid', 1, 'contract:one', 'executor', 'agent:task-executor',
          'openrouter', 'provider_account:openrouter:default', 'provider_account:openrouter:default', 'explicit_profile', 'openrouter/auto',
          80, 400, 120, 20, 3, 3, 'completed'
        )"#,
        [],
    ).unwrap();
    drop(connection);

    let upgraded = NoemaStore::open(&upgrade_config).await.unwrap();
    let fresh_home = TempDir::new().expect("fresh ACP root");
    let fresh = NoemaStore::open(&store_config(fresh_home.path()))
        .await
        .unwrap();
    let snapshot = |connection: &mut Connection| -> Result<(String, String), StoreError> {
        let objects = connection.query_row(
            "SELECT group_concat(name || ':' || sql, '|') FROM sqlite_master WHERE name IN ('acp_agents', 'acp_auth_attempts', 'acp_permission_consumptions', 'tasks_executor_agent', 'acp_auth_attempts_agent') ORDER BY name",
            [],
            |row| row.get(0),
        )?;
        let columns = connection.query_row(
            "SELECT group_concat(name, ',') FROM pragma_table_info('agent_runs') WHERE name LIKE 'acp_%' OR name LIKE 'execution_backend%' OR name = 'effective_cwd' ORDER BY cid",
            [],
            |row| row.get(0),
        )?;
        Ok((objects, columns))
    };
    assert_eq!(
        upgraded.with_connection(snapshot).await.unwrap(),
        fresh.with_connection(snapshot).await.unwrap()
    );
    upgraded.with_connection(|connection| {
        assert_eq!(connection.query_row(
            "SELECT executor_agent_id FROM tasks WHERE task_id = 'task:valid'",
            [],
            |row| row.get::<_, String>(0),
        )?, "agent:task-executor");
        assert!(!schema_object_exists(
            connection,
            "table",
            "task_execution_contracts"
        )?);
        assert_eq!(connection.query_row(
            "SELECT provider_kind || ':' || execution_backend_kind FROM agent_runs WHERE run_id = 'run:provider-history'",
            [],
            |row| row.get::<_, String>(0),
        )?, "openrouter:provider");
        Ok(())
    }).await.unwrap();
}

#[tokio::test]
async fn task_gate_choices_upgrade_existing_schema_and_converge_with_fresh_schema() {
    let home = TempDir::new().expect("task gate choices root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    let mut conn = Connection::open(&config.path).expect("open version 24 database");
    store_migrations()
        .to_version(&mut conn, 24)
        .expect("migrate through version 24");
    conn.execute(
        "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:choice-upgrade', 'workspace:personal', 'workflow:personal:default', 'stage:personal:waiting', 'Choose', 'system', 'actor:system')",
        [],
    )
    .expect("insert existing task");
    conn.execute(
        "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, prompt_markdown, opened_by_actor_id) VALUES ('gate:choice-upgrade', 'task:choice-upgrade', 1, 'clarification', 'open', 'Choose one', 'actor:system')",
        [],
    )
    .expect("insert existing gate");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("upgrade task gate choices");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT suggested_answers_json FROM task_gates WHERE gate_id = 'gate:choice-upgrade'",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "[]"
            );
            assert!(
                table_columns(conn, "task_gates")?
                    .iter()
                    .any(|column| column == "suggested_answers_json")
            );
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .expect("inspect upgraded choices");
}

#[tokio::test]
async fn interaction_transcript_repairs_upgrade_resolved_rows() {
    let home = TempDir::new().expect("interaction repair root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    let mut conn = Connection::open(&config.path).expect("open version 21 database");
    store_migrations()
        .to_version(&mut conn, 21)
        .expect("migrate through version 21");
    insert_migration_conversation(&conn, "conversation:repair", "turn:repair");
    for (item_id, sequence_index, kind, status, actor, payload) in [
        (
            "item:repair-call",
            1,
            "tool_call",
            "running",
            "agent:primary",
            r#"{"status":"running"}"#,
        ),
        (
            "item:repair-prompt",
            2,
            "multiple_choice_prompt",
            "completed",
            "agent:primary",
            "{}",
        ),
        (
            "item:repair-selection",
            3,
            "multiple_choice_selection",
            "completed",
            "human:local",
            "{}",
        ),
        (
            "item:repair-result",
            4,
            "tool_result",
            "completed",
            "agent:primary",
            "{}",
        ),
    ] {
        conn.execute(
            "INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_actor_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![item_id, "conversation:repair", "turn:repair", sequence_index, kind, status, actor, payload],
        )
        .expect("insert repair item");
    }
    conn.execute(
        "INSERT INTO conversation_interactions (interaction_id, conversation_id, originating_turn_id, kind, provider_call_id, canonical_tool_name, provider_tool_name, provider_kind, provider_account_id, provider_instance_key, selection_mode, credential_revision, model, tool_catalog_digest, request_json, projection_json, provider_call_item_id, projection_item_id, revision, lifecycle_status, resolution_item_id, tool_result_item_id, resolution_json, client_message_id, resolved_at) VALUES ('interaction:repair', ?1, ?2, 'multiple_choice', 'call:repair', 'noema.present_multiple_choice', 'present_multiple_choice', 'codex', 'provider_account:codex:default', 'provider-instance:codex:default', 'explicit_profile', 1, 'gpt-test', ?3, '{}', '{}', 'item:repair-call', 'item:repair-prompt', 2, 'answered', 'item:repair-selection', 'item:repair-result', '{}', 'client:repair', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params!["conversation:repair", "turn:repair", "a".repeat(64)],
    )
    .expect("insert repair interaction");
    drop(conn);

    let repaired = NoemaStore::open(&config).await.expect("repair stale call");
    repaired
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT call.status, json_extract(result.payload_json, '$.id'), json_extract(result.payload_json, '$.activity_kind'), json_extract(result.payload_json, '$.title') FROM conversation_items AS call JOIN conversation_items AS result ON result.item_id = 'item:repair-result' WHERE call.item_id = 'item:repair-call'",
                    [],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
                )?,
                (
                    "completed".to_string(),
                    "tool_result:interaction:repair".to_string(),
                    "tool_result".to_string(),
                    "Tool result: noema.present_multiple_choice".to_string(),
                )
            );
            Ok(())
        })
        .await
        .expect("verify repaired call");
}

#[tokio::test]
async fn hosted_search_activity_migration_repairs_only_provider_hosted_rows() {
    let home = TempDir::new().expect("hosted search repair root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    let mut conn = Connection::open(&config.path).expect("open version 23 database");
    store_migrations()
        .to_version(&mut conn, 23)
        .expect("migrate through version 23");
    insert_migration_conversation(&conn, "conversation:hosted", "turn:hosted");
    for (item_id, sequence_index, kind, action_provider) in [
        ("item:hosted-call", 1, "tool_call", "openrouter"),
        ("item:hosted-result", 2, "tool_result", "openrouter"),
        ("item:native-call", 3, "tool_call", "web_x2e_search"),
        ("item:native-result", 4, "tool_result", "web_x2e_search"),
    ] {
        let payload = serde_json::json!({
            "metadata": {
                "action": {
                    "provider_name": action_provider,
                    "name": "web.search"
                }
            }
        });
        conn.execute(
            "INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_actor_id, payload_json, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, 'completed', 'agent:primary', ?6, ?7)",
            params![
                item_id,
                "conversation:hosted",
                "turn:hosted",
                sequence_index,
                kind,
                payload.to_string(),
                r#"{"source":"provider_action","provider":"openrouter"}"#,
            ],
        )
        .expect("insert hosted search item");
    }
    drop(conn);

    let repaired = NoemaStore::open(&config)
        .await
        .expect("repair hosted searches");
    repaired
        .with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT item_id, kind FROM conversation_items WHERE item_id LIKE 'item:%-call' OR item_id LIKE 'item:%-result' ORDER BY sequence_index",
            )?;
            let rows = statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(
                rows,
                vec![
                    ("item:hosted-call".to_string(), "activity".to_string()),
                    ("item:hosted-result".to_string(), "activity".to_string()),
                    ("item:native-call".to_string(), "tool_call".to_string()),
                    ("item:native-result".to_string(), "tool_result".to_string()),
                ]
            );
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .expect("verify hosted search repair");
}

#[tokio::test]
async fn version_66_upgrade_creates_project_documents() {
    let home = TempDir::new().expect("version 66 project root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut connection = Connection::open(&config.path).expect("version 66 database");
    store_migrations()
        .to_version(&mut connection, 66)
        .expect("construct version 66 schema");
    connection
        .execute(
            "INSERT INTO projects (project_id, workspace_id, name, description) VALUES ('project:upgrade', 'workspace:personal', 'Upgrade context', 'Preserved context')",
            [],
        )
        .expect("project fixture");
    drop(connection);

    drop(NoemaStore::open(&config).await.expect("upgrade schema"));

    assert_eq!(
        fs::read_to_string(
            home.path()
                .join("workspaces/personal/projects/upgrade/docs/PROJECT.md")
        )
        .expect("migrated PROJECT.md"),
        "# Upgrade context\n\nPreserved context\n"
    );
}

#[tokio::test]
async fn versions_65_and_66_preserve_supported_rows_and_converge() {
    let upgrade_home = TempDir::new().expect("version 64 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("version 64 database");
    store_migrations()
        .to_version(&mut connection, 64)
        .expect("construct version 64 schema");
    insert_migration_conversation(&connection, "conversation:v65", "turn:v65");
    connection
        .execute_batch(&format!(
            r#"
            INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:v65', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Preserved Task', 'system', 'actor:system');
            INSERT INTO work_events (event_id, event_kind, workspace_id, task_id, actor_id, correlation_id, payload_json) VALUES ('event:v65', 'task.queued', 'workspace:personal', 'task:v65', 'actor:system', 'correlation:v65', '{{"v":1,"contract_id":null}}');
            INSERT INTO clients (client_id, owner_human_id, display_name, token_hash, auth_kind) VALUES ('client:v65', 'human:local', 'Native client', zeroblob(32), 'native_oauth');
            INSERT INTO governed_actions (
              action_id, revision, owner_human_id, requesting_agent_id, capability_name,
              operation_token, review_route, arguments_json, arguments_sha256,
              input_schema_json, authorization_context_json, safe_summary, state)
            VALUES ('action:v65', 1, 'human:local', 'agent:primary', 'fixture.write', 'write',
              'human_review', '{{}}', '{digest}', '{{}}', '{{}}', 'Preserved action', 'awaiting_approval');
            INSERT INTO governed_action_approvals (action_id, action_revision, state) VALUES ('action:v65', 1, 'pending');
            "#,
            digest = "a".repeat(64),
        ))
        .expect("supported version 64 rows");

    connection
        .execute_batch(
            r#"
            INSERT INTO workflow_definitions (workflow_id, workspace_id, name) VALUES ('workflow:custom', 'workspace:personal', 'Custom');
            INSERT INTO workflow_stages (stage_id, workflow_id, stable_key, display_name, ordinal, system_behavior, board_visible) VALUES ('stage:custom:inbox', 'workflow:custom', 'inbox', 'Inbox', 1, 'intake', 1);
            INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:custom', 'workspace:personal', 'workflow:custom', 'stage:custom:inbox', 'Custom Task', 'system', 'actor:system');
            "#,
        )
        .expect("custom workflow fixture");
    assert!(store_migrations().to_latest(&mut connection).is_err());
    connection
        .execute_batch(
            r#"
            DELETE FROM tasks WHERE task_id = 'task:custom';
            DELETE FROM workflow_stages WHERE workflow_id = 'workflow:custom';
            DELETE FROM workflow_definitions WHERE workflow_id = 'workflow:custom';
            INSERT INTO work_events (event_id, event_kind, workspace_id, actor_id, correlation_id)
            VALUES ('event:legacy-contract', 'contract.created', 'workspace:personal', 'actor:system', 'correlation:legacy');
            "#,
        )
        .expect("legacy event fixture");
    assert!(store_migrations().to_latest(&mut connection).is_err());
    connection
        .execute_batch(
            r#"
            DELETE FROM work_events WHERE event_id = 'event:legacy-contract';
            UPDATE governed_action_approvals
            SET state = 'revoked', decided_by_human_id = 'human:local',
                decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE action_id = 'action:v65';
            "#,
        )
        .expect("revoked approval fixture");
    assert!(store_migrations().to_latest(&mut connection).is_err());
    connection
        .execute(
            "UPDATE governed_action_approvals SET state = 'pending', decided_by_human_id = NULL, decided_at = NULL WHERE action_id = 'action:v65'",
            [],
        )
        .expect("restore supported approval");
    store_migrations()
        .to_latest(&mut connection)
        .expect("upgrade supported version 64 rows");

    for (table, predicate) in [
        ("tasks", "task_id = 'task:v65'"),
        ("clients", "client_id = 'client:v65'"),
        (
            "conversations",
            "conversation_id = 'conversation:v65' AND owner_human_id = 'human:local'",
        ),
        (
            "governed_action_approvals",
            "action_id = 'action:v65' AND state = 'pending'",
        ),
    ] {
        assert_eq!(count_where(&connection, table, predicate).unwrap(), 1);
    }
    assert_eq!(
        connection
            .query_row(
                "SELECT payload_json FROM work_events WHERE event_id = 'event:v65'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        r#"{"v":1}"#
    );
    for (table, removed) in [
        ("clients", &["token_hash", "auth_kind"][..]),
        ("conversations", &["owner_object_type", "owner_object_id"]),
        (
            "tasks",
            &[
                "workflow_id",
                "current_contract_id",
                "latest_submission_id",
                "latest_review_id",
            ],
        ),
    ] {
        let columns = table_columns(&connection, table).unwrap();
        for column in removed {
            assert!(!columns.iter().any(|found| found == column));
        }
    }
    for index in [
        "conversation_items_conversation_sequence",
        "artifact_versions_artifact",
        "agent_run_items_run_sequence",
    ] {
        assert!(!schema_object_exists(&connection, "index", index).unwrap());
    }
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh version 65 root");
    let fresh_config = store_config(fresh_home.path());
    drop(
        NoemaStore::open(&fresh_config)
            .await
            .expect("fresh version 65 schema"),
    );
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );

    let web_home = TempDir::new().expect("version 65 web root");
    let web_config = store_config(web_home.path());
    fs::create_dir_all(web_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut web_connection = Connection::open(&web_config.path).expect("version 65 database");
    store_migrations()
        .to_version(&mut web_connection, 65)
        .expect("construct version 65 schema");
    web_connection
        .execute_batch(
            r#"
            INSERT INTO provider_accounts (
              provider_account_id, provider_kind, account_key, display_name, auth_method,
              is_active, is_default, status
            ) VALUES (
              'provider_account:exa:preserved', 'exa', 'preserved', 'Exa preserved',
              'secret_input', 1, 0, 'authenticated'
            );
            INSERT INTO provider_capability_bindings (
              binding_id, tool_name, capability_id, provider_account_id, route_position
            ) VALUES (
              'provider_binding:preserved', 'web.search', 'web.search',
              'provider_account:exa:preserved', 0
            );
            "#,
        )
        .expect("version 65 provider rows");
    store_migrations()
        .to_latest(&mut web_connection)
        .expect("upgrade version 65 provider rows");
    assert_eq!(
        count_where(
            &web_connection,
            "provider_accounts",
            "provider_account_id IN ('provider_account:exa:preserved', 'provider_account:firecrawl:public')"
        )
        .unwrap(),
        2
    );
    assert_eq!(
        count_where(
            &web_connection,
            "provider_capability_bindings",
            "binding_id = 'provider_binding:preserved'"
        )
        .unwrap(),
        1
    );
    drop(web_connection);
    assert_eq!(
        database_snapshot(&web_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn current_schema_enforces_projection_history_and_ledger_invariants() {
    let home = TempDir::new().expect("temp store root");
    let store = NoemaStore::open(&store_config(home.path()))
        .await
        .expect("migrate fresh store");

    store
        .with_connection(|conn| {
            for table in [
                "workspaces", "workspace_memberships", "projects", "tasks", "task_gates",
                "task_messages", "agent_runs",
                "work_events", "work_notification_outbox", "work_command_receipts",
            ] {
                assert!(schema_object_exists(conn, "table", table)?, "missing table {table}");
            }
            for index in [
                "task_gates_one_open_per_task", "agent_runs_one_runnable_per_task",
                "agent_runs_fifo_claim", "work_events_workspace_cursor",
                "work_notification_outbox_claim",
            ] {
                assert!(schema_object_exists(conn, "index", index)?, "missing index {index}");
            }
            assert!(!schema_object_exists(conn, "table", "task_events")?);
            assert!(!schema_object_exists(conn, "table", "run_events")?);
            assert!(!schema_object_exists(conn, "table", "workflow_definitions")?);
            assert!(!schema_object_exists(conn, "table", "workflow_stages")?);
            for table in [
                "task_execution_contracts", "task_contract_criteria", "task_submissions",
                "task_submission_criteria", "task_submission_artifacts",
                "task_submission_citations", "task_reviews", "task_review_criteria",
            ] {
                assert!(!schema_object_exists(conn, "table", table)?, "obsolete table {table}");
            }
            assert!(!table_columns(conn, "tasks")?.iter().any(|column| column == "status"));
            let run_columns = table_columns(conn, "agent_runs")?;
            assert!(!run_columns.iter().any(|column| column == "priority"));
            assert!(!run_columns.iter().any(|column| column == "resume_message"));
            for column in ["contract_id", "triggering_submission_id", "triggering_review_id"] {
                assert!(!run_columns.iter().any(|found| found == column));
            }

            conn.execute_batch(
                r#"
                INSERT INTO workspaces (workspace_id, name) VALUES ('workspace:other', 'Other');
                INSERT INTO projects (project_id, workspace_id, name)
                VALUES ('project:other', 'workspace:other', 'Other project');
                "#,
            )?;
            assert!(conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, project_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:cross-project', 'workspace:personal', 'project:other', 'stage:personal:inbox', 'Bad project', 'system', 'actor:system')",
                [],
            ).is_err());
            assert!(conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:bad-stage', 'workspace:personal', 'stage:personal:other', 'Bad stage', 'system', 'actor:system')",
                [],
            ).is_err());

            conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'stage:personal:queue', 'Valid task', 'system', 'actor:system')",
                [],
            )?;
            conn.execute(
                "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, prompt_markdown, opened_by_actor_id) VALUES ('gate:one', 'task:valid', 1, 'clarification', 'open', 'Question?', 'actor:system')",
                [],
            )?;
            assert!(conn.execute(
                "INSERT INTO task_gates (gate_id, task_id, task_generation, gate_kind, gate_state, prompt_markdown, opened_by_actor_id) VALUES ('gate:two', 'task:valid', 1, 'approval', 'open', 'Approve?', 'actor:system')",
                [],
            ).is_err());

            insert_planner_run(conn, "run:one")?;
            assert!(insert_planner_run(conn, "run:two").is_err());
            conn.execute("UPDATE agent_runs SET status = 'completed' WHERE run_id = 'run:one'", [])?;
            insert_planner_run(conn, "run:two")?;

            for event_id in ["event:one", "event:two"] {
                conn.execute(
                    "INSERT INTO work_events (event_id, event_kind, workspace_id, actor_id, correlation_id) VALUES (?1, 'project.created', 'workspace:personal', 'actor:system', 'correlation:test')",
                    [event_id],
                )?;
            }
            let removed_sequence = conn.query_row(
                "SELECT event_sequence FROM work_events WHERE event_id = 'event:two'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            conn.execute("DELETE FROM work_events WHERE event_id = 'event:two'", [])?;
            conn.execute(
                "INSERT INTO work_events (event_id, event_kind, workspace_id, actor_id, correlation_id) VALUES ('event:three', 'project.created', 'workspace:personal', 'actor:system', 'correlation:test')",
                [],
            )?;
            let next_sequence = conn.query_row(
                "SELECT event_sequence FROM work_events WHERE event_id = 'event:three'",
                [],
                |row| row.get::<_, i64>(0),
            )?;
            assert!(next_sequence > removed_sequence, "AUTOINCREMENT must not recycle cursors");

            conn.execute(
                "INSERT INTO mcp_definitions (mcp_definition_id, display_name, transport_kind, safe_config_json, definition_revision) VALUES ('mcp_definition:auth-integrity', 'Auth integrity', 'streamable_http', '{}', 'mcp_definition_revision:auth-integrity')",
                [],
            )?;
            conn.execute(
                "INSERT INTO mcp_servers (mcp_server_id, mcp_definition_id, connection_config_json, auth_status, health_status, enabled, metadata_fingerprint) VALUES ('mcp:auth-integrity', 'mcp_definition:auth-integrity', '{}', 'needs_auth', 'healthy', 1, 'generation:1')",
                [],
            )?;
            let insert_auth_request = |authority: &str| {
                conn.execute(
                    "INSERT INTO capability_auth_requests (request_id, owner_human_id, task_id, run_id, task_generation, requesting_agent_id, authority_kind, authority_id, destination_id, challenge_kind, destination_revision, capability_name, operation_token, input_schema_json, protected_arguments_ref, arguments_sha256, provider_selection_digest, output_index, result_context_json, state) VALUES (?1, 'human:local', 'task:valid', 'run:two', 1, 'agent:task-executor', 'mcp_server', ?2, ?2, 'reauthenticate', 'generation:1', 'mcp.auth/tool', 'operation', '{}', ?3, ?4, ?4, 0, '{}', 'awaiting_user')",
                    rusqlite::params![format!("cap_auth:{authority}"), authority, "a".repeat(32), "b".repeat(64)],
                )
            };
            assert!(insert_auth_request("").is_err());
            insert_auth_request("mcp:auth-integrity")?;
            assert!(
                conn.execute(
                    "DELETE FROM mcp_servers WHERE mcp_server_id = 'mcp:auth-integrity'",
                    [],
                )
                .is_err()
            );
            conn.execute(
                "UPDATE capability_auth_requests SET state = 'superseded' WHERE authority_kind = 'mcp_server' AND authority_id = 'mcp:auth-integrity'",
                [],
            )?;
            conn.execute(
                "DELETE FROM mcp_servers WHERE mcp_server_id = 'mcp:auth-integrity'",
                [],
            )?;
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM capability_auth_requests WHERE authority_kind = 'mcp_server' AND authority_id = 'mcp:auth-integrity'",
                    [],
                    |row| row.get::<_, usize>(0),
                )?,
                1,
                "terminal audit rows survive authority deletion"
            );
            Ok(())
        })
        .await
        .expect("validate V8 integrity");
}

#[tokio::test]
async fn version_36_upgrade_removes_submission_citations() {
    let upgraded_home = TempDir::new().expect("citation upgrade root");
    let upgraded_config = store_config(upgraded_home.path());
    fs::create_dir_all(upgraded_config.path.parent().expect("database parent"))
        .expect("create database parent");
    let mut connection = Connection::open(&upgraded_config.path).expect("open version 36 database");
    store_migrations()
        .to_version(&mut connection, 36)
        .expect("migrate through version 36");
    assert!(
        !schema_object_exists(&connection, "table", "task_submission_citations")
            .expect("inspect version 36 schema")
    );
    drop(connection);

    let upgraded = NoemaStore::open(&upgraded_config)
        .await
        .expect("upgrade citation schema");
    upgraded
        .with_connection(|connection| {
            assert!(!schema_object_exists(
                connection,
                "table",
                "task_submission_citations"
            )?);
            Ok(())
        })
        .await
        .expect("inspect upgraded schema");
}

#[test]
fn migration_history_is_internally_valid() {
    store_migrations()
        .validate()
        .expect("valid migration history");
}

#[tokio::test]
async fn version_thirteen_adds_mcp_service_description_without_losing_connection() {
    let home = TempDir::new().expect("version thirteen root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("version thirteen database");
    store_migrations()
        .to_version(&mut conn, 13)
        .expect("apply version thirteen migrations");
    conn.execute_batch(
        r#"
        INSERT INTO mcp_definitions (
          mcp_definition_id, display_name, transport_kind, safe_config_json, definition_revision
        ) VALUES ('mcp_definition:dex', 'Dex', 'streamable_http', '{}', 'mcp_definition_revision:dex');
        INSERT INTO mcp_servers (
          mcp_server_id, mcp_definition_id, connection_config_json,
          auth_status, health_status, enabled, metadata_fingerprint
        ) VALUES ('mcp:dex', 'mcp_definition:dex', '{}', 'none', 'healthy', 1, 'generation:dex');
        "#,
    )
    .expect("version thirteen MCP connection");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("migrate version thirteen");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT service_description FROM mcp_servers WHERE mcp_server_id = 'mcp:dex'",
                    [],
                    |row| row.get::<_, Option<String>>(0),
                )?,
                None
            );
            Ok(())
        })
        .await
        .expect("preserved MCP connection");
}

#[tokio::test]
async fn adapter_label_migrations_converge_before_projection_cutover() {
    let home = TempDir::new().expect("version fourteen root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("version fourteen database");
    store_migrations()
        .to_version(&mut conn, 14)
        .expect("apply version fourteen migrations");
    conn.execute(
        r#"
        INSERT INTO adapter_connections (
          connection_id, connection_slug, semantic_digest, account_kind, status,
          connection_revision, credential_revision, grant_revision, policy_revision,
          granted_scopes_json, allowed_operations_json, descriptor_relative_path
        ) VALUES (?1, 'personal', ?2, 'personal_user', 'authentication_required',
          1, 1, 1, 1, '[]', '[]', ?3)
        "#,
        params![
            "a".repeat(32),
            "b".repeat(64),
            format!("adapters/connections/{}/connection.json", "a".repeat(32)),
        ],
    )
    .expect("version fourteen adapter connection");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("migrate version fourteen");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM adapter_connections", [], |row| {
                    row.get::<_, i64>(0)
                })?,
                0
            );
            let table_sql: String = conn.query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'adapter_connections'",
                [],
                |row| row.get(0),
            )?;
            assert!(table_sql.contains("connection_label"));
            assert!(!table_sql.contains("account_label"));
            Ok(())
        })
        .await
        .expect("converged adapter projection");
}

#[tokio::test]
async fn version_fifteen_account_label_shape_converges_before_projection_cutover() {
    let home = TempDir::new().expect("version fifteen root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("version fifteen database");
    store_migrations()
        .to_version(&mut conn, 14)
        .expect("apply version fourteen migrations");
    conn.execute_batch(
        "ALTER TABLE adapter_connections ADD COLUMN account_label TEXT\n  CHECK (account_label IS NULL OR (length(account_label) BETWEEN 1 AND 256 AND trim(account_label) = account_label));\nPRAGMA user_version = 15;",
    )
    .expect("apply drifted version fifteen migration");
    conn.execute(
        r#"
        INSERT INTO adapter_connections (
          connection_id, connection_slug, semantic_digest, account_kind, status,
          connection_revision, credential_revision, grant_revision, policy_revision,
          granted_scopes_json, allowed_operations_json, descriptor_relative_path,
          account_label
        ) VALUES (?1, 'personal', ?2, 'personal_user', 'authentication_required',
          1, 1, 1, 1, '[]', '[]', ?3, 'me@example.com')
        "#,
        params![
            "a".repeat(32),
            "b".repeat(64),
            format!("adapters/connections/{}/connection.json", "a".repeat(32)),
        ],
    )
    .expect("version fifteen adapter connection");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("repair version fifteen schema");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM adapter_connections", [], |row| {
                    row.get::<_, i64>(0)
                })?,
                0
            );
            let table_sql: String = conn.query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'adapter_connections'",
                [],
                |row| row.get(0),
            )?;
            assert!(table_sql.contains("connection_label TEXT"));
            assert!(!table_sql.contains("account_label"));
            Ok(())
        })
        .await
        .expect("converged adapter projection");
}

#[tokio::test]
async fn legacy_v9_is_adopted_without_losing_rows() {
    let home = TempDir::new().expect("legacy root");
    let config = store_config(home.path());
    create_legacy_v9_database(&config.path);
    Connection::open(&config.path)
        .expect("legacy database")
        .execute(
            "INSERT INTO humans (human_id, display_name) VALUES ('human:preserved', 'Preserved')",
            [],
        )
        .expect("legacy row");

    let store = NoemaStore::open(&config)
        .await
        .expect("adopt legacy baseline");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            assert!(!schema_object_exists(conn, "table", "schema_state")?);
            assert_eq!(
                conn.query_row(
                    "SELECT display_name FROM humans WHERE human_id = 'human:preserved'",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "Preserved"
            );
            Ok(())
        })
        .await
        .expect("verify adopted data");
    drop(store);

    assert_eq!(
        database_snapshot(&config.path).schema_objects,
        canonical_schema_objects()
    );
}

#[tokio::test]
async fn pending_versioned_migrations_run_without_losing_rows() {
    let home = TempDir::new().expect("versioned root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("version one database");
    store_migrations()
        .to_version(&mut conn, 3)
        .expect("apply pre-MCP-policy migrations");
    conn.execute(
        "INSERT INTO humans (human_id, display_name) VALUES ('human:preserved', 'Preserved')",
        [],
    )
    .expect("version one row");
    conn.execute(
        "INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, safe_config_json, auth_status, health_status, enabled, metadata_fingerprint) VALUES ('mcp:legacy', 'Legacy', 'stdio', '{\"command\":\"docs\",\"secret_refs\":{\"env\":[\"TOKEN\"]},\"secret_identity_revision\":\"revision\"}', 'none', 'healthy', 1, 'generation')",
        [],
    )
    .expect("legacy MCP server");
    conn.execute(
        "INSERT INTO mcp_tools (mcp_tool_id, mcp_server_id, name, input_schema_json, annotations_json, metadata_fingerprint, discovered_at) VALUES ('tool:legacy', 'mcp:legacy', 'read', '{}', '{}', 'fingerprint', 'now')",
        [],
    )
    .expect("legacy MCP tool");
    conn.execute(
        "INSERT INTO tool_calibrations (calibration_id, mcp_tool_id, read_classification, write_classification, export_classification, status) VALUES ('calibration:legacy', 'tool:legacy', 'trusted', 'none', 'none', 'ready')",
        [],
    )
    .expect("legacy calibration");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("apply pending migrations");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            assert_eq!(
                count_where(conn, "humans", "human_id = 'human:preserved'")?,
                1
            );
            assert!(!schema_object_exists(conn, "table", "tool_calibrations")?);
            assert!(schema_object_exists(conn, "table", "mcp_tool_policies")?);
            assert_eq!(
                count_where(
                    conn,
                    "mcp_tool_policies",
                    "mcp_tool_id = 'tool:legacy' AND status = 'pending' AND read_only IS NULL"
                )?,
                1
            );
            assert_eq!(
                count_where(
                    conn,
                    "mcp_servers",
                    "mcp_server_id = 'mcp:legacy' AND mcp_definition_id IS NOT NULL"
                )?,
                1,
                "existing MCP ids remain connection ids"
            );
            assert_eq!(
                conn.query_row(
                    "SELECT definitions.safe_config_json FROM mcp_servers connections JOIN mcp_definitions definitions USING (mcp_definition_id) WHERE connections.mcp_server_id = 'mcp:legacy'",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "{\"command\":\"docs\"}"
            );
            assert_eq!(
                conn.query_row(
                    "SELECT json_extract(connection_config_json, '$.secret_identity_revision') FROM mcp_servers WHERE mcp_server_id = 'mcp:legacy'",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "revision"
            );
            assert_eq!(
                count_where(
                    conn,
                    "mcp_servers",
                    "mcp_server_id = 'mcp:legacy' AND enabled = 0 AND data_sharing_policy IS NULL AND unsafe_action_policy IS NULL AND policy_revision = 0"
                )?,
                1
            );
            Ok(())
        })
        .await
        .expect("verify migrated row");
}

#[tokio::test]
async fn version_eighteen_preserves_accounts_and_expands_every_provider_constraint() {
    let home = TempDir::new().expect("v17 root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("v17 database");
    store_migrations()
        .to_version(&mut conn, 17)
        .expect("apply v17 migrations");
    conn.execute(
        "INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:codex:default', 'codex', 'default', 'Codex', 'oauth_device_code', 1, 1, 'unknown')",
        [],
    )
    .expect("legacy account");
    drop(conn);

    let store = NoemaStore::open(&config).await.expect("migrate v17 to v18");
    store
        .with_connection(|conn| {
            assert_eq!(count_where(conn, "provider_accounts", "provider_account_id = 'provider_account:codex:default'")?, 1);
            for table in [
                "agent_runtime_preferences",
                "auxiliary_model_preferences",
                "provider_accounts",
                "default_model_preference",
                "conversations",
                "conversation_context_summaries",
                "task_model_pool_entries",
                "agent_runs",
            ] {
                let sql: String = conn.query_row(
                    "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )?;
                assert!(sql.contains("'openrouter'"), "{table} accepts OpenRouter");
            }
            conn.execute(
                "INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:openrouter:default', 'openrouter', 'default', 'OpenRouter', 'oauth_pkce', 1, 1, 'authenticated')",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("verify v18 provider constraints");
}

#[tokio::test]
async fn model_preference_v19_upgrade_preserves_intent_and_enforces_selection_modes() {
    let home = TempDir::new().expect("v19 preference root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("v19 database");
    store_migrations()
        .to_version(&mut conn, 19)
        .expect("apply v19 migrations");
    conn.execute_batch(
        r#"
        INSERT INTO agents (agent_id, display_name, system_role)
        VALUES ('agent:primary', 'Noema', 'primary');
        INSERT INTO agent_runtime_preferences (
          agent_id, provider_kind, provider_account_id, provider_instance_key,
          model_profile, reasoning_effort, is_override
        ) VALUES (
          'agent:primary', 'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'gpt-5.6-luna', 'low', 0
        );
        INSERT INTO auxiliary_model_preferences (
          task_id, provider_kind, provider_account_id, provider_instance_key,
          model_profile, reasoning_effort, is_override
        ) VALUES
          ('web_fetch_summarizer', 'openrouter', 'provider_account:openrouter:default',
           'provider_account:openrouter:default', 'openai/gpt-5.6-luna', 'low', 0),
          ('action_reviewer', 'openrouter', 'provider_account:openrouter:default',
           'provider_account:openrouter:default', 'custom/reviewer', NULL, 1);
        INSERT INTO default_model_preference (
          preference_id, provider_kind, provider_account_id, provider_instance_key,
          model_profile, reasoning_effort, is_override
        ) VALUES (
          'default', 'local_models', 'provider_account:local_models:default',
          'local-model:v1:test', 'local-model', NULL, 0
        );
        INSERT INTO task_model_pool_entries (
          pool_entry_id, complexity, provider_kind, provider_account_id,
          provider_instance_key, model_profile, reasoning_effort, is_override
        ) VALUES
          ('task_pool:legacy:first', 'simple', 'codex', 'provider_account:codex:default',
           'provider_account:codex:default', 'gpt-5.5', 'low', 0),
          ('task_pool:legacy:second', 'simple', 'codex', 'provider_account:codex:default',
           'provider_account:codex:default', 'gpt-5.6-luna', 'low', 0);
        "#,
    )
    .expect("populate v19 preferences");
    drop(conn);

    let store = NoemaStore::open(&config).await.expect("migrate v19 to v20");
    store
        .with_connection(|conn| {
            for table in [
                "agent_runtime_preferences",
                "auxiliary_model_preferences",
                "default_model_preference",
                "task_model_pool_entries",
            ] {
                let columns = table_columns(conn, table)?;
                assert!(columns.iter().any(|column| column == "selection_mode"));
                assert!(!columns.iter().any(|column| column == "is_override"));
            }
            assert_eq!(
                count_where(
                    conn,
                    "agent_runtime_preferences",
                    "selection_mode = 'noema_recommended' AND model_profile IS NULL AND reasoning_effort IS NULL"
                )?,
                1
            );
            assert_eq!(
                count_where(
                    conn,
                    "auxiliary_model_preferences",
                    "task_id = 'action_reviewer' AND selection_mode = 'explicit_profile' AND model_profile = 'custom/reviewer'"
                )?,
                1
            );
            assert_eq!(
                count_where(
                    conn,
                    "default_model_preference",
                    "provider_kind = 'local_models' AND selection_mode = 'explicit_profile' AND model_profile = 'local-model'"
                )?,
                1
            );
            assert_eq!(
                count_where(
                    conn,
                    "task_model_pool_entries",
                    "selection_mode = 'noema_recommended' AND model_profile IS NULL"
                )?,
                2,
                "distinct legacy defaults remain lossless when they converge on delegation"
            );

            for values in [
                "'bad:model', 'primary', 'codex', 'account', 'instance', 'noema_recommended', 'model', NULL",
                "'bad:effort', 'primary', 'codex', 'account', 'instance', 'noema_recommended', NULL, 'low'",
                "'bad:local', 'primary', 'local_models', 'account', 'instance', 'noema_recommended', NULL, NULL",
                "'bad:explicit', 'primary', 'codex', 'account', 'instance', 'explicit_profile', NULL, NULL",
            ] {
                assert!(
                    conn.execute(
                        &format!(
                            "INSERT INTO agent_runtime_preferences (agent_id, provider_kind, provider_account_id, provider_instance_key, selection_mode, model_profile, reasoning_effort) VALUES ({values})"
                        ),
                        [],
                    )
                    .is_err()
                );
            }
            Ok(())
        })
        .await
        .expect("verify v20 preference constraints");
}

#[tokio::test]
async fn version_twenty_repairs_the_delegated_task_pool_index() {
    let home = TempDir::new().expect("v20 preference root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("v20 database");
    store_migrations()
        .to_version(&mut conn, 20)
        .expect("apply v20 migrations");
    conn.execute_batch(
        r#"DROP INDEX task_model_pool_entries_unique_selection;
CREATE UNIQUE INDEX task_model_pool_entries_unique_selection ON task_model_pool_entries(
  complexity, provider_account_id, selection_mode,
  COALESCE(model_profile, ''), COALESCE(reasoning_effort, '')
);
"#,
    )
    .expect("reproduce earlier v20 index");
    drop(conn);

    let store = NoemaStore::open(&config).await.expect("repair v20 index");
    store
        .with_connection(|conn| {
            let sql: String = conn.query_row(
                "SELECT sql FROM sqlite_schema WHERE type = 'index' AND name = 'task_model_pool_entries_unique_selection'",
                [],
                |row| row.get(0),
            )?;
            assert!(sql.contains("COALESCE(model_profile, pool_entry_id)"));
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            Ok(())
        })
        .await
        .expect("verify repaired v20 index");
}

#[tokio::test]
async fn conversation_interaction_v18_upgrade_and_fresh_schema_converge() {
    let cases = [("populated_v18", Some(18_usize)), ("fresh_current", None)];
    for (case, version) in cases {
        let home = TempDir::new().expect("interaction schema root");
        let config = store_config(home.path());
        if let Some(version) = version {
            fs::create_dir_all(config.path.parent().expect("database parent"))
                .expect("database parent");
            let mut conn = Connection::open(&config.path).expect("open v18 fixture");
            store_migrations()
                .to_version(&mut conn, version)
                .expect("apply v18 migrations");
            conn.execute(
                "INSERT INTO conversations (conversation_id, owner_object_type, owner_object_id, provider, agent_status) VALUES ('conversation:interaction-schema', 'human', 'human:local', 'codex', 'idle')",
                [],
            )
            .expect("preserved conversation");
            conn.execute(
                "INSERT INTO conversation_turns (turn_id, conversation_id, status) VALUES ('turn:interaction-schema', 'conversation:interaction-schema', 'running')",
                [],
            )
            .expect("preserved turn");
            drop(conn);
        }

        let store = NoemaStore::open(&config)
            .await
            .expect("reach current schema");
        store
            .with_connection(|conn| {
                assert_eq!(
                    conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                    STORE_SCHEMA_VERSION
                );
                assert!(schema_object_exists(
                    conn,
                    "table",
                    "conversation_interactions"
                )?);
                for index in [
                    "conversation_interactions_conversation_provider_call",
                    "conversation_interactions_conversation_client_message",
                    "conversation_interactions_conversation_status",
                    "conversation_interactions_resume_claim",
                ] {
                    assert!(
                        schema_object_exists(conn, "index", index)?,
                        "missing {index}"
                    );
                }
                if case == "populated_v18" {
                    assert_eq!(
                        count_where(
                            conn,
                            "conversations",
                            "conversation_id = 'conversation:interaction-schema'"
                        )?,
                        1
                    );
                    assert_eq!(
                        count_where(
                            conn,
                            "conversation_turns",
                            "turn_id = 'turn:interaction-schema'"
                        )?,
                        1
                    );
                }
                Ok(())
            })
            .await
            .expect("inspect converged interaction schema");
        drop(store);
        assert_eq!(
            database_snapshot(&config.path).schema_objects,
            canonical_schema_objects()
        );
    }
}

#[tokio::test]
async fn reviewed_action_policy_migration_preserves_history_without_inventing_hints() {
    let home = TempDir::new().expect("versioned root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("version eleven database");
    store_migrations()
        .to_version(&mut conn, 11)
        .expect("apply effect-era migrations");
    for (id, state, admission) in [
        ("action:terminal", "succeeded", "always_ask"),
        ("action:pending", "proposed", "reviewer_may_approve"),
    ] {
        conn.execute(
            "INSERT INTO governed_actions (action_id, revision, owner_human_id, requesting_agent_id, capability_name, operation_token, effect, arguments_json, arguments_sha256, input_schema_json, authorization_context_json, safe_summary, state) VALUES (?1, 1, 'human:local', 'agent:test', 'fixture.call', 'token', 'write', '{}', ?2, '{}', json_object('admission_policy', ?3), 'Fixture call', ?4)",
            rusqlite::params![id, "0".repeat(64), admission, state],
        )
        .expect("legacy action");
    }
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("migrate reviewed actions");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM pragma_table_info('governed_actions') WHERE name = 'effect'", [], |row| row.get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                conn.query_row("SELECT review_route FROM governed_actions WHERE action_id = 'action:terminal'", [], |row| row.get::<_, String>(0))?,
                "human_review"
            );
            assert_eq!(
                conn.query_row("SELECT state FROM governed_actions WHERE action_id = 'action:pending'", [], |row| row.get::<_, String>(0))?,
                "superseded"
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM governed_actions WHERE read_only IS NOT NULL OR idempotent IS NOT NULL OR destructive IS NOT NULL OR open_world IS NOT NULL", [], |row| row.get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .await
        .expect("verify reviewed action migration");
}

#[tokio::test]
async fn known_v8_capability_auth_drift_is_repaired_without_losing_rows() {
    let home = TempDir::new().expect("drift root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("open v8 fixture");
    store_migrations()
        .to_version(&mut conn, 8)
        .expect("apply v8 migrations");

    conn.execute(
        "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Drift task', 'system', 'actor:system')",
        [],
    )
    .expect("task row");
    insert_planner_run(&conn, "run:drift").expect("run row");
    conn.execute(
        "INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, safe_config_json, auth_status, health_status, enabled, metadata_fingerprint) VALUES ('mcp:drift', 'Drift', 'streamable_http', '{}', 'needs_auth', 'healthy', 1, 'generation')",
        [],
    )
    .expect("MCP server row");

    let canonical_table_sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'capability_auth_requests'",
            [],
            |row| row.get(0),
        )
        .expect("read canonical v8 table SQL");
    let stale_table_sql = canonical_table_sql
        .replace("  origin_resumed_at TEXT,\n", "")
        .replace(
            "  FOREIGN KEY (governed_action_id, governed_action_revision)\n    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,\n",
            "  FOREIGN KEY (mcp_server_id) REFERENCES mcp_servers(mcp_server_id) ON DELETE CASCADE,\n  FOREIGN KEY (governed_action_id, governed_action_revision)\n    REFERENCES governed_actions(action_id, revision) ON DELETE RESTRICT,\n",
        );

    conn.execute_batch(
        r#"
        DROP TRIGGER capability_auth_requests_active_mcp_insert;
        DROP TRIGGER capability_auth_requests_active_mcp_update;
        DROP TRIGGER mcp_servers_active_capability_auth_delete;
        DROP INDEX capability_auth_requests_conversation_call;
        DROP INDEX capability_auth_requests_run_call;
        DROP INDEX capability_auth_requests_governed_action;
        DROP INDEX capability_auth_requests_attention;
        DROP INDEX capability_auth_requests_attempt;
        ALTER TABLE capability_auth_requests RENAME TO capability_auth_requests_stale_source;
        "#,
    )
    .expect("remove v8 objects that drifted in development");
    conn.execute_batch(&stale_table_sql)
        .expect("create stale v8 capability-auth table");
    for index_sql in [
        "CREATE UNIQUE INDEX capability_auth_requests_conversation_call\nON capability_auth_requests(conversation_id, turn_id, output_index)\nWHERE conversation_id IS NOT NULL AND governed_action_id IS NULL",
        "CREATE UNIQUE INDEX capability_auth_requests_run_call\nON capability_auth_requests(run_id, output_index)\nWHERE run_id IS NOT NULL AND governed_action_id IS NULL",
        "CREATE UNIQUE INDEX capability_auth_requests_governed_action\nON capability_auth_requests(governed_action_id, governed_action_revision)\nWHERE governed_action_id IS NOT NULL",
        "CREATE INDEX capability_auth_requests_attention\nON capability_auth_requests(owner_human_id, state, created_at, request_id)\nWHERE state IN ('awaiting_user', 'authorizing')",
        "CREATE INDEX capability_auth_requests_attempt\nON capability_auth_requests(authentication_attempt_id, state)\nWHERE authentication_attempt_id IS NOT NULL",
    ] {
        conn.execute(index_sql, [])
            .expect("restore stale v8 capability-auth index");
    }
    conn.execute(
        r#"
        INSERT INTO capability_auth_requests (
          request_id, owner_human_id, task_id, run_id, task_generation,
          requesting_agent_id, mcp_server_id, challenge_kind, authority_revision,
          capability_name, operation_token, input_schema_json, protected_arguments_ref,
          arguments_sha256, provider_selection_digest, output_index, result_context_json, state
        ) VALUES (
          'cap_auth:drift', 'human:local', 'task:valid', 'run:drift', 1,
          'agent:task-executor', 'mcp:drift', 'reauthenticate', 'generation:1',
          'mcp.auth/tool', 'operation', '{}', ?1, ?2, ?2, 0, '{}', 'awaiting_user'
        )
        "#,
        params!["a".repeat(32), "b".repeat(64)],
    )
    .expect("preserve stale capability-auth row");
    conn.execute_batch(
        r#"
        DROP TABLE capability_auth_requests_stale_source;
        PRAGMA user_version = 8;
        "#,
    )
    .expect("install known drift fixture");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("repair known v8 capability-auth drift");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            assert_eq!(
                count_where(conn, "capability_auth_requests", "request_id = 'cap_auth:drift'")?,
                1
            );
            assert_eq!(
                conn.query_row(
                    "SELECT origin_resumed_at FROM capability_auth_requests WHERE request_id = 'cap_auth:drift'",
                    [],
                    |row| row.get::<_, Option<String>>(0),
                )?,
                None
            );
            for trigger in [
                "capability_auth_requests_active_mcp_insert",
                "capability_auth_requests_active_mcp_update",
                "mcp_servers_active_capability_auth_delete",
            ] {
                assert!(schema_object_exists(conn, "trigger", trigger)?);
            }
            Ok(())
        })
        .await
        .expect("verify repaired capability-auth schema");
    drop(store);

    assert_eq!(
        database_snapshot(&config.path).schema_objects,
        canonical_schema_objects()
    );
}

#[tokio::test]
async fn transitional_mcp_auth_schema_drops_raw_pending_arguments() {
    let home = TempDir::new().expect("transitional MCP auth root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    let mut conn = Connection::open(&config.path).expect("version five database");
    store_migrations()
        .to_version(&mut conn, 5)
        .expect("apply transitional MCP auth migration");
    conn.execute(
        "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Valid task', 'system', 'actor:system')",
        [],
    )
    .expect("task row");
    insert_planner_run(&conn, "run:mcp-auth").expect("run row");
    conn.execute(
        "INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, safe_config_json, auth_status, health_status, enabled, metadata_fingerprint) VALUES ('mcp:auth', 'Auth', 'streamable_http', '{}', 'needs_auth', 'healthy', 1, 'generation')",
        [],
    )
    .expect("MCP server row");
    conn.execute(
        "INSERT INTO mcp_auth_requests (request_id, owner_human_id, task_id, run_id, requesting_agent_id, mcp_server_id, capability_name, operation_token, input_schema_json, arguments_json, arguments_sha256, output_index, state) VALUES ('mcp_auth:preserved', 'human:local', 'task:valid', 'run:mcp-auth', 'agent:task-executor', 'mcp:auth', 'mcp.auth/tool', 'operation', '{}', '{}', ?1, 0, 'awaiting_user')",
        ["0".repeat(64)],
    )
    .expect("authentication request row");
    drop(conn);

    let store = NoemaStore::open(&config)
        .await
        .expect("repair transitional MCP auth schema");
    store
        .with_connection(|conn| {
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?,
                STORE_SCHEMA_VERSION
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM capability_auth_requests", [], |row| {
                    row.get::<_, i64>(0)
                })?,
                0
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('governed_actions') WHERE name = 'authentication_pending'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                1
            );
            assert!(!schema_object_exists(conn, "table", "mcp_auth_requests")?);
            assert!(schema_object_exists(conn, "table", "capability_auth_requests")?);
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('capability_auth_requests') WHERE name = 'arguments_json'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                0
            );
            assert!(schema_object_exists(
                conn,
                "index",
                "capability_auth_requests_governed_action"
            )?);
            Ok(())
        })
        .await
        .expect("verify repaired schema");
    drop(store);

    assert_eq!(
        database_snapshot(&config.path).schema_objects,
        canonical_schema_objects()
    );
}

#[tokio::test]
async fn unknown_unversioned_schema_is_rejected_without_mutation() {
    let home = TempDir::new().expect("unknown schema root");
    let config = store_config(home.path());
    create_legacy_v9_database(&config.path);
    Connection::open(&config.path)
        .expect("unknown schema fixture")
        .execute_batch("CREATE TABLE unknown_owner_data (value TEXT NOT NULL);")
        .expect("unknown table");

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Shape { .. },
            ..
        }
    ));
}

#[tokio::test]
async fn zero_byte_database_runs_all_migrations() {
    let home = TempDir::new().expect("zero-byte root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent")).expect("database parent");
    fs::File::create(&config.path).expect("zero-byte database");

    drop(NoemaStore::open(&config).await.expect("migrate empty file"));

    assert_eq!(
        database_snapshot(&config.path).schema_version,
        Some(STORE_SCHEMA_VERSION as i64)
    );
}

#[tokio::test]
async fn opening_partial_schema_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open partial fixture");
    conn.execute_batch("DROP INDEX work_events_run_cursor;")
        .expect("remove one schema object");
    drop(conn);

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Shape { .. },
            ..
        }
    ));
}

#[tokio::test]
async fn opening_future_schema_version_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_database(&config.path);
    let conn = Connection::open(&config.path).expect("open future fixture");
    conn.pragma_update(None, "user_version", STORE_SCHEMA_VERSION + 1)
        .expect("install future version");
    drop(conn);

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Version {
                expected_version: STORE_SCHEMA_VERSION,
                found_version,
            },
            ..
        } if found_version == (STORE_SCHEMA_VERSION + 1) as i64
    ));
}

#[tokio::test]
async fn opening_invalid_legacy_marker_is_rejected_without_mutation() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_legacy_v9_database(&config.path);
    let conn = Connection::open(&config.path).expect("open extra marker fixture");
    conn.execute(
        "INSERT INTO schema_state (name, version) VALUES ('unexpected_schema', ?1)",
        [9],
    )
    .expect("install extra marker");
    drop(conn);

    let error = assert_rejected_without_mutation(&config).await;
    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::LegacyMarker,
            ..
        }
    ));
}

#[tokio::test]
async fn rejected_pending_wal_schema_preserves_main_wal_and_shm_bytes() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_wal_database(&config.path);
    let conn = Connection::open(&config.path).expect("open pending WAL fixture");
    conn.pragma_update(None, "user_version", STORE_SCHEMA_VERSION + 1)
        .expect("write future version into WAL");
    assert!(sidecar_path(&config.path, "-wal").exists());
    assert!(sidecar_path(&config.path, "-shm").exists());
    let before = database_snapshot(&config.path);

    let error = NoemaStore::open(&config)
        .await
        .expect_err("pending future schema must be rejected");

    assert!(matches!(error, StoreError::IncompatibleSchema { .. }));
    assert_eq!(database_snapshot(&config.path), before);
    drop(conn);
}

#[tokio::test]
async fn current_schema_with_pending_wal_rows_opens_and_preserves_data() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    create_current_wal_database(&config.path);
    let writer = Connection::open(&config.path).expect("open pending WAL fixture");
    writer
        .execute(
            "INSERT INTO humans (human_id, display_name) VALUES ('human:pending', 'Pending')",
            [],
        )
        .expect("write application row into WAL");
    assert!(sidecar_path(&config.path, "-wal").exists());

    let store = NoemaStore::open(&config)
        .await
        .expect("current pending-WAL schema must open");
    let display_name = store
        .with_connection(|conn| {
            conn.query_row(
                "SELECT display_name FROM humans WHERE human_id = 'human:pending'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
        .expect("read pending WAL row");

    assert_eq!(display_name, "Pending");
    drop(store);
    drop(writer);
}

#[tokio::test]
async fn hot_rollback_journal_is_recovered_only_in_private_inspection_copy() {
    let home = TempDir::new().expect("temp store root");
    let source_path = home.path().join("source.sqlite3");
    let target_path = home.path().join("db/noema.sqlite3");
    fs::create_dir_all(target_path.parent().expect("database parent"))
        .expect("create database parent");
    let mut source = Connection::open(&source_path).expect("open crash source");
    source
        .execute_batch(
            r#"
            PRAGMA journal_mode = DELETE;
            PRAGMA synchronous = FULL;
            CREATE TABLE seed (value TEXT);
            DROP TABLE seed;
            VACUUM;
            PRAGMA cache_size = 1;
            PRAGMA cache_spill = ON;
            "#,
        )
        .expect("prepare empty rollback-journal database");
    let tx = source
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .expect("begin interrupted bootstrap");
    tx.execute_batch(
        r#"
        CREATE TABLE schema_state (
          name TEXT PRIMARY KEY NOT NULL,
          version INTEGER NOT NULL,
          applied_at TEXT NOT NULL
        );
        CREATE TABLE interrupted_bootstrap (payload BLOB NOT NULL);
        INSERT INTO interrupted_bootstrap (payload) VALUES (zeroblob(2097152));
        "#,
    )
    .expect("write partial bootstrap pages");
    let source_journal = sidecar_path(&source_path, "-journal");
    assert!(source_journal.exists());
    assert!(
        fs::metadata(&source_journal)
            .expect("journal metadata")
            .len()
            > 512
    );
    fs::copy(&source_path, &target_path).expect("copy interrupted main database");
    let target_journal = sidecar_path(&target_path, "-journal");
    fs::copy(&source_journal, &target_journal).expect("copy hot rollback journal");
    drop(tx);
    drop(source);

    let main_before = fs::read(&target_path).expect("snapshot interrupted main database");
    let journal_before = fs::read(&target_journal).expect("snapshot hot rollback journal");
    assert!(
        inspect_empty_schema_for_test(&target_path).expect("inspect hot-journal family"),
        "private-copy recovery should reveal the pre-bootstrap empty schema"
    );
    assert_eq!(
        fs::read(&target_path).expect("reread interrupted main database"),
        main_before
    );
    assert_eq!(
        fs::read(&target_journal).expect("reread hot rollback journal"),
        journal_before
    );

    NoemaStore::open(&StoreConfig::new(&target_path))
        .await
        .expect("recover hot journal and bootstrap current schema");
}

#[tokio::test]
async fn non_sqlite_file_is_typed_incompatible_and_unchanged() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    fs::create_dir_all(config.path.parent().expect("database parent"))
        .expect("create database parent");
    fs::write(&config.path, b"not a sqlite database").expect("write invalid database");
    let before = fs::read(&config.path).expect("snapshot invalid database");

    let error = NoemaStore::open(&config)
        .await
        .expect_err("non-SQLite file must be rejected");

    assert!(matches!(
        error,
        StoreError::IncompatibleSchema {
            kind: SchemaIncompatibility::Unreadable,
            ..
        }
    ));
    assert_eq!(
        fs::read(&config.path).expect("reread invalid database"),
        before
    );
    assert!(!sidecar_path(&config.path, "-wal").exists());
    assert!(!sidecar_path(&config.path, "-shm").exists());
}

#[test]
fn injected_mid_migration_failure_rolls_back_every_schema_object() {
    let home = TempDir::new().expect("temp store root");
    let path = home.path().join("db/noema.sqlite3");
    fs::create_dir_all(path.parent().expect("database parent")).expect("create database parent");
    fs::File::create(&path).expect("create empty database file");
    let mut conn = Connection::open(&path).expect("open empty fixture");
    let before = database_snapshot(&path);
    let faulty_schema = LEGACY_V9_SCHEMA_SQL.replacen(
        "INSERT INTO schema_state (name, version, applied_at)",
        "SELECT noema_injected_bootstrap_failure();\n\nINSERT INTO schema_state (name, version, applied_at)",
        1,
    );

    run_migration_for_test(&mut conn, &faulty_schema)
        .expect_err("injected migration failure must abort");
    drop(conn);

    let after = database_snapshot(&path);
    assert_eq!(after, before);
    assert!(after.schema_objects.is_empty());
    assert!(after.schema_markers.is_none());
}

#[tokio::test]
async fn immutable_schema_inspection_handles_uri_reserved_path_characters() {
    let home = TempDir::new().expect("temp store root");
    let config = StoreConfig::new(
        home.path()
            .join("db with space")
            .join("noema?#schema.sqlite3"),
    );
    drop(NoemaStore::open(&config).await.expect("bootstrap store"));

    NoemaStore::open(&config)
        .await
        .expect("inspect reserved-character path");
}

#[tokio::test]
async fn v44_upgrade_preserves_passkeys_repairs_terminal_records_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v42 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v42 database");
    store_migrations()
        .to_version(&mut connection, 42)
        .expect("construct v42 schema");
    connection
        .execute(
            r#"INSERT INTO human_passkeys (human_id, credential_json)
               VALUES ('human:local', '{"cred":{"cred_id":"legacy-credential"}}')"#,
            [],
        )
        .expect("legacy passkey");
    connection
        .execute(
            "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'workflow:personal:default', 'stage:personal:done', 'Terminal records', 'system', 'actor:system')",
            [],
        )
        .expect("task");
    insert_planner_run(&connection, "run:terminal-records").expect("run");
    connection
        .execute(
            "UPDATE agent_runs SET status = 'completed' WHERE run_id = 'run:terminal-records'",
            [],
        )
        .expect("finish run");
    connection
        .execute_batch(
            r#"
            INSERT INTO agent_run_items (item_id, run_id, sequence_index, kind, status)
            VALUES
              ('run_item:terminal-output', 'run:terminal-records', 1, 'assistant_output', 'running'),
              ('run_item:terminal-call', 'run:terminal-records', 2, 'tool_call', 'running');
            INSERT INTO runtime_debug_spans (span_id, agent_run_id, category, name)
            VALUES ('debug_span:terminal-run', 'run:terminal-records', 'provider', 'terminal run');
            "#,
        )
        .expect("run child records");
    insert_migration_conversation(
        &connection,
        "conversation:terminal-records",
        "turn:terminal-records",
    );
    connection
        .execute_batch(
            r#"
            INSERT INTO conversation_items (
              item_id, conversation_id, turn_id, sequence_index, kind, status, author_actor_id
            ) VALUES (
              'item:terminal-call', 'conversation:terminal-records', 'turn:terminal-records',
              1, 'tool_call', 'running', 'agent:primary'
            );
            INSERT INTO runtime_debug_spans (span_id, conversation_turn_id, category, name)
            VALUES ('debug_span:terminal-turn', 'turn:terminal-records', 'tool', 'terminal turn');
            "#,
        )
        .expect("turn child records");
    drop(connection);

    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("upgrade v42 database"),
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    assert_eq!(
        connection
            .query_row(
                r#"SELECT
                  (SELECT status FROM agent_run_items WHERE item_id = 'run_item:terminal-output'),
                  (SELECT status FROM agent_run_items WHERE item_id = 'run_item:terminal-call'),
                  (SELECT status FROM conversation_items WHERE item_id = 'item:terminal-call'),
                  (SELECT count(*) FROM runtime_debug_spans WHERE span_id IN ('debug_span:terminal-run', 'debug_span:terminal-turn') AND status = 'completed' AND ended_at IS NOT NULL),
                  (SELECT credential_id FROM human_passkeys WHERE human_id = 'human:local'),
                  (SELECT user_version FROM pragma_user_version)"#,
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?, row.get::<_, String>(4)?, row.get::<_, usize>(5)?)),
            )
            .expect("repaired records"),
        (
            "completed".to_string(),
            "failed".to_string(),
            "failed".to_string(),
            2,
            "legacy-credential".to_string(),
            STORE_SCHEMA_VERSION,
        )
    );
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh v44 root");
    let fresh_config = store_config(fresh_home.path());
    drop(
        NoemaStore::open(&fresh_config)
            .await
            .expect("fresh v44 schema"),
    );
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn v46_upgrade_removes_legacy_clients_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v45 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v45 database");
    store_migrations()
        .to_version(&mut connection, 45)
        .expect("construct v45 schema");
    connection
        .execute(
            "INSERT INTO clients (client_id, owner_human_id, display_name, token_hash) VALUES ('legacy-client', 'human:local', 'Legacy client', zeroblob(32))",
            [],
        )
        .expect("legacy client");
    drop(connection);

    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("upgrade v45 database"),
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    assert_eq!(
        count_where(&connection, "clients", "client_id = 'legacy-client'").unwrap(),
        0
    );
    let columns = table_columns(&connection, "clients").unwrap();
    assert!(!columns.iter().any(|column| column == "token_hash"));
    assert!(!columns.iter().any(|column| column == "auth_kind"));
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh v46 root");
    let fresh_config = store_config(fresh_home.path());
    drop(
        NoemaStore::open(&fresh_config)
            .await
            .expect("fresh v46 schema"),
    );
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn v47_upgrade_invalidates_unbound_web_push_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v46 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v46 database");
    store_migrations()
        .to_version(&mut connection, 46)
        .expect("construct v46 schema");
    connection
        .execute(
            "INSERT INTO web_push_subscriptions (subscription_id, owner_human_id, endpoint, p256dh, auth_secret) VALUES ('push:legacy', 'human:local', 'https://push.example/legacy', ?1, ?2)",
            params!["p".repeat(40), "a".repeat(16)],
        )
        .expect("legacy Push registration");
    drop(connection);

    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("upgrade v46 database"),
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM web_push_subscriptions", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count Push registrations"),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info('web_push_subscriptions') WHERE name = 'browser_session_hash' AND \"notnull\" = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("session binding column"),
        1
    );
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh v47 root");
    let fresh_config = store_config(fresh_home.path());
    drop(
        NoemaStore::open(&fresh_config)
            .await
            .expect("fresh v47 schema"),
    );
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

#[tokio::test]
async fn v48_model_preference_speed_upgrade_defaults_to_standard_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v47 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v47 database");
    store_migrations()
        .to_version(&mut connection, 47)
        .expect("construct v47 schema");
    store_migrations()
        .to_latest(&mut connection)
        .expect("upgrade to v48");

    for table in [
        "agent_runtime_preferences",
        "auxiliary_model_preferences",
        "default_model_preference",
        "task_model_pool_entries",
        "agent_runs",
        "conversation_interactions",
    ] {
        let fast_columns = connection
            .query_row(
                &format!(
                    "SELECT count(*) FROM pragma_table_info('{table}') WHERE name LIKE '%fast_mode' AND \"notnull\" = 1 AND dflt_value = '0'"
                ),
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("fast-mode columns");
        assert_eq!(fast_columns, 1, "{table}");
    }
    drop(connection);

    let fresh_home = TempDir::new().expect("fresh v48 root");
    let fresh_config = store_config(fresh_home.path());
    drop(
        NoemaStore::open(&fresh_config)
            .await
            .expect("fresh v48 schema"),
    );
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

fn count_where(conn: &Connection, table: &str, predicate: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE {predicate}"),
        [],
        |row| row.get(0),
    )
}

fn insert_migration_conversation(conn: &Connection, conversation_id: &str, turn_id: &str) {
    conn.execute(
        "INSERT INTO agents (agent_id, display_name, system_role) VALUES ('agent:primary', 'Primary', 'primary') ON CONFLICT(agent_id) DO NOTHING",
        [],
    )
    .expect("insert migration agent");
    conn.execute(
        "INSERT INTO conversations (conversation_id, owner_object_type, owner_object_id, primary_human_id, primary_agent_id, provider, model) VALUES (?1, 'human', 'human:local', 'human:local', 'agent:primary', 'codex', 'gpt-test')",
        [conversation_id],
    )
    .expect("insert migration conversation");
    conn.execute(
        "INSERT INTO conversation_turns (turn_id, conversation_id, status, metadata_json) VALUES (?1, ?2, 'completed', '{\"turn_index\":1}')",
        [turn_id, conversation_id],
    )
    .expect("insert migration turn");
}

fn schema_object_exists(
    conn: &Connection,
    object_type: &str,
    name: &str,
) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2)",
        params![object_type, name],
        |row| row.get(0),
    )
}

#[tokio::test]
async fn v37_oauth_authority_upgrade_terminalizes_old_requests_and_matches_fresh_schema() {
    let upgrade_home = TempDir::new().expect("v37 root");
    let upgrade_config = store_config(upgrade_home.path());
    fs::create_dir_all(upgrade_config.path.parent().expect("database parent"))
        .expect("database parent");
    let mut connection = Connection::open(&upgrade_config.path).expect("v37 database");
    store_migrations()
        .to_version(&mut connection, 37)
        .expect("construct v37 schema");
    connection
        .execute(
            "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'OAuth migration', 'system', 'actor:system')",
            [],
        )
        .expect("task");
    insert_planner_run(&connection, "run:oauth-migration").expect("run");
    connection
        .execute(
            r#"INSERT INTO capability_auth_requests (
              request_id, owner_human_id, task_id, run_id, task_generation,
              requesting_agent_id, adapter_connection_id, challenge_kind,
              authority_revision, capability_name, operation_token, input_schema_json,
              protected_arguments_ref, arguments_sha256, provider_selection_digest,
              output_index, result_context_json, state
            ) VALUES (
              'cap_auth:oauth-migration', 'human:local', 'task:valid',
              'run:oauth-migration', 1, 'agent:task-executor', ?1,
              'reauthenticate', 'grant:1', 'adapter.read', 'operation', '{}',
              ?2, ?3, ?3, 0, '{}', 'authorizing'
            )"#,
            params!["a".repeat(32), "b".repeat(32), "c".repeat(64)],
        )
        .expect("active adapter authentication");
    drop(connection);

    drop(
        NoemaStore::open(&upgrade_config)
            .await
            .expect("upgrade v37"),
    );
    let connection = Connection::open(&upgrade_config.path).expect("upgraded database");
    assert_eq!(
        connection
            .query_row(
                "SELECT state, supersession_reason FROM capability_auth_requests WHERE request_id = 'cap_auth:oauth-migration'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("terminal request"),
        (
            "superseded".to_string(),
            "adapter_oauth_authority_replaced".to_string()
        )
    );
    for table in [
        "adapter_oauth_profiles",
        "adapter_oauth_applications",
        "adapter_external_accounts",
        "adapter_oauth_grants",
    ] {
        assert!(
            schema_object_exists(&connection, "table", table).expect("schema lookup"),
            "missing {table}"
        );
    }

    let fresh_home = TempDir::new().expect("fresh root");
    let fresh_config = store_config(fresh_home.path());
    drop(NoemaStore::open(&fresh_config).await.expect("fresh schema"));
    assert_eq!(
        database_snapshot(&upgrade_config.path).schema_objects,
        database_snapshot(&fresh_config.path).schema_objects
    );
}

fn table_columns(conn: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    statement
        .query_map([], |row| row.get(1))?
        .collect::<Result<Vec<_>, _>>()
}

fn insert_planner_run(conn: &Connection, run_id: &str) -> rusqlite::Result<usize> {
    conn.execute(
        r#"
        INSERT INTO agent_runs (
          run_id, instance_name, task_id, task_generation, run_kind, agent_id,
          attempt_index, review_round, provider_kind, provider_account_id,
          provider_instance_key, selection_mode, model_profile,
          max_provider_continuations, max_tool_calls, max_active_minutes,
          progress_audit_interval, max_automatic_retries, max_review_rounds, status
        ) VALUES (
          ?1, ?2, 'task:valid', 1, 'planner', 'agent:task-executor',
          0, 0, 'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
          80, 400, 120, 20, 3, 3, 'queued'
        )
        "#,
        [run_id, &format!("Planner {run_id}")],
    )
}

fn insert_legacy_delegated_contract(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        INSERT INTO task_execution_contracts (
          contract_id, task_id, version, task_generation, origin,
          request_markdown, complexity,
          executor_provider_kind, executor_provider_account_id,
          executor_provider_instance_key, executor_selection_mode,
          executor_model_profile,
          reviewer_provider_kind, reviewer_provider_account_id,
          reviewer_provider_instance_key, reviewer_selection_mode,
          reviewer_model_profile,
          max_provider_continuations, max_tool_calls, max_active_minutes,
          progress_audit_interval, max_automatic_retries, max_review_rounds,
          workspace_id_snapshot, workspace_name_snapshot,
          workspace_description_snapshot, created_by_actor_id
        ) VALUES (
          'contract:one', 'task:valid', 1, 1, 'delegated',
          'Complete the task', 'simple',
          'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
          'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
          80, 400, 120, 20, 3, 3,
          'workspace:personal', 'Personal', '', 'actor:system'
        );
        "#,
    )
}
