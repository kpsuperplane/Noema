use std::{env, ops::Deref};

use super::{
    ConversationItemKind, ConversationItemStatus, MemoryType, NewConversation, NewConversationItem,
    NewConversationTurn, NewMemoryCandidate, NewMemoryParticipant, NewMemorySubject,
    NewObjectProvenanceEdge, ObjectRef, ObjectType, PostgresMemoryRepository, ReplayMode,
    postgres_schema::POSTGRES_SCHEMA_SQL,
    provenance::DeleteConversationItem,
    repository::{POSTGRES_BOOTSTRAP_MIGRATION_NAME, POSTGRES_BOOTSTRAP_MIGRATION_VERSION},
};
use crate::memory::{MemoryStatus, ParticipantRole, Sensitivity, SubjectRole};

const TEST_DATABASE_URL_ENV: &str = "NOEMA_TEST_DATABASE_URL";
static POSTGRES_TEST_SCHEMA_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct TestRepo {
    repo: PostgresMemoryRepository,
    _schema_guard: tokio::sync::MutexGuard<'static, ()>,
}

impl Deref for TestRepo {
    type Target = PostgresMemoryRepository;

    fn deref(&self) -> &Self::Target {
        &self.repo
    }
}

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

#[tokio::test]
async fn append_memory_candidate_records_source_conversation_and_edges() {
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
    let source_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::human("human:local"),
            content_text: Some(
                "Remember that Noema Postgres memory writes need provenance.".to_string(),
            ),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source item");

    let mut subject =
        NewMemorySubject::new("human:local", "human", "Local Human", SubjectRole::About);
    subject.linked_object = Some(ObjectRef::human("human:local"));
    subject.aliases = vec!["Local".to_string()];
    subject.metadata = serde_json::json!({"source": "postgres_test"});

    let mut candidate = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("conversation object ref"),
        "Noema Postgres memory writes need provenance.",
        ObjectRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    candidate.participants = vec![NewMemoryParticipant::new(
        ObjectRef::human("human:local"),
        ParticipantRole::Originator,
    )];
    candidate.subjects = vec![subject];

    let summary = repo
        .append_memory_candidate(candidate)
        .await
        .expect("memory candidate");

    assert!(summary.id.starts_with("mem_"));
    assert_eq!(summary.status, MemoryStatus::Confirmed);
    assert_eq!(summary.memory_type, MemoryType::Note);
    assert_eq!(summary.owner_object_type, "conversation");
    assert_eq!(summary.owner_object_id, conversation.conversation_id);
    assert_eq!(
        summary.home_scope_id,
        format!("conversation:{}", summary.owner_object_id)
    );
    assert_eq!(summary.sensitivity, Sensitivity::Normal);
    assert_eq!(
        summary.title,
        "Noema Postgres memory writes need provenance."
    );
    assert_eq!(
        summary.content,
        "Noema Postgres memory writes need provenance."
    );
    assert_eq!(
        summary.source_object_type.as_deref(),
        Some("conversation_item")
    );
    assert_eq!(
        summary.source_object_id.as_deref(),
        Some(source_item.item_id.as_str())
    );
    assert_eq!(summary.source_type, summary.source_object_type);
    assert_eq!(summary.source_id, summary.source_object_id);
    assert_eq!(
        summary.conversation_id.as_deref(),
        Some(summary.owner_object_id.as_str())
    );
    assert!(!summary.created_at.is_empty());

    let edge_count = sqlx::query_scalar::<_, i64>(
        r"
        SELECT COUNT(*)
        FROM object_provenance_edges
        WHERE target_object_type = 'memory_item'
          AND target_object_id = $1
          AND source_object_type = 'conversation_item'
          AND source_object_id = $2
          AND relation = 'derived_from'
        ",
    )
    .bind(summary.id.as_str())
    .bind(source_item.item_id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("source edge count");
    assert_eq!(edge_count, 1);

    let participant_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM memory_participants WHERE memory_id = $1",
    )
    .bind(summary.id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("participant count");
    assert_eq!(participant_count, 1);

    let subject_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM memory_subjects WHERE memory_id = $1")
            .bind(summary.id.as_str())
            .fetch_one(repo.pool())
            .await
            .expect("subject count");
    assert_eq!(subject_count, 1);

    let extra_edge_id = repo
        .add_object_provenance_edge(NewObjectProvenanceEdge {
            target: ObjectRef::new(ObjectType::MemoryItem, summary.id.as_str())
                .expect("memory object ref"),
            source: ObjectRef::new(
                ObjectType::Conversation,
                source_item.conversation_id.as_str(),
            )
            .expect("conversation object ref"),
            relation: "supports".to_string(),
            evidence_excerpt: Some("The conversation contains the source item.".to_string()),
            created_by: ObjectRef::agent("agent:primary"),
            metadata: serde_json::json!({"kind": "test_support"}),
        })
        .await
        .expect("extra provenance edge");
    assert!(extra_edge_id.starts_with("edge_"));
}

#[tokio::test]
async fn deleting_source_item_deletes_sole_provenance_memory() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let source_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::human("human:local"),
            content_text: Some("remember that I prefer early trains".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source item");

    let memory = repo
        .append_memory_candidate(NewMemoryCandidate::confirmed_note(
            ObjectRef::new(
                ObjectType::Conversation,
                conversation.conversation_id.as_str(),
            )
            .expect("conversation object ref"),
            "The user prefers early trains.",
            ObjectRef::agent("agent:primary"),
            ObjectRef::conversation_item(source_item.item_id.as_str()),
        ))
        .await
        .expect("memory");

    let listed = repo
        .list_recent_memories(Some(10))
        .await
        .expect("list memories");
    let listed_memory = listed
        .iter()
        .find(|listed_memory| listed_memory.id == memory.id)
        .expect("memory in list");
    assert_eq!(listed_memory.content, "The user prefers early trains.");
    assert_eq!(
        listed_memory.source_object_type.as_deref(),
        Some("conversation_item")
    );
    assert_eq!(
        listed_memory.source_object_id.as_deref(),
        Some(source_item.item_id.as_str())
    );
    assert_eq!(
        listed_memory.conversation_id.as_deref(),
        Some(conversation.conversation_id.as_str())
    );

    let shown = repo
        .get_memory(&memory.id)
        .await
        .expect("show memory")
        .expect("memory exists");
    assert_eq!(shown.id, memory.id);
    assert_eq!(shown.content, "The user prefers early trains.");
    assert_eq!(
        shown.conversation_id.as_deref(),
        Some(conversation.conversation_id.as_str())
    );

    repo.soft_delete_conversation_item(DeleteConversationItem {
        item_id: source_item.item_id.clone(),
        deleted_by: ObjectRef::human("human:local"),
        reason: Some("user deleted source message".to_string()),
    })
    .await
    .expect("delete source item");

    let deleted = repo
        .get_memory(&memory.id)
        .await
        .expect("memory lookup")
        .expect("memory exists");
    assert_eq!(deleted.status, MemoryStatus::Deleted);
    assert_eq!(deleted.title, "[redacted]");
    assert_eq!(deleted.content, "[redacted]");

    let stored_item = sqlx::query_as::<_, (Option<String>, serde_json::Value, Option<String>)>(
        r"
        SELECT content_text, payload_json, redaction_reason
        FROM conversation_items
        WHERE item_id = $1
        ",
    )
    .bind(source_item.item_id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("stored source item");
    assert_eq!(stored_item.0.as_deref(), Some("[redacted]"));
    assert_eq!(stored_item.1, serde_json::json!({}));
    assert_eq!(
        stored_item.2.as_deref(),
        Some("user deleted source message")
    );

    let evidence_excerpt = sqlx::query_scalar::<_, Option<String>>(
        r"
        SELECT evidence_excerpt
        FROM object_provenance_edges
        WHERE target_object_type = 'memory_item'
          AND target_object_id = $1
        ",
    )
    .bind(memory.id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("redacted edge excerpt");
    assert_eq!(evidence_excerpt.as_deref(), Some("[redacted]"));
}

#[tokio::test]
async fn list_redacts_sensitive_and_secret_postgres_memories() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let source_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ObjectRef::human("human:local"),
            content_text: Some("sensitive memory source".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source item");

    let mut sensitive = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("conversation object ref"),
        "Sensitive medical detail",
        ObjectRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    sensitive.title = Some("Medical detail".to_string());
    sensitive.sensitivity = Sensitivity::Sensitive;
    let sensitive_memory = repo
        .append_memory_candidate(sensitive)
        .await
        .expect("sensitive memory");

    let mut secret = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("conversation object ref"),
        "Secret credential-like detail",
        ObjectRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    secret.title = Some("Credential detail".to_string());
    secret.sensitivity = Sensitivity::Secret;
    repo.append_memory_candidate(secret)
        .await
        .expect("secret memory");

    let memories = repo
        .list_recent_memories(Some(10))
        .await
        .expect("list memories");
    assert_eq!(memories.len(), 2);
    assert!(memories.iter().all(|memory| memory.title == "[redacted]"));
    assert!(memories.iter().all(|memory| memory.content == "[redacted]"));

    let shown = repo
        .get_memory(&sensitive_memory.id)
        .await
        .expect("show memory")
        .expect("memory exists");
    assert_eq!(shown.title, "Medical detail");
    assert_eq!(shown.content, "Sensitive medical detail");
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

async fn test_repo() -> Option<TestRepo> {
    let Some(database_url) = test_database_url() else {
        println!("skipping Postgres test: {TEST_DATABASE_URL_ENV} is unset");
        return None;
    };
    assert_test_database_url(&database_url);

    let schema_guard = POSTGRES_TEST_SCHEMA_LOCK.lock().await;
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
    Some(TestRepo {
        repo,
        _schema_guard: schema_guard,
    })
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
