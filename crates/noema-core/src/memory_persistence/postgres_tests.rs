use std::{env, ops::Deref};

use super::{
    ActorId, ActorRef, ConversationId, ConversationItemKind, ConversationItemStatus, MemoryItemId,
    MemoryPersistenceError, MemoryType, NewConversation, NewConversationItem, NewConversationTurn,
    NewMemoryCandidate, NewMemoryParticipant, NewMemorySubject, NewObjectProvenanceEdge, ObjectId,
    ObjectProvenanceSource, ObjectRef, ObjectType, PostgresMemoryRepository, ReplayMode,
    postgres_schema::POSTGRES_SCHEMA_SQL,
    provenance::DeleteConversationItem,
    queries::POSTGRES_MEMORY_SUMMARY_BY_DEDUPE_FINGERPRINT_SQL,
    repository::{POSTGRES_BOOTSTRAP_MIGRATION_NAME, POSTGRES_BOOTSTRAP_MIGRATION_VERSION},
};
use crate::memory::{
    AgentVisibleOmission, AuditDenial, DenialReason, EligibilityReason, MemoryRetrievalRequest,
    MemoryRetrievalResult, MemoryUseRecord, MemoryUseStage, Purpose, RankReason, RetrievedMemory,
    TrustedRetrievalContext, UntrustedHints,
};
use crate::memory::{MemoryStatus, ParticipantRole, Sensitivity, SubjectRole};

const TEST_DATABASE_URL_ENV: &str = "NOEMA_TEST_DATABASE_URL";
static POSTGRES_TEST_SCHEMA_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[test]
fn persistence_ids_reject_empty_values() {
    assert!(ActorId::try_from("").is_err());
    assert!(ObjectId::try_from("   ").is_err());
    assert!(MemoryItemId::try_from("").is_err());
    assert!(ConversationId::try_from("").is_err());
}

