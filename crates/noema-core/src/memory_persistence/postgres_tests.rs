use std::env;

use super::{
    ConversationItemKind, ConversationItemStatus, NewConversation, NewConversationItem,
    NewConversationTurn, ObjectRef, PostgresMemoryRepository, ReplayMode,
    postgres_schema::POSTGRES_SCHEMA_SQL,
    repository::{POSTGRES_BOOTSTRAP_MIGRATION_NAME, POSTGRES_BOOTSTRAP_MIGRATION_VERSION},
};

const TEST_DATABASE_URL_ENV: &str = "NOEMA_TEST_DATABASE_URL";

#[tokio::test]
async fn bootstrap_creates_core_tables() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let tables = sqlx::query_scalar::<_, String>(
        r"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'public'
          AND table_type = 'BASE TABLE'
        ORDER BY table_name
        ",
    )
    .fetch_all(repo.pool())
    .await
    .expect("list public tables");

    assert_eq!(
        tables,
        [
            "agents",
            "context_packet_memory_edges",
            "context_packet_omissions",
            "context_packets",
            "conversation_items",
            "conversation_turns",
            "conversations",
            "entities",
            "humans",
            "memory_items",
            "memory_participants",
            "memory_retrieval_object_links",
            "memory_retrieval_purpose_rules",
            "memory_subjects",
            "memory_use_records",
            "object_access_grants",
            "object_events",
            "object_links",
            "object_provenance_edges",
            "relationships",
            "schema_migrations",
            "tools",
        ]
    );

    for (table_name, index_name) in [
        (
            "conversation_items",
            "idx_conversation_items_conversation_created_at",
        ),
        ("object_events", "idx_object_events_target_time"),
        ("object_events", "idx_object_events_actor_time"),
        ("object_links", "idx_object_links_source_relation"),
        ("object_links", "idx_object_links_target_relation"),
        ("memory_items", "idx_memory_items_search_vector"),
    ] {
        assert_index_exists(repo.pool(), table_name, index_name).await;
    }

    assert_memory_search_vector_column_exists(repo.pool()).await;

    let migration_name = sqlx::query_scalar::<_, Option<String>>(
        "SELECT name FROM schema_migrations WHERE version = 0",
    )
    .fetch_one(repo.pool())
    .await
    .expect("check bootstrap migration row");
    assert_eq!(migration_name.as_deref(), Some("postgres_bootstrap_v0"));
}

#[tokio::test]
async fn conversation_items_replay_in_created_order() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let conversation = repo
        .create_conversation(NewConversation::local_chat(
            Some("test-model".to_string()),
            Some("/tmp/noema".to_string()),
        ))
        .await
        .expect("conversation");
    let turn = repo
        .create_conversation_turn(NewConversationTurn {
            conversation_id: conversation.conversation_id.clone(),
            trigger_item_id: None,
            metadata: serde_json::json!({}),
        })
        .await
        .expect("turn");

    repo.append_conversation_item(NewConversationItem {
        conversation_id: conversation.conversation_id.clone(),
        turn_id: Some(turn.turn_id.clone()),
        parent_item_id: None,
        kind: ConversationItemKind::UserText,
        status: ConversationItemStatus::Completed,
        author: ObjectRef::human("human:local"),
        content_text: Some("hello".to_string()),
        payload_json: serde_json::json!({}),
        metadata: serde_json::json!({}),
    })
    .await
    .expect("user item");

    let items = repo
        .list_conversation_items(&conversation.conversation_id, ReplayMode::Visible)
        .await
        .expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].content_text.as_deref(), Some("hello"));
}

async fn assert_index_exists(pool: &sqlx::PgPool, table_name: &str, index_name: &str) {
    let has_index = sqlx::query_scalar::<_, bool>(
        r"
        SELECT EXISTS (
          SELECT 1
          FROM pg_indexes
          WHERE schemaname = 'public'
            AND tablename = $1
            AND indexname = $2
        )
        ",
    )
    .bind(table_name)
    .bind(index_name)
    .fetch_one(pool)
    .await
    .expect("check index exists");
    assert!(has_index, "missing index {index_name} on {table_name}");
}

async fn assert_memory_search_vector_column_exists(pool: &sqlx::PgPool) {
    let column = sqlx::query_as::<_, (String, String)>(
        r"
        SELECT is_generated, udt_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'memory_items'
          AND column_name = 'search_vector'
        ",
    )
    .fetch_optional(pool)
    .await
    .expect("check memory_items.search_vector column")
    .expect("missing memory_items.search_vector column");

    assert_eq!(column.0, "ALWAYS");
    assert_eq!(column.1, "tsvector");
}

