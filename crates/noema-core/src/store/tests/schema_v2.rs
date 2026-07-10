use std::{fs, path::Path};

use rusqlite::Connection;
use tempfile::TempDir;

use super::super::{NoemaStore, StoreConfig, StoreError, schema::SchemaVersion};

#[tokio::test]
async fn empty_database_bootstraps_v2_once_and_reopen_preserves_marker() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);

    let store = NoemaStore::open(&config).await.expect("bootstrap v2");
    assert_eq!(store.schema_version().await.expect("schema version"), 2);
    let first_marker = schema_marker(&store).await;
    assert_eq!(first_marker.0, SchemaVersion::V2.as_i64());
    assert!(!first_marker.1.is_empty());
    store.close().await.expect("close");

    let reopened = NoemaStore::open(&config)
        .await
        .expect("validate existing v2");
    assert_eq!(schema_marker(&reopened).await, first_marker);
}

#[tokio::test]
async fn schema_marker_cannot_be_replaced_or_duplicated() {
    let store = test_store().await;
    let before = schema_marker(&store).await;
    store
        .with_connection(|conn| {
            assert!(conn.execute("INSERT OR REPLACE INTO schema_state (name, version, structural_fingerprint, applied_at) VALUES ('sqlite_store_v2', 2, 'replacement', '2026-01-01T00:00:00.000Z')", []).is_err());
            assert!(conn.execute("INSERT INTO schema_state (name, version, structural_fingerprint) VALUES ('other', 2, 'other')", []).is_err());
            Ok(())
        })
        .await
        .expect("marker checks");
    assert_eq!(schema_marker(&store).await, before);
}

#[tokio::test]
async fn existing_v1_database_is_refused_without_mutation() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    fs::create_dir_all(paths.db_dir()).expect("db dir");
    let db_path = paths.sqlite_db_path();
    let conn = Connection::open(&db_path).expect("v1 database");
    conn.execute_batch(
        "CREATE TABLE schema_state (name TEXT PRIMARY KEY, version INTEGER NOT NULL);\
         INSERT INTO schema_state VALUES ('sqlite_store_v1', 1);\
         CREATE TABLE humans (human_id TEXT PRIMARY KEY);",
    )
    .expect("v1 schema");
    drop(conn);
    let before = database_file_set(&db_path);

    let error = NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect_err("v1 must require reset");

    assert!(matches!(error, StoreError::ResetRequired { .. }));
    assert_eq!(database_file_set(&db_path), before);
}

#[tokio::test]
async fn arbitrary_nonempty_schema_is_refused_without_mutation() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    fs::create_dir_all(paths.db_dir()).expect("db dir");
    let conn = Connection::open(paths.sqlite_db_path()).expect("database");
    conn.execute_batch("CREATE TABLE unrelated (id TEXT PRIMARY KEY)")
        .expect("arbitrary schema");
    drop(conn);
    let before = database_file_set(&paths.sqlite_db_path());

    let error = NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect_err("arbitrary schema must require reset");

    assert!(matches!(error, StoreError::ResetRequired { .. }));
    assert_eq!(database_file_set(&paths.sqlite_db_path()), before);
}

#[tokio::test]
async fn malformed_nonempty_database_is_refused_without_mutation() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    fs::create_dir_all(paths.db_dir()).expect("db dir");
    fs::write(paths.sqlite_db_path(), b"not a sqlite database").expect("malformed database");
    let before = database_file_set(&paths.sqlite_db_path());

    let error = NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect_err("malformed database must require reset");

    assert!(matches!(error, StoreError::ResetRequired { .. }));
    assert_eq!(database_file_set(&paths.sqlite_db_path()), before);
}

#[tokio::test]
async fn valid_v2_with_live_wal_reopens_without_being_misclassified_as_empty() {
    let root = TempDir::new().expect("temp root");
    let home_path = root.path().join("Noema home #1 ? live WAL");
    let paths = crate::NoemaPaths::from_noema_home(&home_path).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let first = NoemaStore::open(&config).await.expect("bootstrap v2");
    first
        .ensure_default_actors()
        .await
        .expect("write into live WAL");
    assert!(sidecar_path(&paths.sqlite_db_path(), "-wal").exists());

    let second = NoemaStore::open(&config)
        .await
        .expect("validate schema including live WAL");

    assert_eq!(second.schema_version().await.expect("version"), 2);
    assert!(
        second
            .get_human("human:local")
            .await
            .expect("human lookup")
            .is_some()
    );
}