#[test]
fn persistence_ids_display_inner_value() {
    let actor_id = ActorId::try_from("agent:primary").expect("valid actor id");
    let object_id = ObjectId::try_from("conversation:abc").expect("valid object id");

    assert_eq!(actor_id.as_str(), "agent:primary");
    assert_eq!(object_id.to_string(), "conversation:abc");
}

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
            "actors",
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
            "provider_accounts",
            "relationships",
            "schema_migrations",
            "tools",
        ]
    );

    for (table_name, index_name) in [
        ("actors", "idx_actors_kind_active"),
        ("actors", "idx_actors_handle"),
        (
            "conversation_items",
            "idx_conversation_items_conversation_created_at",
        ),
        ("object_events", "idx_object_events_target_time"),
        ("object_events", "idx_object_events_actor_time"),
        ("object_links", "idx_object_links_source_relation"),
        ("object_links", "idx_object_links_target_relation"),
        ("memory_items", "idx_memory_items_search_vector"),
        ("memory_items", "idx_memory_items_live_dedupe_fingerprint"),
    ] {
        assert_index_exists(repo.pool(), table_name, index_name).await;
    }

    assert_memory_dedupe_fingerprint_column_exists(repo.pool()).await;
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
async fn actors_enforce_concrete_profile_kind() {
    let Some(repo) = test_repo().await else {
        return;
    };

    sqlx::query(
        r"
        INSERT INTO actors (actor_id, actor_kind, display_name)
        VALUES ('actor:test-human', 'human', 'Test Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("insert human actor");

    sqlx::query(
        r"
        INSERT INTO humans (human_id, actor_id, display_name)
        VALUES ('human:test', 'actor:test-human', 'Test Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("human profile may point at human actor");

    let invalid_agent_profile = sqlx::query(
        r"
        INSERT INTO agents (agent_id, actor_id, display_name)
        VALUES ('agent:wrong-kind', 'actor:test-human', 'Wrong Kind')
        ",
    )
    .execute(repo.pool())
    .await;

    assert_sqlstate(invalid_agent_profile, "23503");
}

#[tokio::test]
async fn actors_cannot_be_reused_by_two_human_profiles() {
    let Some(repo) = test_repo().await else {
        return;
    };

    sqlx::query(
        r"
        INSERT INTO actors (actor_id, actor_kind, display_name)
        VALUES ('actor:shared-human', 'human', 'Shared Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("insert actor");

    sqlx::query(
        r"
        INSERT INTO humans (human_id, actor_id, display_name)
        VALUES ('human:first', 'actor:shared-human', 'First Human')
        ",
    )
    .execute(repo.pool())
    .await
    .expect("first profile may use actor");

    let duplicate_actor = sqlx::query(
        r"
        INSERT INTO humans (human_id, actor_id, display_name)
        VALUES ('human:second', 'actor:shared-human', 'Second Human')
        ",
    )
    .execute(repo.pool())
    .await;

    assert_sqlstate(duplicate_actor, "23505");
}

#[tokio::test]
async fn postgres_bootstrap_creates_provider_accounts_table() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let columns = sqlx::query_scalar::<_, String>(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'provider_accounts'
        ORDER BY ordinal_position
        "#,
    )
    .fetch_all(repo.pool())
    .await
    .expect("provider account columns");

    assert_eq!(
        columns,
        [
            "provider_account_id",
            "provider_kind",
            "account_key",
            "display_name",
            "auth_method",
            "is_active",
            "is_default",
            "status",
            "last_checked_at",
            "last_authenticated_at",
            "last_error_code",
            "last_error_message",
            "metadata",
            "created_at",
            "updated_at",
            "deleted_at",
        ]
    );

    assert_index_exists(
        repo.pool(),
        "provider_accounts",
        "idx_provider_accounts_active_default",
    )
    .await;
    assert_index_exists(
        repo.pool(),
        "provider_accounts",
        "idx_provider_accounts_one_active_default_per_provider",
    )
    .await;
}

#[tokio::test]
async fn postgres_bootstrap_rejects_invalid_provider_account_enums() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let invalid_auth = sqlx::query(
        r#"
        INSERT INTO provider_accounts (
          provider_account_id, provider_kind, account_key, display_name,
          auth_method, status
        )
        VALUES (
          'provider_account:bad:auth', 'bad', 'auth', 'Bad Auth',
          'browser_cookie', 'unknown'
        )
        "#,
    )
    .execute(repo.pool())
    .await;
    assert_sqlstate(invalid_auth, "23514");

    let invalid_status = sqlx::query(
        r#"
        INSERT INTO provider_accounts (
          provider_account_id, provider_kind, account_key, display_name,
          auth_method, status
        )
        VALUES (
          'provider_account:bad:status', 'bad', 'status', 'Bad Status',
          'none', 'logged_in'
        )
        "#,
    )
    .execute(repo.pool())
    .await;
    assert_sqlstate(invalid_status, "23514");
}

#[tokio::test]
async fn provider_accounts_rejects_second_active_default_for_provider() {
    let Some(repo) = test_repo().await else {
        return;
    };

    sqlx::query(
        r#"
        INSERT INTO provider_accounts (
          provider_account_id, provider_kind, account_key, display_name,
          auth_method, is_active, is_default, status
        )
        VALUES (
          'provider_account:codex:default', 'codex', 'default', 'Codex',
          'oauth_device_code', true, true, 'unknown'
        )
        "#,
    )
    .execute(repo.pool())
    .await
    .expect("first active default");

    let duplicate = sqlx::query(
        r#"
        INSERT INTO provider_accounts (
          provider_account_id, provider_kind, account_key, display_name,
          auth_method, is_active, is_default, status
        )
        VALUES (
          'provider_account:codex:secondary', 'codex', 'secondary', 'Codex Secondary',
          'oauth_device_code', true, true, 'unknown'
        )
        "#,
    )
    .execute(repo.pool())
    .await;

    assert_sqlstate(duplicate, "23505");
}

#[tokio::test]
async fn ensure_default_provider_account_creates_codex_default() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let account = repo
        .ensure_default_provider_account()
        .await
        .expect("default provider account");

    assert_eq!(
        account.provider_account_id,
        "provider_account:codex:default"
    );
    assert_eq!(account.provider_kind, "codex");
    assert_eq!(account.account_key, "default");
    assert_eq!(account.display_name, "Codex");
    assert_eq!(
        account.auth_method,
        crate::ProviderAuthMethod::OauthDeviceCode
    );
    assert!(account.is_active);
    assert!(account.is_default);
    assert_eq!(account.status, crate::ProviderAccountStatus::Unknown);
}

#[tokio::test]
async fn ensure_default_provider_account_restores_soft_deleted_default() {
    let Some(repo) = test_repo().await else {
        return;
    };

    sqlx::query(
        r#"
        INSERT INTO provider_accounts (
          provider_account_id, provider_kind, account_key, display_name,
          auth_method, is_active, is_default, status, deleted_at
        )
        VALUES (
          'provider_account:codex:default', 'codex', 'default', 'Old Codex',
          'oauth_device_code', false, false, 'unauthenticated', now()
        )
        "#,
    )
    .execute(repo.pool())
    .await
    .expect("soft deleted default provider account");

    let account = repo
        .ensure_default_provider_account()
        .await
        .expect("restored default provider account");

    assert_eq!(
        account.provider_account_id,
        "provider_account:codex:default"
    );
    assert!(account.is_active);
    assert!(account.is_default);
    assert_eq!(account.display_name, "Codex");
    let deleted_at: Option<String> = sqlx::query_scalar(
        "SELECT deleted_at::text FROM provider_accounts WHERE provider_account_id = $1",
    )
    .bind(account.provider_account_id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("deleted_at");
    assert_eq!(deleted_at, None);
}

#[tokio::test]
async fn active_provider_account_returns_default_account() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_provider_account()
        .await
        .expect("default provider account");

    let account = repo
        .active_provider_account("codex")
        .await
        .expect("active provider account")
        .expect("account exists");

    assert_eq!(account.provider_kind, "codex");
    assert_eq!(account.account_key, "default");
}

#[tokio::test]
async fn active_provider_account_filters_by_provider_kind() {
    let Some(repo) = test_repo().await else {
        return;
    };
    sqlx::query(
        r#"
        INSERT INTO provider_accounts (
          provider_account_id, provider_kind, account_key, display_name,
          auth_method, is_active, is_default, status
        )
        VALUES (
          'provider_account:other:default', 'other', 'default', 'Other',
          'none', true, true, 'authenticated'
        )
        "#,
    )
    .execute(repo.pool())
    .await
    .expect("other default provider account");
    repo.ensure_default_provider_account()
        .await
        .expect("default provider account");

    let account = repo
        .active_provider_account("codex")
        .await
        .expect("active provider account")
        .expect("account exists");
    let missing = repo
        .active_provider_account("missing")
        .await
        .expect("missing provider account");

    assert_eq!(account.provider_kind, "codex");
    assert_eq!(account.account_key, "default");
    assert_eq!(missing, None);
}

#[tokio::test]
async fn update_provider_account_status_records_auth_metadata() {
    let Some(repo) = test_repo().await else {
        return;
    };
    let account = repo
        .ensure_default_provider_account()
        .await
        .expect("default provider account");

    repo.update_provider_account_status(
        account.provider_account_id.as_str(),
        crate::ProviderAccountStatus::Authenticated,
        None,
        None,
    )
    .await
    .expect("status update");

    let stored = repo
        .active_provider_account("codex")
        .await
        .expect("active provider account")
        .expect("account exists");
    assert_eq!(stored.status, crate::ProviderAccountStatus::Authenticated);
    assert!(stored.last_checked_at.is_some());
    assert!(stored.last_authenticated_at.is_some());
    assert_eq!(stored.last_error_code, None);
    assert_eq!(stored.last_error_message, None);
}

#[tokio::test]
async fn update_provider_account_status_rejects_missing_or_deleted_account() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let missing = repo
        .update_provider_account_status(
            "provider_account:codex:missing",
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect_err("missing account should fail");
    assert!(matches!(
        missing,
        MemoryPersistenceError::ProviderAccountNotFound {
            provider_account_id
        } if provider_account_id == "provider_account:codex:missing"
    ));

    let account = repo
        .ensure_default_provider_account()
        .await
        .expect("default provider account");
    sqlx::query("UPDATE provider_accounts SET deleted_at = now() WHERE provider_account_id = $1")
        .bind(account.provider_account_id.as_str())
        .execute(repo.pool())
        .await
        .expect("soft delete provider account");

    let deleted = repo
        .update_provider_account_status(
            account.provider_account_id.as_str(),
            crate::ProviderAccountStatus::Authenticated,
            None,
            None,
        )
        .await
        .expect_err("deleted account should fail");
    assert!(matches!(
        deleted,
        MemoryPersistenceError::ProviderAccountNotFound {
            provider_account_id
        } if provider_account_id == "provider_account:codex:default"
    ));
}

#[tokio::test]
async fn postgres_bootstrap_adds_primary_conversation_to_humans() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
          SELECT 1
          FROM information_schema.columns
          WHERE table_schema = 'public'
            AND table_name = 'humans'
            AND column_name = 'primary_conversation_id'
        )
        "#,
    )
    .fetch_one(repo.pool())
    .await
    .expect("primary conversation column exists query");

    assert!(exists);
}