async fn test_repo() -> Option<PostgresMemoryRepository> {
    let Some(database_url) = test_database_url() else {
        println!("skipping Postgres test: {TEST_DATABASE_URL_ENV} is unset");
        return None;
    };
    assert_test_database_url(&database_url);

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("connect to test Postgres database");
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(&pool)
        .await
        .expect("reset test schema");

    Some(
        PostgresMemoryRepository::from_pool(pool)
            .await
            .expect("bootstrap Postgres memory schema"),
    )
}

fn test_database_url() -> Option<String> {
    env::var(TEST_DATABASE_URL_ENV)
        .ok()
        .filter(|url| !url.trim().is_empty())
}

fn assert_test_database_url(database_url: &str) {
    assert!(
        is_test_database_url(database_url),
        "{TEST_DATABASE_URL_ENV} must name an explicit test database"
    );
}

fn is_test_database_url(database_url: &str) -> bool {
    test_database_name(database_url).is_some_and(is_explicit_test_database_name)
}

fn is_explicit_test_database_name(database_name: &str) -> bool {
    let database_name = database_name.to_ascii_lowercase();
    database_name == "test"
        || database_name.starts_with("test_")
        || database_name.ends_with("_test")
        || database_name.starts_with("noema_test")
}

fn test_database_name(database_url: &str) -> Option<&str> {
    let after_scheme = database_url
        .split_once("://")
        .map_or(database_url, |(_, rest)| rest);
    let path = after_scheme.split_once('/')?.1;
    let name_with_query = path.rsplit('/').next()?.trim();
    let database_name = name_with_query
        .split_once('?')
        .map_or(name_with_query, |(name, _)| name);
    (!database_name.is_empty()).then_some(database_name)
}

#[test]
fn test_database_url_guard_checks_database_name() {
    assert_test_database_url("postgres://noema:noema@localhost:5432/test");
    assert_test_database_url("postgres://noema:noema@localhost:5432/noema_test");
    assert_test_database_url("postgres://noema:noema@localhost:5432/test_noema");
    assert_test_database_url("postgres://noema:noema@localhost:5432/noema_test_local");
    assert_test_database_url("postgres://noema:noema@localhost:5432/noema_test?sslmode=disable");

    for database_url in [
        "postgres://test_user:noema@localhost:5432/noema",
        "postgres://noema:noema@test-host:5432/noema",
        "postgres://noema:noema@localhost:5432/noema?application_name=test",
        "postgres://noema:noema@localhost:5432/contest",
        "postgres://noema:noema@localhost:5432/latest",
        "postgres://noema:noema@localhost:5432/integrationtest",
    ] {
        assert!(
            !is_test_database_url(database_url),
            "accepted unsafe database URL: {database_url}"
        );
    }
}

#[test]
fn bootstrap_migration_constants_are_stable() {
    assert_eq!(POSTGRES_BOOTSTRAP_MIGRATION_VERSION, 0);
    assert_eq!(POSTGRES_BOOTSTRAP_MIGRATION_NAME, "postgres_bootstrap_v0");
}

#[test]
fn postgres_schema_includes_object_event_and_link_tables() {
    for table_name in ["object_events", "object_links"] {
        assert!(
            POSTGRES_SCHEMA_SQL.contains(&format!("CREATE TABLE IF NOT EXISTS {table_name}")),
            "missing table {table_name}"
        );
    }

    for index_name in [
        "idx_object_events_target_time",
        "idx_object_events_actor_time",
        "idx_object_links_source_relation",
        "idx_object_links_target_relation",
    ] {
        assert!(
            POSTGRES_SCHEMA_SQL.contains(&format!("CREATE INDEX IF NOT EXISTS {index_name}")),
            "missing index {index_name}"
        );
    }
}

#[test]
fn postgres_schema_includes_memory_fts_generated_column_and_index() {
    assert!(
        POSTGRES_SCHEMA_SQL.contains("search_vector TSVECTOR GENERATED ALWAYS AS"),
        "missing generated memory search column"
    );
    assert!(
        POSTGRES_SCHEMA_SQL.contains(
            "to_tsvector(\n      'simple',\n      title || ' ' || content || ' ' || coalesce(retrieval_hints::text, '')\n    )"
        ),
        "memory search vector does not index title, content, and retrieval hints"
    );
    assert!(
        POSTGRES_SCHEMA_SQL.contains(
            "CREATE INDEX IF NOT EXISTS idx_memory_items_search_vector ON memory_items USING GIN (search_vector)"
        ),
        "missing memory search vector GIN index"
    );
}