#[tokio::test]
async fn concrete_ownership_requires_exactly_one_foreign_key() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let error = store
        .with_connection(|conn| {
            conn.execute(
                "INSERT INTO conversations (conversation_id, owner_human_id, owner_agent_id, provider) VALUES ('conversation:invalid', 'human:local', 'agent:primary', 'codex')",
                [],
            )?;
            Ok(())
        })
        .await
        .expect_err("two concrete owners must fail");
    assert!(error.to_string().contains("CHECK"));

    let columns = table_columns(&store, "conversations").await;
    assert!(!columns.contains(&"owner_object_type".to_string()));
    assert!(!columns.contains(&"owner_object_id".to_string()));
}

#[tokio::test]
async fn conversation_children_cascade_and_provenance_is_set_null() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .with_connection(|conn| {
            conn.execute(
                "INSERT INTO conversations (conversation_id, owner_human_id, primary_human_id, provider) VALUES ('conversation:cascade', 'human:local', 'human:local', 'codex')",
                [],
            )?;
            conn.execute(
                "INSERT INTO conversation_turns (turn_id, conversation_id, status) VALUES ('turn:cascade', 'conversation:cascade', 'running')",
                [],
            )?;
            conn.execute(
                "INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_agent_id) VALUES ('item:cascade', 'conversation:cascade', 'turn:cascade', 1, 'assistant_text', 'completed', 'agent:primary')",
                [],
            )?;
            conn.execute(
                "INSERT INTO artifacts (artifact_id, owner_human_id, title, artifact_kind, storage_kind, created_by_agent_id, source_conversation_id, source_turn_id, source_item_id) VALUES ('artifact:audit', 'human:local', 'Audit', 'document', 'external_url', 'agent:primary', 'conversation:cascade', 'turn:cascade', 'item:cascade')",
                [],
            )?;
            conn.execute("DELETE FROM conversations WHERE conversation_id = 'conversation:cascade'", [])?;
            let children: i64 = conn.query_row(
                "SELECT (SELECT COUNT(*) FROM conversation_turns WHERE conversation_id = 'conversation:cascade') + (SELECT COUNT(*) FROM conversation_items WHERE conversation_id = 'conversation:cascade')",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(children, 0);
            let provenance: (Option<String>, Option<String>, Option<String>) = conn.query_row(
                "SELECT source_conversation_id, source_turn_id, source_item_id FROM artifacts WHERE artifact_id = 'artifact:audit'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            assert_eq!(provenance, (None, None, None));
            assert!(
                conn.execute("DELETE FROM humans WHERE human_id = 'human:local'", [])
                    .is_err(),
                "canonical owner deletion must be restricted"
            );
            Ok(())
        })
        .await
        .expect("declared delete actions");
}

#[tokio::test]
async fn provider_preferences_and_bindings_cascade_with_account() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO provider_accounts (provider_account_id, provider_kind, account_key, display_name, auth_method, is_active, is_default, status) VALUES ('provider_account:cascade', 'codex', 'cascade', 'Cascade', 'none', 1, 0, 'authenticated')", [])?;
            conn.execute("INSERT INTO agent_runtime_preferences (agent_id, provider_kind, provider_account_id, model_profile) VALUES ('agent:primary', 'codex', 'provider_account:cascade', 'model')", [])?;
            conn.execute("INSERT INTO auxiliary_model_preferences (task_id, provider_kind, provider_account_id, model_profile) VALUES ('web_fetch_summarizer', 'codex', 'provider_account:cascade', 'model')", [])?;
            conn.execute("INSERT INTO provider_capability_bindings (binding_id, tool_name, capability_id, provider_account_id) VALUES ('binding:cascade', 'web.search', 'web.search', 'provider_account:cascade')", [])?;
            conn.execute("DELETE FROM provider_accounts WHERE provider_account_id = 'provider_account:cascade'", [])?;
            let children: i64 = conn.query_row("SELECT (SELECT COUNT(*) FROM agent_runtime_preferences) + (SELECT COUNT(*) FROM auxiliary_model_preferences) + (SELECT COUNT(*) FROM provider_capability_bindings)", [], |row| row.get(0))?;
            assert_eq!(children, 0);
            Ok(())
        })
        .await
        .expect("provider cascades");
}