#[tokio::test]
async fn postgres_bootstrap_removes_provider_thread_id_from_conversations() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
          SELECT 1
          FROM information_schema.columns
          WHERE table_schema = 'public'
            AND table_name = 'conversations'
            AND column_name = 'provider_thread_id'
        )
        "#,
    )
    .fetch_one(repo.pool())
    .await
    .expect("provider thread column exists query");

    assert!(!exists);
}

#[tokio::test]
async fn postgres_bootstrap_primary_conversation_fk_sets_null_on_delete() {
    let Some(repo) = test_repo().await else {
        return;
    };

    let action: String = sqlx::query_scalar(
        r#"
        SELECT rc.delete_rule
        FROM information_schema.referential_constraints rc
        JOIN information_schema.table_constraints tc
          ON rc.constraint_catalog = tc.constraint_catalog
         AND rc.constraint_schema = tc.constraint_schema
         AND rc.constraint_name = tc.constraint_name
        WHERE tc.table_schema = 'public'
          AND tc.table_name = 'humans'
          AND tc.constraint_name = 'fk_humans_primary_conversation'
        "#,
    )
    .fetch_one(repo.pool())
    .await
    .expect("primary conversation fk delete rule");

    assert_eq!(action, "SET NULL");
}

#[tokio::test]
async fn primary_conversation_is_created_and_reused_for_local_human() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let first = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("first primary conversation");
    let second = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("second primary conversation");

    assert_eq!(first.conversation_id, second.conversation_id);

    let stored: Option<String> =
        sqlx::query_scalar("SELECT primary_conversation_id FROM humans WHERE human_id = $1")
            .bind("human:local")
            .fetch_one(repo.pool())
            .await
            .expect("stored primary conversation");
    assert_eq!(stored.as_deref(), Some(first.conversation_id.as_str()));
}

#[tokio::test]
async fn primary_conversation_replaces_deleted_assignment() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let first = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("first primary conversation");

    sqlx::query("UPDATE conversations SET lifecycle_status = 'deleted' WHERE conversation_id = $1")
        .bind(first.conversation_id.as_str())
        .execute(repo.pool())
        .await
        .expect("mark conversation deleted");

    let replacement = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("replacement primary conversation");

    assert_ne!(first.conversation_id, replacement.conversation_id);
}

