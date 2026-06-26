use std::env;

use super::{
    PostgresMemoryRepository,
    repository::{POSTGRES_BOOTSTRAP_MIGRATION_NAME, POSTGRES_BOOTSTRAP_MIGRATION_VERSION},
};

const TEST_DATABASE_URL_ENV: &str = "NOEMA_TEST_DATABASE_URL";

#[tokio::test]
async fn bootstrap_creates_core_tables() {
    let Some(database_url) = test_database_url() else {
        println!("skipping Postgres bootstrap test: {TEST_DATABASE_URL_ENV} is unset");
        return;
    };
    assert_test_database_url(&database_url);

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("connect to test Postgres database");
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(&pool)
        .await
        .expect("reset test schema");

    let repo = PostgresMemoryRepository::from_pool(pool)
        .await
        .expect("bootstrap Postgres memory schema");

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
            "object_provenance_edges",
            "relationships",
            "schema_migrations",
            "tools",
        ]
    );

    let has_conversation_items_time_index = sqlx::query_scalar::<_, bool>(
        r"
        SELECT EXISTS (
          SELECT 1
          FROM pg_indexes
          WHERE schemaname = 'public'
            AND tablename = 'conversation_items'
            AND indexname = 'idx_conversation_items_conversation_created_at'
        )
        ",
    )
    .fetch_one(repo.pool())
    .await
    .expect("check conversation items time index");
    assert!(has_conversation_items_time_index);

    let migration_name = sqlx::query_scalar::<_, Option<String>>(
        "SELECT name FROM schema_migrations WHERE version = 0",
    )
    .fetch_one(repo.pool())
    .await
    .expect("check bootstrap migration row");
    assert_eq!(migration_name.as_deref(), Some("postgres_bootstrap_v0"));
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
    test_database_name(database_url)
        .is_some_and(|database_name| database_name.to_ascii_lowercase().contains("test"))
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
    assert_test_database_url("postgres://noema:noema@localhost:5432/noema_test");
    assert_test_database_url("postgres://noema:noema@localhost:5432/test_noema");
    assert_test_database_url("postgres://noema:noema@localhost:5432/noema_test?sslmode=disable");

    for database_url in [
        "postgres://test_user:noema@localhost:5432/noema",
        "postgres://noema:noema@test-host:5432/noema",
        "postgres://noema:noema@localhost:5432/noema?application_name=test",
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