#[tokio::test]
async fn schema_rejects_noncanonical_timestamp_writes() {
    let store = test_store().await;
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, auth_status, health_status, enabled) VALUES ('mcp_server:time', 'Time', 'stdio', 'none', 'unknown', 0)", [])?;
            assert!(conn.execute("INSERT INTO mcp_tools (mcp_tool_id, mcp_server_id, name, metadata_fingerprint, discovered_at) VALUES ('mcp_tool:time', 'mcp_server:time', 'time', 'fingerprint', '1720000000')", []).is_err());
            conn.execute("INSERT INTO mcp_tools (mcp_tool_id, mcp_server_id, name, metadata_fingerprint, discovered_at) VALUES ('mcp_tool:time', 'mcp_server:time', 'time', 'fingerprint', '2026-07-10T12:00:00.123Z')", [])?;
            assert!(conn.execute("UPDATE mcp_tools SET discovered_at = '2026-07-10T12:00:00Z' WHERE mcp_tool_id = 'mcp_tool:time'", []).is_err());
            Ok(())
        })
        .await
        .expect("forge timestamp");

    assert!(
        store
            .get_mcp_tool("mcp_tool:time")
            .await
            .expect("tool")
            .is_some()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_primary_conversation_creation_converges_across_connections() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let first = NoemaStore::open(&config).await.expect("first connection");
    let second = NoemaStore::open(&config).await.expect("second connection");
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));

    let first_task = tokio::spawn(primary_after_barrier(first.clone(), barrier.clone()));
    let second_task = tokio::spawn(primary_after_barrier(second, barrier.clone()));
    barrier.wait().await;
    let first_id = first_task
        .await
        .expect("first task")
        .expect("first primary");
    let second_id = second_task
        .await
        .expect("second task")
        .expect("second primary");

    assert_eq!(first_id, second_id);
    let count = first
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM conversations WHERE primary_human_id = 'human:local' AND is_primary = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
        .expect("primary count");
    assert_eq!(count, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_active_context_summary_replacement_converges_across_connections() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    let first = NoemaStore::open(&config).await.expect("first connection");
    let conversation = first
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let second = NoemaStore::open(&config).await.expect("second connection");
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let conversation_id = conversation.conversation_id;

    let first_task = tokio::spawn(summary_after_barrier(
        first.clone(),
        barrier.clone(),
        conversation_id.clone(),
        1,
    ));
    let second_task = tokio::spawn(summary_after_barrier(
        second,
        barrier.clone(),
        conversation_id.clone(),
        2,
    ));
    barrier.wait().await;
    first_task
        .await
        .expect("first task")
        .expect("first summary");
    second_task
        .await
        .expect("second task")
        .expect("second summary");

    let active = first
        .latest_active_context_summary(&conversation_id, "codex", None)
        .await
        .expect("active summary")
        .expect("winner");
    assert_eq!(active.covered_item_end_sequence, 2);
    let active_count = first
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM conversation_context_summaries WHERE conversation_id = ?1 AND status = 'active'",
                [&conversation_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
        .expect("active count");
    assert_eq!(active_count, 1);
}

#[tokio::test]
async fn equal_coverage_active_summary_keeps_existing_winner() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let first = store
        .insert_conversation_context_summary(active_summary(&conversation.conversation_id, 5))
        .await
        .expect("first summary");
    let second = store
        .insert_conversation_context_summary(active_summary(&conversation.conversation_id, 5))
        .await
        .expect("equal summary");

    assert_eq!(second.summary_id, first.summary_id);
    assert_eq!(
        store
            .list_context_summaries_for_conversation(&conversation.conversation_id)
            .await
            .expect("summaries")
            .len(),
        1
    );
}

#[tokio::test]
async fn same_end_with_earlier_start_replaces_narrower_active_summary() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let mut narrow = active_summary(&conversation.conversation_id, 5);
    narrow.covered_item_start_sequence = 2;
    let first = store
        .insert_conversation_context_summary(narrow)
        .await
        .expect("narrow summary");
    let replacement = store
        .insert_conversation_context_summary(active_summary(&conversation.conversation_id, 5))
        .await
        .expect("broader summary");

    assert_ne!(replacement.summary_id, first.summary_id);
    assert_eq!(replacement.covered_item_start_sequence, 1);
    let summaries = store
        .list_context_summaries_for_conversation(&conversation.conversation_id)
        .await
        .expect("summaries");
    assert_eq!(summaries.len(), 2);
    assert_eq!(
        summaries
            .iter()
            .filter(|summary| summary.status == crate::ConversationContextSummaryStatus::Active)
            .count(),
        1
    );
}