#[tokio::test]
async fn primary_conversation_replaces_inaccessible_assignment() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    sqlx::query(
        r#"
        INSERT INTO actors (actor_id, actor_kind, display_name, handle)
        VALUES ('human:other', 'human', 'Other human', 'other')
        "#,
    )
    .execute(repo.pool())
    .await
    .expect("insert other human actor");
    sqlx::query(
        r#"
        INSERT INTO humans (human_id, actor_id, display_name, handle)
        VALUES ('human:other', 'human:other', 'Other human', 'other')
        "#,
    )
    .execute(repo.pool())
    .await
    .expect("insert other human");
    sqlx::query(
        r#"
        INSERT INTO conversations (
          conversation_id, title, owner_object_type, owner_object_id,
          primary_human_id, primary_agent_id, provider, model, cwd, metadata
        )
        VALUES (
          'conversation_other', 'Other home', 'human', 'human:other',
          'human:other', 'agent:primary', 'codex', NULL, NULL, '{}'::jsonb
        )
        "#,
    )
    .execute(repo.pool())
    .await
    .expect("insert other conversation");
    sqlx::query(
        "UPDATE humans SET primary_conversation_id = 'conversation_other' WHERE human_id = $1",
    )
    .bind("human:local")
    .execute(repo.pool())
    .await
    .expect("point local human at inaccessible conversation");

    let replacement = repo
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("replacement primary conversation");

    assert_ne!(replacement.conversation_id, "conversation_other");
    let stored: Option<String> =
        sqlx::query_scalar("SELECT primary_conversation_id FROM humans WHERE human_id = $1")
            .bind("human:local")
            .fetch_one(repo.pool())
            .await
            .expect("stored primary conversation");
    assert_eq!(
        stored.as_deref(),
        Some(replacement.conversation_id.as_str())
    );
}

#[tokio::test]
async fn primary_conversation_concurrent_startup_creates_one_home_thread() {
    let Some(repo_guard) = test_repo().await else {
        return;
    };
    repo_guard.ensure_default_actors().await.expect("actors");
    let repo = repo_guard.repo.clone();

    let mut tasks = Vec::new();
    for _ in 0..8 {
        let repo = repo.clone();
        tasks.push(tokio::spawn(async move {
            repo.get_or_create_primary_conversation("human:local", None, None)
                .await
                .expect("primary conversation")
                .conversation_id
        }));
    }

    let mut ids = Vec::new();
    for task in tasks {
        ids.push(task.await.expect("join primary conversation task"));
    }
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 1);

    let home_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversations WHERE title = 'Home' AND primary_human_id = $1",
    )
    .bind("human:local")
    .fetch_one(repo_guard.pool())
    .await
    .expect("home conversation count");
    assert_eq!(home_count, 1);
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
        author: ActorRef::human("human:local"),
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
            author: ActorRef::human("human:local"),
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
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    candidate.participants = vec![NewMemoryParticipant::new(
        ActorRef::human("human:local"),
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
            created_by: ActorRef::agent("agent:primary"),
            metadata: serde_json::json!({"kind": "test_support"}),
        })
        .await
        .expect("extra provenance edge");
    assert!(extra_edge_id.starts_with("edge_"));
}

#[tokio::test]
async fn append_memory_candidate_reuses_exact_dedupe_fingerprint() {
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
    let first_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I like ice cream.".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("first item");
    let second_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some(turn.turn_id.clone()),
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I LIKE   ICE CREAM".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("second item");

    let mut first = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("owner"),
        "I like ice cream.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(first_item.item_id.as_str()),
    );
    first.memory_type = MemoryType::Preference;
    first.participants = vec![NewMemoryParticipant::new(
        ActorRef::human("human:local"),
        ParticipantRole::HumanInScope,
    )];
    let mut subject =
        NewMemorySubject::new("human:local", "human", "Local human", SubjectRole::About);
    subject.linked_object = Some(ObjectRef::human("human:local"));
    first.subjects = vec![subject.clone()];

    let mut second = first.clone();
    second.content = "I LIKE   ICE CREAM".to_string();
    second.source = Some(ObjectProvenanceSource {
        source: ObjectRef::conversation_item(second_item.item_id.as_str()),
        evidence_excerpt: Some("I LIKE   ICE CREAM".to_string()),
    });

    let first_summary = repo
        .append_memory_candidate(first)
        .await
        .expect("first memory");
    let second_summary = repo
        .append_memory_candidate(second)
        .await
        .expect("second memory");

    assert_eq!(first_summary.id, second_summary.id);
    assert_eq!(
        first_summary.dedupe_fingerprint,
        second_summary.dedupe_fingerprint
    );
    let dedupe_fingerprint = first_summary
        .dedupe_fingerprint
        .as_deref()
        .expect("dedupe fingerprint");

    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);

    let edge_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM object_provenance_edges
        WHERE target_object_type = 'memory_item'
          AND target_object_id = $1
          AND source_object_type = 'conversation_item'
        "#,
    )
    .bind(first_summary.id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("edge count");
    assert_eq!(edge_count, 2);

    let reused_event_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM object_events
        WHERE target_object_type = 'memory_item'
          AND target_object_id = $1
          AND event_type = 'memory_reused'
          AND reason = 'exact_dedupe_fingerprint'
          AND details->>'dedupe_fingerprint' = $2
        "#,
    )
    .bind(first_summary.id.as_str())
    .bind(dedupe_fingerprint)
    .fetch_one(repo.pool())
    .await
    .expect("reused event count");
    assert_eq!(reused_event_count, 1);
}

