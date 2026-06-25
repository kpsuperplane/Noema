use super::*;
use rusqlite::{ToSql, params_from_iter};

fn table_exists(repo: &SqliteMemoryRepository, table_name: &str) -> bool {
    repo.conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .expect("table lookup")
        == 1
}

fn column_names(repo: &SqliteMemoryRepository, table_name: &str) -> Vec<String> {
    let mut stmt = repo
        .conn
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .expect("table info");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .expect("column query");
    rows.map(|row| row.expect("column name")).collect()
}

fn foreign_keys(
    repo: &SqliteMemoryRepository,
    table_name: &str,
) -> Vec<(String, String, String, String)> {
    let mut stmt = repo
        .conn
        .prepare(&format!("PRAGMA foreign_key_list({table_name})"))
        .expect("foreign key info");
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(6)?,
            ))
        })
        .expect("foreign key query");
    rows.map(|row| row.expect("foreign key row")).collect()
}

fn assert_check_constraint_failed(error: rusqlite::Error) {
    assert!(matches!(
        error,
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.contains("CHECK constraint failed")
    ));
}

fn expect_pair_check_violation(repo: &SqliteMemoryRepository, sql: &str, params: &[&dyn ToSql]) {
    let error = repo
        .conn
        .execute(sql, params_from_iter(params.iter().copied()))
        .expect_err("half-populated object ref pair should fail");
    assert_check_constraint_failed(error);
}

#[test]
fn bootstrap_schema_uses_concrete_object_tables() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    for table in [
        "humans",
        "agents",
        "tools",
        "conversations",
        "conversation_turns",
        "conversation_items",
        "memory_items",
        "entities",
        "relationships",
        "object_provenance_edges",
        "object_access_grants",
        "object_events",
        "object_links",
        "context_packets",
        "memory_use_records",
    ] {
        assert!(table_exists(&repo, table), "expected {table}");
    }

    for legacy_table in ["principals", "scopes", "episodes", "messages"] {
        assert!(
            !table_exists(&repo, legacy_table),
            "legacy table {legacy_table} should not be bootstrapped"
        );
    }
}

#[test]
fn conversation_items_have_object_ref_and_redaction_columns() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    let columns = column_names(&repo, "conversation_items");
    for column in [
        "item_id",
        "conversation_id",
        "turn_id",
        "parent_item_id",
        "kind",
        "status",
        "author_object_type",
        "author_object_id",
        "content_text",
        "payload_json",
        "created_at",
        "updated_at",
        "deleted_at",
        "deleted_by_object_type",
        "deleted_by_object_id",
        "redacted_at",
        "redaction_reason",
        "metadata",
    ] {
        assert!(columns.contains(&column.to_string()), "missing {column}");
    }
}

#[test]
fn conversation_turn_trigger_item_fk_targets_conversation_items() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    let foreign_keys = foreign_keys(&repo, "conversation_turns");
    assert!(foreign_keys.iter().any(|(table, from, to, on_delete)| {
        table == "conversation_items"
            && from == "trigger_item_id"
            && to == "item_id"
            && on_delete == "SET NULL"
    }));
}

#[test]
fn schema_rejects_half_populated_optional_object_ref_pairs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    let error = repo
        .conn
        .execute(
            r"
            INSERT INTO conversations (
              conversation_id,
              owner_object_type,
              owner_object_id,
              deleted_by_object_type
            ) VALUES (?1, ?2, ?3, ?4)
            ",
            params!["conv_bad_pair", "human", "human:local", "agent"],
        )
        .expect_err("half-populated deleted_by pair should fail");

    assert_check_constraint_failed(error);
}

#[test]
fn conversation_items_reject_half_populated_deleted_by_pair() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    repo.conn
        .execute(
            r"
            INSERT INTO conversations (
              conversation_id,
              owner_object_type,
              owner_object_id
            ) VALUES (?1, ?2, ?3)
            ",
            params!["conv_items_pair", "human", "human:local"],
        )
        .expect("conversation row");

    expect_pair_check_violation(
        &repo,
        r"
        INSERT INTO conversation_items (
          item_id,
          conversation_id,
          kind,
          author_object_type,
          author_object_id,
          content_text,
          deleted_by_object_type
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        &[
            &"item_bad_pair",
            &"conv_items_pair",
            &"user_text",
            &"human",
            &"human:local",
            &"hello",
            &"agent",
        ],
    );
}

#[test]
fn retrieval_object_links_reject_half_populated_optional_pairs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    repo.conn
        .execute(
            r"
            INSERT INTO memory_items (
              memory_id,
              owner_object_type,
              owner_object_id,
              memory_type,
              title,
              content,
              created_by_object_type,
              created_by_object_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                "mem_links_pair",
                "conversation",
                "conversation:local",
                "fact",
                "title",
                "content",
                "agent",
                "agent:primary"
            ],
        )
        .expect("memory row");

    expect_pair_check_violation(
        &repo,
        r"
        INSERT INTO memory_retrieval_object_links (
          memory_id,
          object_type,
          object_id,
          relation,
          resolver_object_type
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        ",
        &[
            &"mem_links_pair",
            &"conversation",
            &"conversation:local",
            &"active_context",
            &"agent",
        ],
    );
}

#[test]
fn memory_use_records_reject_half_populated_optional_pairs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = NoemaPaths::from_noema_home(dir.path()).expect("paths");
    let repo = SqliteMemoryRepository::open(&paths).expect("repo");

    repo.conn
        .execute(
            r"
            INSERT INTO memory_items (
              memory_id,
              owner_object_type,
              owner_object_id,
              memory_type,
              title,
              content,
              created_by_object_type,
              created_by_object_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                "mem_use_pair",
                "conversation",
                "conversation:local",
                "fact",
                "title",
                "content",
                "agent",
                "agent:primary"
            ],
        )
        .expect("memory row");

    expect_pair_check_violation(
        &repo,
        r"
        INSERT INTO memory_use_records (
          memory_use_id,
          run_id,
          memory_id,
          stage,
          purpose,
          agent_object_type
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        &[
            &"use_bad_pair",
            &"run_1",
            &"mem_use_pair",
            &"retrieved",
            &"answer_human_question",
            &"agent",
        ],
    );
}