#[tokio::test]
async fn partially_overlapping_active_summary_is_rejected_as_incomparable() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let first = store
        .insert_conversation_context_summary(active_summary(&conversation.conversation_id, 5))
        .await
        .expect("first summary");
    let mut partial_overlap = active_summary(&conversation.conversation_id, 7);
    partial_overlap.covered_item_start_sequence = 3;

    let error = store
        .insert_conversation_context_summary(partial_overlap)
        .await
        .expect_err("partial overlap must be rejected");

    assert!(matches!(error, StoreError::InvariantViolation { .. }));
    assert_eq!(
        store
            .latest_active_context_summary(&conversation.conversation_id, "codex", None)
            .await
            .expect("active summary")
            .expect("winner")
            .summary_id,
        first.summary_id
    );
}

#[tokio::test]
async fn disjoint_active_summary_is_rejected_as_incomparable() {
    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let first = store
        .insert_conversation_context_summary(active_summary(&conversation.conversation_id, 5))
        .await
        .expect("first summary");
    let mut disjoint = active_summary(&conversation.conversation_id, 9);
    disjoint.covered_item_start_sequence = 7;

    let error = store
        .insert_conversation_context_summary(disjoint)
        .await
        .expect_err("disjoint interval must be rejected");

    assert!(matches!(error, StoreError::InvariantViolation { .. }));
    assert_eq!(
        store
            .latest_active_context_summary(&conversation.conversation_id, "codex", None)
            .await
            .expect("active summary")
            .expect("winner")
            .summary_id,
        first.summary_id
    );
}

#[tokio::test]
async fn composite_relationship_guards_reject_cross_owner_links() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO humans (human_id, display_name) VALUES ('human:other', 'Other')", [])?;
            conn.execute("INSERT INTO conversations (conversation_id, owner_human_id, primary_human_id, is_primary, provider) VALUES ('conversation:local', 'human:local', 'human:local', 1, 'codex')", [])?;
            conn.execute("INSERT INTO conversations (conversation_id, owner_human_id, primary_human_id, is_primary, provider) VALUES ('conversation:other', 'human:other', 'human:other', 1, 'codex')", [])?;
            assert!(conn.execute("UPDATE humans SET primary_conversation_id = 'conversation:other' WHERE human_id = 'human:local'", []).is_err());
            conn.execute("INSERT INTO conversation_turns (turn_id, conversation_id, status) VALUES ('turn:local', 'conversation:local', 'running')", [])?;
            conn.execute("INSERT INTO conversation_turns (turn_id, conversation_id, status) VALUES ('turn:other', 'conversation:other', 'running')", [])?;
            conn.execute("INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_agent_id) VALUES ('item:local', 'conversation:local', 'turn:local', 1, 'assistant_text', 'completed', 'agent:primary')", [])?;
            assert!(conn.execute("INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_agent_id) VALUES ('item:cross', 'conversation:local', 'turn:other', 2, 'assistant_text', 'completed', 'agent:primary')", []).is_err());
            assert!(conn.execute("INSERT INTO artifacts (artifact_id, owner_human_id, title, artifact_kind, storage_kind, created_by_agent_id, source_conversation_id, source_turn_id) VALUES ('artifact:cross', 'human:local', 'Cross', 'document', 'external_url', 'agent:primary', 'conversation:local', 'turn:other')", []).is_err());
            conn.execute("INSERT INTO artifacts (artifact_id, owner_human_id, title, artifact_kind, storage_kind, created_by_agent_id) VALUES ('artifact:one', 'human:local', 'One', 'document', 'external_url', 'agent:primary')", [])?;
            conn.execute("INSERT INTO artifacts (artifact_id, owner_human_id, title, artifact_kind, storage_kind, created_by_agent_id) VALUES ('artifact:two', 'human:local', 'Two', 'document', 'external_url', 'agent:primary')", [])?;
            conn.execute("INSERT INTO artifact_versions (artifact_version_id, artifact_id, version_index, external_url, created_by_agent_id) VALUES ('artifact_version:two', 'artifact:two', 1, 'https://example.com/two', 'agent:primary')", [])?;
            assert!(conn.execute("UPDATE artifacts SET current_version_id = 'artifact_version:two' WHERE artifact_id = 'artifact:one'", []).is_err());
            conn.execute("INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, auth_status, health_status, enabled) VALUES ('mcp_server:a', 'A', 'stdio', 'none', 'unknown', 0), ('mcp_server:b', 'B', 'stdio', 'none', 'unknown', 0)", [])?;
            conn.execute("INSERT INTO mcp_discovery_snapshots (snapshot_id, mcp_server_id, metadata_fingerprint, is_current) VALUES ('snapshot:a', 'mcp_server:a', 'a', 1), ('snapshot:b', 'mcp_server:b', 'b', 1)", [])?;
            assert!(conn.execute("UPDATE mcp_servers SET current_discovery_snapshot_id = 'snapshot:b' WHERE mcp_server_id = 'mcp_server:a'", []).is_err());
            Ok(())
        })
        .await
        .expect("relationship guards");
}

