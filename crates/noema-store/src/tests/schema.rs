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
async fn fresh_migrations_are_exact_idempotent_and_enforce_foreign_keys() {
    let home = TempDir::new().expect("temp store root");
    let config = store_config(home.path());
    let store = NoemaStore::open(&config)
        .await
        .expect("migrate fresh store");

    store
        .with_connection(|conn| {
            assert_eq!(conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))?, 1);
            assert_eq!(count_where(conn, "humans", "human_id = 'human:local'")?, 1);
            assert_eq!(count_where(conn, "workspaces", "workspace_id = 'workspace:personal' AND name = 'Personal' AND description = '' AND is_personal = 1 AND archived_at IS NULL AND revision = 1")?, 1);
            assert_eq!(count_where(conn, "workspace_memberships", "workspace_id = 'workspace:personal' AND human_id = 'human:local' AND role = 'owner'")?, 1);
            assert_eq!(count_where(conn, "workflow_definitions", "workflow_id = 'workflow:personal:default' AND workspace_id = 'workspace:personal' AND name = 'Personal workflow' AND is_default = 1 AND revision = 1")?, 1);
            assert_eq!(count_where(conn, "projects", "1 = 1")?, 0);

            let mut statement = conn.prepare(
                "SELECT stage_id, stable_key, display_name, ordinal, system_behavior, board_visible FROM workflow_stages WHERE workflow_id = 'workflow:personal:default' ORDER BY ordinal",
            )?;
            let stages = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(
                stages,
                vec![
                    stage("inbox", "Inbox", 10, "intake", 1),
                    stage("queue", "Queue", 20, "dispatch", 1),
                    stage("doing", "Doing", 30, "active", 1),
                    stage("waiting", "Waiting", 40, "human_gate", 1),
                    stage("done", "Done", 50, "terminal_success", 1),
                    stage("cancelled", "Cancelled", 60, "terminal_cancelled", 0),
                ]
            );

            let policy = conn.query_row(
                "SELECT max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, max_automatic_retries, max_review_rounds FROM task_execution_policy WHERE policy_id = 'default'",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?)),
            )?;
            assert_eq!(policy, (80, 400, 120, 20, 3, 3));
            assert_eq!(conn.query_row("PRAGMA user_version", [], |row| row.get::<_, usize>(0))?, STORE_SCHEMA_VERSION);
            assert!(!schema_object_exists(conn, "table", "schema_state")?);
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
                (
                    "workflow_stages",
                    "workflow_id = 'workflow:personal:default'",
                    6,
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
async fn current_schema_enforces_projection_history_and_ledger_invariants() {
    let home = TempDir::new().expect("temp store root");
    let store = NoemaStore::open(&store_config(home.path()))
        .await
        .expect("migrate fresh store");

    store
        .with_connection(|conn| {
            for table in [
                "workspaces", "workspace_memberships", "projects", "workflow_definitions",
                "workflow_stages", "tasks", "task_execution_contracts",
                "task_contract_criteria", "task_gates", "task_messages", "agent_runs",
                "task_submissions", "task_reviews", "work_events",
                "work_notification_outbox", "work_command_receipts",
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
            assert!(!table_columns(conn, "tasks")?.iter().any(|column| column == "status"));
            let run_columns = table_columns(conn, "agent_runs")?;
            assert!(!run_columns.iter().any(|column| column == "priority"));
            assert!(!run_columns.iter().any(|column| column == "resume_message"));

            conn.execute_batch(
                r#"
                INSERT INTO workspaces (workspace_id, name) VALUES ('workspace:other', 'Other');
                INSERT INTO projects (project_id, workspace_id, name)
                VALUES ('project:other', 'workspace:other', 'Other project');
                INSERT INTO workflow_definitions (workflow_id, workspace_id, name)
                VALUES ('workflow:other', 'workspace:personal', 'Other workflow');
                "#,
            )?;
            assert!(conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, project_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:cross-project', 'workspace:personal', 'project:other', 'workflow:personal:default', 'stage:personal:inbox', 'Bad project', 'system', 'actor:system')",
                [],
            ).is_err());
            assert!(conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:cross-stage', 'workspace:personal', 'workflow:other', 'stage:personal:inbox', 'Bad stage', 'system', 'actor:system')",
                [],
            ).is_err());

            conn.execute(
                "INSERT INTO tasks (task_id, workspace_id, workflow_id, stage_id, title, source_kind, created_by_actor_id) VALUES ('task:valid', 'workspace:personal', 'workflow:personal:default', 'stage:personal:queue', 'Valid task', 'system', 'actor:system')",
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

            insert_delegated_contract(conn)?;
            conn.execute(
                "INSERT INTO task_contract_criteria (criterion_id, contract_id, ordinal, description) VALUES ('criterion:one', 'contract:one', 1, 'First')",
                [],
            )?;
            assert!(conn.execute(
                "INSERT INTO task_contract_criteria (criterion_id, contract_id, ordinal, description) VALUES ('criterion:two', 'contract:one', 1, 'Second')",
                [],
            ).is_err());

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
                    "INSERT INTO capability_auth_requests (request_id, owner_human_id, task_id, run_id, task_generation, requesting_agent_id, mcp_server_id, challenge_kind, authority_revision, capability_name, operation_token, input_schema_json, protected_arguments_ref, arguments_sha256, provider_selection_digest, output_index, result_context_json, state) VALUES (?1, 'human:local', 'task:valid', 'run:two', 1, 'agent:task-executor', ?2, 'reauthenticate', 'generation:1', 'mcp.auth/tool', 'operation', '{}', ?3, ?4, ?4, 0, '{}', 'awaiting_user')",
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
                "UPDATE capability_auth_requests SET state = 'superseded' WHERE mcp_server_id = 'mcp:auth-integrity'",
                [],
            )?;
            conn.execute(
                "DELETE FROM mcp_servers WHERE mcp_server_id = 'mcp:auth-integrity'",
                [],
            )?;
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM capability_auth_requests WHERE mcp_server_id = 'mcp:auth-integrity'",
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

fn stage(
    stable_key: &str,
    display_name: &str,
    ordinal: i64,
    behavior: &str,
    board_visible: i64,
) -> (String, String, String, i64, String, i64) {
    (
        format!("stage:personal:{stable_key}"),
        stable_key.to_string(),
        display_name.to_string(),
        ordinal,
        behavior.to_string(),
        board_visible,
    )
}

fn count_where(conn: &Connection, table: &str, predicate: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE {predicate}"),
        [],
        |row| row.get(0),
    )
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
          run_id, instance_name, task_id, task_generation, contract_id, run_kind, agent_id,
          attempt_index, review_round, provider_kind, provider_account_id,
          provider_instance_key, selection_mode, model_profile,
          max_provider_continuations, max_tool_calls, max_active_minutes,
          progress_audit_interval, max_automatic_retries, max_review_rounds, status
        ) VALUES (
          ?1, ?2, 'task:valid', 1, NULL, 'planner', 'agent:task-executor',
          0, 0, 'codex', 'provider_account:codex:default',
          'provider_account:codex:default', 'explicit_profile', 'gpt-5.6-luna',
          80, 400, 120, 20, 3, 3, 'queued'
        )
        "#,
        [run_id, &format!("Planner {run_id}")],
    )
}

fn insert_delegated_contract(conn: &Connection) -> rusqlite::Result<()> {
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