#[tokio::test]
async fn append_memory_candidate_reuses_inferred_dedupe_fingerprint() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let first_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I prefer window seats.".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("first item");
    let second_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I PREFER WINDOW SEATS".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("second item");

    let mut first = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("owner"),
        "I prefer window seats.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(first_item.item_id.as_str()),
    );
    first.memory_type = MemoryType::Preference;
    first.status = MemoryStatus::Inferred;

    let mut second = first.clone();
    second.content = "I PREFER WINDOW SEATS".to_string();
    second.status = MemoryStatus::Confirmed;
    second.source = Some(ObjectProvenanceSource {
        source: ObjectRef::conversation_item(second_item.item_id.as_str()),
        evidence_excerpt: Some("I PREFER WINDOW SEATS".to_string()),
    });

    let first_summary = repo
        .append_memory_candidate(first)
        .await
        .expect("first memory");
    let second_summary = repo
        .append_memory_candidate(second)
        .await
        .expect("second memory");

    assert_eq!(first_summary.status, MemoryStatus::Inferred);
    assert_eq!(first_summary.id, second_summary.id);
    assert_eq!(
        first_summary.dedupe_fingerprint,
        second_summary.dedupe_fingerprint
    );

    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
}

#[tokio::test]
async fn append_memory_candidate_does_not_reuse_archived_dedupe_fingerprint() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let first_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I prefer aisle seats.".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("first item");
    let second_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I PREFER AISLE SEATS".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("second item");

    let mut archived = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("owner"),
        "I prefer aisle seats.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(first_item.item_id.as_str()),
    );
    archived.memory_type = MemoryType::Preference;
    archived.status = MemoryStatus::Archived;

    let mut current = archived.clone();
    current.content = "I PREFER AISLE SEATS".to_string();
    current.status = MemoryStatus::Confirmed;
    current.source = Some(ObjectProvenanceSource {
        source: ObjectRef::conversation_item(second_item.item_id.as_str()),
        evidence_excerpt: Some("I PREFER AISLE SEATS".to_string()),
    });

    let archived_summary = repo
        .append_memory_candidate(archived)
        .await
        .expect("archived memory");
    let current_summary = repo
        .append_memory_candidate(current)
        .await
        .expect("current memory");

    assert_ne!(archived_summary.id, current_summary.id);
    assert_eq!(
        archived_summary.dedupe_fingerprint,
        current_summary.dedupe_fingerprint
    );

    let matching_count = sqlx::query_scalar::<_, i64>(
        r"
        SELECT COUNT(*)
        FROM memory_items
        WHERE memory_dedupe_fingerprint = $1
        ",
    )
    .bind(
        current_summary
            .dedupe_fingerprint
            .as_deref()
            .expect("dedupe fingerprint"),
    )
    .fetch_one(repo.pool())
    .await
    .expect("matching memory count");
    assert_eq!(matching_count, 2);
}