#[tokio::test]
async fn cross_resource_links_and_reverse_mutations_fail_closed() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO humans (human_id, display_name) VALUES ('human:other', 'Other')", [])?;
            conn.execute("INSERT INTO conversations (conversation_id, owner_human_id, primary_human_id, provider) VALUES ('conversation:guard', 'human:local', 'human:local', 'codex')", [])?;
            conn.execute("INSERT INTO conversation_turns (turn_id, conversation_id, status) VALUES ('turn:guard', 'conversation:guard', 'running')", [])?;
            conn.execute("INSERT INTO conversation_items (item_id, conversation_id, turn_id, sequence_index, kind, status, author_agent_id) VALUES ('item:guard', 'conversation:guard', 'turn:guard', 1, 'assistant_text', 'completed', 'agent:primary')", [])?;
            conn.execute("INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, auth_status, health_status, enabled) VALUES ('mcp_server:guard-a', 'A', 'stdio', 'none', 'unknown', 0), ('mcp_server:guard-b', 'B', 'stdio', 'none', 'unknown', 0)", [])?;
            conn.execute("INSERT INTO mcp_tools (mcp_tool_id, mcp_server_id, name, metadata_fingerprint, discovered_at) VALUES ('mcp_tool:guard-a', 'mcp_server:guard-a', 'a', 'a', '2026-07-10T12:00:00.000Z'), ('mcp_tool:guard-b', 'mcp_server:guard-b', 'b', 'b', '2026-07-10T12:00:00.000Z')", [])?;
            conn.execute("INSERT INTO mcp_discovery_snapshots (snapshot_id, mcp_server_id, metadata_fingerprint, is_current) VALUES ('snapshot:guard-a', 'mcp_server:guard-a', 'a', 1), ('snapshot:guard-b', 'mcp_server:guard-b', 'b', 1)", [])?;

            assert!(conn.execute("INSERT INTO mcp_discovery_snapshot_tools (snapshot_id, mcp_tool_id) VALUES ('snapshot:guard-a', 'mcp_tool:guard-b')", []).is_err());
            conn.execute("INSERT INTO mcp_discovery_snapshot_tools (snapshot_id, mcp_tool_id) VALUES ('snapshot:guard-a', 'mcp_tool:guard-a')", [])?;
            assert!(conn.execute("UPDATE mcp_tools SET mcp_server_id = 'mcp_server:guard-b' WHERE mcp_tool_id = 'mcp_tool:guard-a'", []).is_err());
            assert!(conn.execute("UPDATE mcp_discovery_snapshots SET mcp_server_id = 'mcp_server:guard-b' WHERE snapshot_id = 'snapshot:guard-a'", []).is_err());
            conn.execute("UPDATE mcp_servers SET current_discovery_snapshot_id = 'snapshot:guard-a' WHERE mcp_server_id = 'mcp_server:guard-a'", [])?;
            assert!(conn.execute("UPDATE mcp_discovery_snapshots SET is_current = 0 WHERE snapshot_id = 'snapshot:guard-a'", []).is_err());

            assert!(conn.execute("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, mcp_server_id, requester_agent_id, owner_human_id, active_human_id, destination_summary, data_source_summary, source_owner_identity, source_owner_trust, destination_owner_identity, destination_owner_trust, export_summary, status) VALUES ('approval:server-only', 'Server only', 'invocation:server-only', 'mcp_server:guard-a', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'pending')", []).is_err());
            assert!(conn.execute("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, mcp_tool_id, requester_agent_id, owner_human_id, active_human_id, destination_summary, data_source_summary, source_owner_identity, source_owner_trust, destination_owner_identity, destination_owner_trust, export_summary, status) VALUES ('approval:tool-only', 'Tool only', 'invocation:tool-only', 'mcp_tool:guard-a', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'pending')", []).is_err());
            assert!(conn.execute("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, mcp_server_id, mcp_tool_id, requester_agent_id, owner_human_id, active_human_id, destination_summary, data_source_summary, source_owner_identity, source_owner_trust, destination_owner_identity, destination_owner_trust, export_summary, status) VALUES ('approval:cross', 'Cross', 'invocation:cross', 'mcp_server:guard-a', 'mcp_tool:guard-b', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'pending')", []).is_err());
            conn.execute("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, mcp_server_id, mcp_tool_id, requester_agent_id, owner_human_id, active_human_id, destination_summary, data_source_summary, source_owner_identity, source_owner_trust, destination_owner_identity, destination_owner_trust, export_summary, status) VALUES ('approval:guard', 'Guard', 'invocation:guard', 'mcp_server:guard-a', 'mcp_tool:guard-a', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'pending')", [])?;
            assert!(conn.execute("UPDATE approval_requests SET mcp_server_id = 'mcp_server:guard-b' WHERE approval_id = 'approval:guard'", []).is_err());

            assert!(conn.execute("INSERT INTO policy_decisions (policy_decision_id, owner_human_id, conversation_id, decision, reason_code, policy_fingerprint) VALUES ('policy:cross', 'human:other', 'conversation:guard', 'deny', 'cross', 'fingerprint')", []).is_err());
            conn.execute("INSERT INTO policy_decisions (policy_decision_id, owner_human_id, conversation_id, decision, reason_code, policy_fingerprint) VALUES ('policy:guard', 'human:local', 'conversation:guard', 'allow', 'guard', 'fingerprint')", [])?;
            assert!(conn.execute("UPDATE policy_decisions SET owner_human_id = 'human:other' WHERE policy_decision_id = 'policy:guard'", []).is_err());

            assert!(conn.execute("UPDATE conversations SET owner_human_id = 'human:other' WHERE conversation_id = 'conversation:guard'", []).is_err());
            assert!(conn.execute("UPDATE conversation_turns SET conversation_id = 'conversation:missing' WHERE turn_id = 'turn:guard'", []).is_err());
            assert!(conn.execute("UPDATE conversation_items SET conversation_id = 'conversation:missing' WHERE item_id = 'item:guard'", []).is_err());
            conn.execute("DELETE FROM mcp_servers WHERE mcp_server_id = 'mcp_server:guard-a'", [])?;
            let approval_links: (Option<String>, Option<String>) = conn.query_row("SELECT mcp_server_id, mcp_tool_id FROM approval_requests WHERE approval_id = 'approval:guard'", [], |row| Ok((row.get(0)?, row.get(1)?)))?;
            assert_eq!(approval_links, (None, None));
            Ok(())
        })
        .await
        .expect("cross-resource guards");
}

