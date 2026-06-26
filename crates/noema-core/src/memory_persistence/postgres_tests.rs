use std::env;

use super::PostgresMemoryRepository;

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
}

fn test_database_url() -> Option<String> {
    env::var(TEST_DATABASE_URL_ENV)
        .ok()
        .filter(|url| !url.trim().is_empty())
}

fn assert_test_database_url(database_url: &str) {
    assert!(
        database_url.to_ascii_lowercase().contains("test"),
        "{TEST_DATABASE_URL_ENV} must name an explicit test database"
    );
}