#[tokio::test]
async fn append_memory_candidate_concurrent_exact_appends_reuse_fingerprint() {
    let Some(repo) = test_repo().await else {
        return;
    };
    repo.ensure_default_actors().await.expect("actors");

    let conversation = repo
        .create_conversation(NewConversation::local_chat(None, None))
        .await
        .expect("conversation");
    let first_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I like sleeper trains.".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("first item");
    let second_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("I LIKE SLEEPER TRAINS".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("second item");

    let mut first = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("owner"),
        "I like sleeper trains.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(first_item.item_id.as_str()),
    );
    first.memory_type = MemoryType::Preference;
    let mut second = first.clone();
    second.content = "I LIKE SLEEPER TRAINS".to_string();
    second.source = Some(ObjectProvenanceSource {
        source: ObjectRef::conversation_item(second_item.item_id.as_str()),
        evidence_excerpt: Some("I LIKE SLEEPER TRAINS".to_string()),
    });

    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let first_repo = repo.repo.clone();
    let second_repo = repo.repo.clone();
    let first_barrier = barrier.clone();
    let second_barrier = barrier.clone();

    let first_task = tokio::spawn(async move {
        first_barrier.wait().await;
        first_repo.append_memory_candidate(first).await
    });
    let second_task = tokio::spawn(async move {
        second_barrier.wait().await;
        second_repo.append_memory_candidate(second).await
    });

    let first_summary = first_task.await.expect("first task").expect("first memory");
    let second_summary = second_task
        .await
        .expect("second task")
        .expect("second memory");

    assert_eq!(first_summary.id, second_summary.id);
    assert_eq!(
        first_summary.dedupe_fingerprint,
        second_summary.dedupe_fingerprint
    );

    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
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
            author: ActorRef::human("human:local"),
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
            ActorRef::agent("agent:primary"),
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
        deleted_by: ActorRef::human("human:local"),
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
            author: ActorRef::human("human:local"),
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
        ActorRef::agent("agent:primary"),
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
        ActorRef::agent("agent:primary"),
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

#[tokio::test]
async fn record_context_packet_writes_postgres_manifest_edges_omissions_and_use_records() {
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
            author: ActorRef::human("human:local"),
            content_text: Some("context packet postgres source".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source item");

    let mut included = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("conversation object ref"),
        "Noema should record Postgres context packet manifests.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    included.status = MemoryStatus::Active;
    included.participants = vec![NewMemoryParticipant::new(
        ActorRef::human("human:local"),
        ParticipantRole::HumanInScope,
    )];
    let included_memory = repo
        .append_memory_candidate(included)
        .await
        .expect("included memory");

    let mut denied = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("conversation object ref"),
        "Sensitive context packet detail should remain audit-only.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    denied.status = MemoryStatus::Active;
    denied.sensitivity = Sensitivity::Sensitive;
    let denied_memory = repo
        .append_memory_candidate(denied)
        .await
        .expect("denied memory");

    let mut trusted =
        TrustedRetrievalContext::for_human("human:local", Purpose::AnswerHumanQuestion);
    trusted.active_agent_ids = vec!["agent:primary".to_string()];
    trusted.active_scopes = vec![format!("conversation:{}", conversation.conversation_id)];
    trusted.explicit_memory_request = true;
    trusted.sensitivity_ceiling = Sensitivity::Normal;
    let request = MemoryRetrievalRequest {
        requesting_principal_id: "agent:primary".to_string(),
        trusted,
        untrusted_hints: UntrustedHints::default(),
    };
    let result = MemoryRetrievalResult {
        included: vec![RetrievedMemory {
            memory_id: included_memory.id.clone(),
            rank_score: 42,
            eligibility_reason: EligibilityReason::ActiveScope,
            rank_reasons: vec![
                RankReason::ExplicitMemoryRequest,
                RankReason::SameHumanParticipant,
            ],
        }],
        denied_for_audit: vec![AuditDenial {
            memory_id: Some(denied_memory.id.clone()),
            relationship_id: None,
            reason: DenialReason::SensitivityCeiling,
        }],
        agent_visible_omissions: vec![AgentVisibleOmission {
            reason: "policy_restricted_context",
        }],
        use_records: vec![MemoryUseRecord {
            memory_id: included_memory.id.clone(),
            stage: MemoryUseStage::Retrieved,
        }],
    };

    repo.record_context_packet("ctx_pg_packet", "run:pg-packet", &request, &result)
        .await
        .expect("record context packet");

    let packet = sqlx::query_as::<_, (String, String, serde_json::Value, serde_json::Value)>(
        r"
        SELECT
          requesting_actor_id,
          purpose,
          active_objects,
          agent_visible_omissions
        FROM context_packets
        WHERE context_packet_id = $1
        ",
    )
    .bind("ctx_pg_packet")
    .fetch_one(repo.pool())
    .await
    .expect("packet row");
    assert_eq!(packet.0, "agent:primary");
    assert_eq!(packet.1, "answer_human_question");
    assert_eq!(
        packet.2,
        serde_json::json!([
            {
                "object_type": "conversation",
                "object_id": conversation.conversation_id,
            }
        ])
    );
    assert_eq!(
        packet.3,
        serde_json::json!([{ "reason": "policy_restricted_context" }])
    );

    let edge = sqlx::query_as::<_, (String, i32, String, serde_json::Value)>(
        r"
        SELECT stage, rank_score, eligibility_reason, rank_reasons
        FROM context_packet_memory_edges
        WHERE context_packet_id = $1
          AND memory_id = $2
        ",
    )
    .bind("ctx_pg_packet")
    .bind(included_memory.id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("packet memory edge");
    assert_eq!(edge.0, "included_in_packet");
    assert_eq!(edge.1, 42);
    assert_eq!(edge.2, "active_scope");
    assert_eq!(
        edge.3,
        serde_json::json!(["explicit_memory_request", "same_human_participant"])
    );

    let omission =
        sqlx::query_as::<_, (Option<String>, String, String, String, serde_json::Value)>(
            r"
        SELECT memory_id, omission_sensitivity, agent_visible_reason, audit_reason, details
        FROM context_packet_omissions
        WHERE context_packet_id = $1
        ",
        )
        .bind("ctx_pg_packet")
        .fetch_one(repo.pool())
        .await
        .expect("packet omission");
    assert_eq!(omission.0.as_deref(), Some(denied_memory.id.as_str()));
    assert_eq!(omission.1, "sensitive");
    assert_eq!(omission.2, "policy_restricted_context");
    assert_eq!(omission.3, "sensitivity_ceiling");
    assert_eq!(
        omission.4,
        serde_json::json!({
            "run_id": "run:pg-packet",
            "purpose": "answer_human_question",
        })
    );

    let use_records = sqlx::query_as::<_, (String, String, String, String)>(
        r"
        SELECT
          memory_id,
          stage,
          agent_actor_id,
          context_object_type
        FROM memory_use_records
        WHERE context_packet_id = $1
        ORDER BY stage
        ",
    )
    .bind("ctx_pg_packet")
    .fetch_all(repo.pool())
    .await
    .expect("memory use rows");
    assert_eq!(use_records.len(), 2);
    assert!(use_records.iter().any(|record| {
        record.0 == included_memory.id
            && record.1 == "included_in_packet"
            && record.2 == "agent:primary"
            && record.3 == "conversation"
    }));
    assert!(use_records.iter().any(|record| {
        record.0 == included_memory.id
            && record.1 == "retrieved"
            && record.2 == "agent:primary"
            && record.3 == "conversation"
    }));
}

#[tokio::test]
async fn postgres_context_graph_filter_includes_relationship_omission_backing_memory() {
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
            author: ActorRef::human("human:local"),
            content_text: Some("Alice prefers quiet train cars.".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source item");

    let mut backing = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("conversation object ref"),
        "Alice prefers quiet train cars.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    backing.status = MemoryStatus::Active;
    backing.subjects = vec![
        NewMemorySubject::new("human:alice", "human", "Alice", SubjectRole::About),
        NewMemorySubject::new(
            "concept:quiet_train_cars",
            "concept",
            "Quiet train cars",
            SubjectRole::Affected,
        ),
    ];
    let backing_memory = repo
        .append_memory_candidate(backing)
        .await
        .expect("backing memory");

    sqlx::query(
        r"
        INSERT INTO relationships (
          relationship_id,
          owner_object_type,
          owner_object_id,
          subject_entity_id,
          predicate,
          object_entity_id,
          memory_id,
          status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ",
    )
    .bind("rel_pg_omitted")
    .bind("conversation")
    .bind(conversation.conversation_id.as_str())
    .bind("human:alice")
    .bind("prefers")
    .bind("concept:quiet_train_cars")
    .bind(backing_memory.id.as_str())
    .bind("active")
    .execute(repo.pool())
    .await
    .expect("relationship");

    let mut trusted =
        TrustedRetrievalContext::for_human("human:local", Purpose::AnswerHumanQuestion);
    trusted.active_agent_ids = vec!["agent:primary".to_string()];
    trusted.active_scopes = vec![format!("conversation:{}", conversation.conversation_id)];
    let request = MemoryRetrievalRequest {
        requesting_principal_id: "agent:primary".to_string(),
        trusted,
        untrusted_hints: UntrustedHints::default(),
    };
    let result = MemoryRetrievalResult {
        included: vec![],
        denied_for_audit: vec![AuditDenial {
            memory_id: None,
            relationship_id: Some("rel_pg_omitted".to_string()),
            reason: DenialReason::PurposeDenied,
        }],
        agent_visible_omissions: vec![AgentVisibleOmission {
            reason: "relationship_restricted_context",
        }],
        use_records: vec![],
    };

    repo.record_context_packet(
        "ctx_pg_relationship_omission",
        "run:pg-relationship-omission",
        &request,
        &result,
    )
    .await
    .expect("record context packet");

    let filter = crate::ContextGraphFilter {
        run_id: Some("run:pg-relationship-omission".to_string()),
        context_packet_id: Some("ctx_pg_relationship_omission".to_string()),
    };
    let graph = repo
        .inspect_context_graph_with_filter(&filter, Some(20))
        .await
        .expect("context graph");

    assert!(
        graph
            .memories
            .iter()
            .any(|memory| memory.memory_id == backing_memory.id)
    );
    assert!(
        graph
            .subject_edges
            .iter()
            .any(|edge| { edge.memory_id == backing_memory.id && edge.entity_id == "human:alice" })
    );
    assert!(graph.provenance_edges.iter().any(|edge| {
        edge.memory_id == backing_memory.id && edge.source_object_id == source_item.item_id
    }));
    assert!(graph.relationships.iter().any(|relationship| {
        relationship.relationship_id == "rel_pg_omitted"
            && relationship.memory_id.as_deref() == Some(backing_memory.id.as_str())
    }));
}

#[tokio::test]
async fn retrieve_memories_uses_postgres_search_and_policy_fingerprint() {
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
            author: ActorRef::human("human:local"),
            content_text: Some("remember the postgres retrieval caboose".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source item");

    let mut candidate = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(
            ObjectType::Conversation,
            conversation.conversation_id.as_str(),
        )
        .expect("conversation ref"),
        "The Postgres retrieval caboose should be easy to find.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    candidate.status = MemoryStatus::Active;
    candidate.sensitivity = Sensitivity::Private;
    candidate.retrieval_hints =
        serde_json::json!({"topics": ["postgres"], "keywords": ["caboose"]});
    candidate.participants = vec![NewMemoryParticipant::new(
        ActorRef::human("human:local"),
        ParticipantRole::HumanInScope,
    )];
    let memory = repo
        .append_memory_candidate(candidate)
        .await
        .expect("memory");
    let mut public_hint_candidate = NewMemoryCandidate::confirmed_note(
        ObjectRef::human("human:local"),
        "The public Postgres retrieval signal mentions a switchstand.",
        ActorRef::agent("agent:primary"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    public_hint_candidate.status = MemoryStatus::Active;
    public_hint_candidate.sensitivity = Sensitivity::Public;
    public_hint_candidate.retrieval_hints =
        serde_json::json!({"topics": ["postgres"], "keywords": ["switchstand"]});
    let public_memory = repo
        .append_memory_candidate(public_hint_candidate)
        .await
        .expect("public memory");

    sqlx::query(
        r"
        INSERT INTO memory_retrieval_purpose_rules (
          memory_id,
          purpose,
          effect,
          created_by_actor_id
        )
        VALUES ($1, 'answer_human_question', 'allow', 'agent:primary')
        ",
    )
    .bind(memory.id.as_str())
    .execute(repo.pool())
    .await
    .expect("purpose rule");
    sqlx::query(
        r"
        INSERT INTO object_access_grants (
          grant_id,
          target_object_type,
          target_object_id,
          grantee_object_type,
          grantee_object_id,
          permission,
          effect,
          created_by_actor_id
        )
        VALUES (
          'grant_pg_retrieval_policy',
          'memory_item',
          $1,
          'agent',
          'agent:primary',
          'use_for_retrieval',
          'allow',
          'human:local'
        )
        ",
    )
    .bind(memory.id.as_str())
    .execute(repo.pool())
    .await
    .expect("grant");

    let mut trusted =
        TrustedRetrievalContext::for_human("human:local", Purpose::AnswerHumanQuestion);
    trusted.active_agent_ids = vec!["agent:primary".to_string()];
    trusted.active_scopes = vec![format!("conversation:{}", conversation.conversation_id)];
    trusted.explicit_memory_request = true;
    trusted.sensitivity_ceiling = Sensitivity::Private;
    let request = MemoryRetrievalRequest {
        requesting_principal_id: "agent:primary".to_string(),
        trusted,
        untrusted_hints: UntrustedHints {
            query_text: "postgres caboose".to_string(),
            fuzzy_topics: vec!["retrieval".to_string()],
            fuzzy_entities: Vec::new(),
        },
    };

    let stale = repo
        .retrieve_memories(&request)
        .await
        .expect("stale retrieve");
    assert_eq!(stale.included.len(), 1);
    assert_eq!(stale.included[0].memory_id, public_memory.id);
    assert_eq!(
        stale.included[0].eligibility_reason,
        EligibilityReason::PublicHint
    );
    assert_eq!(
        stale
            .denied_for_audit
            .iter()
            .find(|denial| denial.memory_id.as_deref() == Some(memory.id.as_str()))
            .expect("private memory denied")
            .reason,
        DenialReason::RetrievalPolicyInvalid
    );

    let fingerprint = repo
        .refresh_retrieval_policy_fingerprint(
            memory.id.as_str(),
            ActorRef::agent("agent:primary"),
            "postgres-test",
        )
        .await
        .expect("refresh fingerprint");
    assert!(fingerprint.starts_with("sha256:"));

    let policy_row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        r"
        SELECT
          retrieval_policy_status,
          retrieval_policy_fingerprint,
          retrieval_policy_extractor_actor_id,
          retrieval_policy_extractor_version
        FROM memory_items
        WHERE memory_id = $1
        ",
    )
    .bind(memory.id.as_str())
    .fetch_one(repo.pool())
    .await
    .expect("policy row");
    assert_eq!(policy_row.0, "valid");
    assert_eq!(policy_row.1.as_deref(), Some(fingerprint.as_str()));
    assert_eq!(policy_row.2.as_deref(), Some("agent:primary"));
    assert_eq!(policy_row.3.as_deref(), Some("postgres-test"));

    let result = repo.retrieve_memories(&request).await.expect("retrieve");
    assert_eq!(result.included.len(), 2);
    assert!(result.included.iter().any(|retrieved| {
        retrieved.memory_id == memory.id
            && retrieved.eligibility_reason == EligibilityReason::ActiveScope
    }));
    assert!(result.included.iter().any(|retrieved| {
        retrieved.memory_id == public_memory.id
            && retrieved.eligibility_reason == EligibilityReason::PublicHint
    }));
    assert!(result.denied_for_audit.is_empty());
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

fn assert_sqlstate(
    result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
    expected_sqlstate: &str,
) {
    let Err(error) = result else {
        panic!("expected database error with SQLSTATE {expected_sqlstate}");
    };
    let database_error = error.as_database_error().expect("expected database error");
    assert_eq!(database_error.code().as_deref(), Some(expected_sqlstate));
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

async fn assert_memory_dedupe_fingerprint_column_exists(pool: &sqlx::PgPool) {
    let column = sqlx::query_as::<_, (String, String)>(
        r"
        SELECT is_nullable, udt_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'memory_items'
          AND column_name = 'memory_dedupe_fingerprint'
        ",
    )
    .fetch_optional(pool)
    .await
    .expect("check memory_items.memory_dedupe_fingerprint column")
    .expect("missing memory_items.memory_dedupe_fingerprint column");

    assert_eq!(column.0, "YES");
    assert_eq!(column.1, "text");
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
        POSTGRES_SCHEMA_SQL.contains("memory_dedupe_fingerprint TEXT"),
        "missing memory dedupe fingerprint column"
    );
    assert!(
        POSTGRES_SCHEMA_SQL
            .contains("CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_items_live_dedupe_fingerprint"),
        "missing live memory dedupe fingerprint unique index"
    );
    assert!(
        POSTGRES_SCHEMA_SQL
            .contains("AND status IN ('candidate', 'active', 'confirmed', 'inferred')"),
        "memory dedupe fingerprint index should cover only usable live statuses"
    );
    assert!(
        POSTGRES_MEMORY_SUMMARY_BY_DEDUPE_FINGERPRINT_SQL
            .contains("AND mi.status IN ('candidate', 'active', 'confirmed', 'inferred')"),
        "memory dedupe fingerprint lookup should match the usable live statuses"
    );
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