#[tokio::test]
async fn approval_terminal_states_preserve_durable_evidence_without_requiring_actor() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO humans (human_id, display_name) VALUES ('human:decider', 'Decider')", [])?;
            let base = "requester_agent_id, owner_human_id, active_human_id, destination_summary, data_source_summary, source_owner_identity, source_owner_trust, destination_owner_identity, destination_owner_trust, export_summary";
            conn.execute(&format!("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, {base}, status, decided_at) VALUES ('approval:cancelled', 'Cancelled', 'invocation:cancelled', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'cancelled', '2026-07-10T12:00:00.000Z')"), [])?;
            conn.execute(&format!("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, {base}, status, decided_at) VALUES ('approval:expired', 'Expired', 'invocation:expired', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'expired', '2026-07-10T12:00:00.000Z')"), [])?;
            conn.execute(&format!("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, {base}, status, decision_human_id, decided_at) VALUES ('approval:approved', 'Approved', 'invocation:approved', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'approved', 'human:decider', '2026-07-10T12:00:00.000Z')"), [])?;
            conn.execute("DELETE FROM humans WHERE human_id = 'human:decider'", [])?;
            let decision_actor: Option<String> = conn.query_row("SELECT decision_human_id FROM approval_requests WHERE approval_id = 'approval:approved'", [], |row| row.get(0))?;
            assert_eq!(decision_actor, None);
            assert!(conn.execute(&format!("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, {base}, status) VALUES ('approval:no-decision', 'No decision', 'invocation:no-decision', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'expired')"), []).is_err());
            assert!(conn.execute(&format!("INSERT INTO approval_requests (approval_id, action_summary, tool_invocation_id, {base}, status, decided_at) VALUES ('approval:no-consumption', 'No consumption', 'invocation:no-consumption', 'agent:primary', 'human:local', 'human:local', 'Destination', 'Source', 'human:local', 'trusted', 'other', 'untrusted', 'Export', 'consumed', '2026-07-10T12:00:00.000Z')"), []).is_err());
            Ok(())
        })
        .await
        .expect("approval lifecycle");
}

