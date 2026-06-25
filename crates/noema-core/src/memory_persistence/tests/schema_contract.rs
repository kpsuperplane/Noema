use super::*;

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