#[tokio::test]
async fn v2_contains_secret_free_policy_approval_snapshot_and_saga_rows() {
    let store = test_store().await;
    for table in [
        "policy_decisions",
        "approval_requests",
        "mcp_discovery_snapshots",
        "mcp_discovery_snapshot_tools",
        "persistence_sagas",
    ] {
        let columns = table_columns(&store, table).await;
        assert!(!columns.is_empty(), "missing {table}");
        assert!(
            columns.iter().all(|column| {
                let column = column.to_ascii_lowercase();
                !column.contains("secret")
                    && !column.contains("token")
                    && !column.contains("payload")
                    && !column.contains("argument")
            }),
            "{table} contains unsafe columns: {columns:?}"
        );
    }
}

#[tokio::test]
async fn null_model_active_summary_and_current_snapshot_are_unique() {
    let store = test_store().await;
    store.ensure_default_actors().await.expect("actors");
    store
        .with_connection(|conn| {
            conn.execute("INSERT INTO conversations (conversation_id, owner_human_id, provider) VALUES ('conversation:unique', 'human:local', 'codex')", [])?;
            conn.execute("INSERT INTO conversation_context_summaries (summary_id, conversation_id, provider_kind, model_profile, summary_text, covered_item_start_sequence, covered_item_end_sequence, input_token_estimate, summary_token_estimate, compaction_provider_kind, status) VALUES ('summary:one', 'conversation:unique', 'codex', NULL, 'one', 1, 1, 1, 1, 'codex', 'active')", [])?;
            assert!(conn.execute("INSERT INTO conversation_context_summaries (summary_id, conversation_id, provider_kind, model_profile, summary_text, covered_item_start_sequence, covered_item_end_sequence, input_token_estimate, summary_token_estimate, compaction_provider_kind, status) VALUES ('summary:empty-model', 'conversation:unique', 'codex', '', 'empty', 1, 1, 1, 1, 'codex', 'pending')", []).is_err());
            let duplicate = conn.execute("INSERT INTO conversation_context_summaries (summary_id, conversation_id, provider_kind, model_profile, summary_text, covered_item_start_sequence, covered_item_end_sequence, input_token_estimate, summary_token_estimate, compaction_provider_kind, status) VALUES ('summary:two', 'conversation:unique', 'codex', NULL, 'two', 1, 2, 1, 1, 'codex', 'active')", []);
            assert!(duplicate.is_err());
            conn.execute("INSERT INTO mcp_servers (mcp_server_id, display_name, transport_kind, auth_status, health_status, enabled) VALUES ('mcp_server:one', 'One', 'stdio', 'none', 'unknown', 0)", [])?;
            conn.execute("INSERT INTO mcp_discovery_snapshots (snapshot_id, mcp_server_id, metadata_fingerprint, is_current) VALUES ('snapshot:one', 'mcp_server:one', 'one', 1)", [])?;
            let duplicate = conn.execute("INSERT INTO mcp_discovery_snapshots (snapshot_id, mcp_server_id, metadata_fingerprint, is_current) VALUES ('snapshot:two', 'mcp_server:one', 'two', 1)", []);
            assert!(duplicate.is_err());
            Ok(())
        })
        .await
        .expect("uniqueness invariants");
}

#[tokio::test]
async fn altered_v2_structure_is_refused_without_mutation() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    NoemaStore::open(&config)
        .await
        .expect("bootstrap v2")
        .close()
        .await
        .expect("close");

    let conn = Connection::open(paths.sqlite_db_path()).expect("open database");
    conn.execute_batch("DROP INDEX conversation_items_conversation_sequence")
        .expect("alter schema");
    drop(conn);
    let before = database_file_set(&paths.sqlite_db_path());

    let error = NoemaStore::open(&config)
        .await
        .expect_err("altered v2 must require reset");

    assert!(matches!(error, StoreError::ResetRequired { .. }));
    assert_eq!(database_file_set(&paths.sqlite_db_path()), before);
}

#[tokio::test]
async fn missing_table_and_trigger_are_refused_without_mutation() {
    assert_schema_mutation_refused("DROP TABLE persistence_sagas").await;
    assert_schema_mutation_refused("DROP TRIGGER schema_state_reject_delete").await;
}

#[tokio::test]
async fn persisted_fingerprint_mismatch_is_refused_without_mutation() {
    assert_schema_mutation_refused(
        "DROP TRIGGER schema_state_reject_update;\
         UPDATE schema_state SET structural_fingerprint = 'mismatch';\
         CREATE TRIGGER schema_state_reject_update BEFORE UPDATE ON schema_state BEGIN SELECT RAISE(ABORT, 'schema_state marker is immutable'); END",
    )
    .await;
}

#[tokio::test]
async fn foreign_key_violation_is_refused_without_mutation() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    NoemaStore::open(&config)
        .await
        .expect("bootstrap")
        .close()
        .await
        .expect("close");
    let conn = Connection::open(paths.sqlite_db_path()).expect("database");
    conn.pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys");
    conn.execute("INSERT INTO conversation_turns (turn_id, conversation_id, status) VALUES ('turn:orphan', 'conversation:missing', 'running')", []).expect("orphan");
    drop(conn);
    let before = database_file_set(&paths.sqlite_db_path());

    assert!(matches!(
        NoemaStore::open(&config).await,
        Err(StoreError::ResetRequired { .. })
    ));
    assert_eq!(database_file_set(&paths.sqlite_db_path()), before);
}

#[tokio::test]
async fn non_wal_database_is_refused_without_mutation() {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    NoemaStore::open(&config)
        .await
        .expect("bootstrap")
        .close()
        .await
        .expect("close");
    let conn = Connection::open(paths.sqlite_db_path()).expect("database");
    conn.pragma_update(None, "journal_mode", "DELETE")
        .expect("delete journal mode");
    drop(conn);
    let before = database_file_set(&paths.sqlite_db_path());

    assert!(matches!(
        NoemaStore::open(&config).await,
        Err(StoreError::ResetRequired { .. })
    ));
    assert_eq!(database_file_set(&paths.sqlite_db_path()), before);
}

async fn schema_marker(store: &NoemaStore) -> (i64, String, String) {
    store
        .with_connection(|conn| {
            conn.query_row(
                "SELECT version, structural_fingerprint, applied_at FROM schema_state WHERE name = 'sqlite_store_v2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(StoreError::Sqlite)
        })
        .await
        .expect("schema marker")
}

fn database_file_set(db_path: &Path) -> Vec<(String, Vec<u8>)> {
    ["", "-wal", "-shm"]
        .into_iter()
        .filter_map(|suffix| {
            let path = std::path::PathBuf::from(format!("{}{}", db_path.display(), suffix));
            path.exists().then(|| {
                (
                    suffix.to_string(),
                    fs::read(path).expect("read database file"),
                )
            })
        })
        .collect()
}

fn sidecar_path(db_path: &Path, suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{}", db_path.display(), suffix))
}

async fn assert_schema_mutation_refused(mutation: &str) {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let config = StoreConfig::from_paths(&paths);
    NoemaStore::open(&config)
        .await
        .expect("bootstrap")
        .close()
        .await
        .expect("close");
    let conn = Connection::open(paths.sqlite_db_path()).expect("database");
    conn.execute_batch(mutation).expect("schema mutation");
    drop(conn);
    let before = database_file_set(&paths.sqlite_db_path());

    assert!(matches!(
        NoemaStore::open(&config).await,
        Err(StoreError::ResetRequired { .. })
    ));
    assert_eq!(database_file_set(&paths.sqlite_db_path()), before);
}

async fn test_store() -> NoemaStore {
    let home = TempDir::new().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.keep()).expect("paths");
    NoemaStore::open(&StoreConfig::from_paths(&paths))
        .await
        .expect("store")
}

async fn table_columns(store: &NoemaStore, table: &str) -> Vec<String> {
    store
        .with_connection(|conn| {
            let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("table columns")
}

async fn primary_after_barrier(
    store: NoemaStore,
    barrier: std::sync::Arc<tokio::sync::Barrier>,
) -> Result<String, StoreError> {
    barrier.wait().await;
    store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .map(|record| record.conversation_id)
}

async fn summary_after_barrier(
    store: NoemaStore,
    barrier: std::sync::Arc<tokio::sync::Barrier>,
    conversation_id: String,
    covered_end: i64,
) -> Result<(), StoreError> {
    barrier.wait().await;
    store
        .insert_conversation_context_summary(active_summary(&conversation_id, covered_end))
        .await
        .map(|_| ())
}

fn active_summary(conversation_id: &str, covered_end: i64) -> crate::NewConversationContextSummary {
    crate::NewConversationContextSummary {
        conversation_id: conversation_id.to_string(),
        provider_kind: "codex".to_string(),
        model_profile: None,
        summary_text: format!("summary {covered_end}"),
        covered_item_start_sequence: 1,
        covered_item_end_sequence: covered_end,
        source_item_ids: Vec::new(),
        input_token_estimate: 1,
        summary_token_estimate: 1,
        compaction_provider_kind: "codex".to_string(),
        compaction_model_profile: None,
        status: crate::ConversationContextSummaryStatus::Active,
        error_code: None,
        error_message: None,
    }
}
