use super::*;
use super::{
    memory_pipeline::{explicit_memory_content, infer_chat_sensitivity},
    protocol::TurnStreamEvent,
    runtime::CodexRuntimeHandle,
    server::bind_listener,
};
use crate::{
    DatabaseConfig,
    memory::Sensitivity,
    memory_persistence::{
        ConversationItemKind, ConversationItemStatus, MemoryType, PostgresMemoryRepository,
        ReplayMode,
    },
    providers::codex::CodexProviderConfig,
};
use serde_json::json;
use std::path::PathBuf;
use tokio::sync::mpsc;

const TEST_DATABASE_URL_ENV: &str = "NOEMA_TEST_DATABASE_URL";
static DAEMON_POSTGRES_TEST_SCHEMA_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[test]
fn protocol_round_trips_requests_and_responses() {
    let request = DaemonRequest::ConversationTurn {
        conversation_id: "conversation_1".to_string(),
        input: "hello".to_string(),
    };
    let encoded = serde_json::to_string(&request).expect("encode");
    let decoded: DaemonRequest = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded, request);

    let response = DaemonResponse::ConversationStarted {
        conversation_id: "conversation_1".to_string(),
        provider: "codex".to_string(),
        provider_thread_id: "thread_1".to_string(),
    };
    let encoded = serde_json::to_string(&response).expect("encode");
    let decoded: DaemonResponse = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded, response);

    let response = DaemonResponse::ConversationItem {
        conversation_id: "conversation_1".to_string(),
        item_id: "item_1".to_string(),
        turn_id: Some("turn_1".to_string()),
        item: TurnTranscriptItem::Activity {
            id: "memory_extraction:conversation_1:1".to_string(),
            activity_kind: "memory_extraction".to_string(),
            status: TurnActivityStatus::Started,
            title: "Extracting memory proposals".to_string(),
            summary: Some("ordinary chat memory extraction is running".to_string()),
            metadata: json!({ "turn_index": 1 }),
        },
    };
    let encoded = serde_json::to_string(&response).expect("encode");
    assert!(encoded.contains(r#""type":"conversation_item""#));
    assert!(encoded.contains(r#""item_id":"item_1""#));
    assert!(encoded.contains(r#""turn_id":"turn_1""#));
    let decoded: DaemonResponse = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded, response);

    let response = DaemonResponse::AgentStatusChanged {
        conversation_id: "conversation_1".to_string(),
        status: AgentStatus::Thinking,
    };
    let encoded = serde_json::to_string(&response).expect("encode");
    assert!(encoded.contains(r#""type":"agent_status_changed""#));
    assert!(encoded.contains(r#""status":"thinking""#));
    let decoded: DaemonResponse = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded, response);
}

#[test]
fn socket_path_is_under_noema_run_directory() {
    let path = socket_path_for_home("/tmp/noema-test-home");

    assert_eq!(
        path,
        PathBuf::from("/tmp/noema-test-home/.noema/run/noema.sock")
    );
}

struct RuntimeTestDatabase {
    url: String,
    _schema_guard: tokio::sync::MutexGuard<'static, ()>,
}

async fn test_database() -> Option<RuntimeTestDatabase> {
    let Ok(url) = std::env::var(TEST_DATABASE_URL_ENV) else {
        eprintln!("skipping daemon Postgres test; {TEST_DATABASE_URL_ENV} is unset");
        return None;
    };
    let schema_guard = DAEMON_POSTGRES_TEST_SCHEMA_LOCK.lock().await;
    let database = DatabaseConfig::new(url.clone()).expect("database config");
    let pool = database.connect().await.expect("connect test database");
    sqlx::raw_sql("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .execute(&pool)
        .await
        .expect("reset test schema");
    Some(RuntimeTestDatabase {
        url,
        _schema_guard: schema_guard,
    })
}

async fn postgres_repo(database: &RuntimeTestDatabase) -> PostgresMemoryRepository {
    let config = DatabaseConfig::new(database.url.clone()).expect("database config");
    PostgresMemoryRepository::connect(&config)
        .await
        .expect("repo")
}

#[tokio::test]
async fn second_listener_on_same_socket_is_rejected() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("noema.sock");
    let _listener = bind_listener(&socket_path).await.expect("listener");

    let error = bind_listener(&socket_path).await.unwrap_err();

    assert!(matches!(error, DaemonError::AlreadyRunning { .. }));
}

#[tokio::test]
async fn runtime_actor_allocates_distinct_conversation_ids() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let first = handle
        .start_conversation(None, None)
        .await
        .expect("first conversation");
    let second = handle
        .start_conversation(None, None)
        .await
        .expect("second conversation");

    assert_ne!(first.conversation_id, second.conversation_id);
    assert_ne!(first.provider_thread_id, second.provider_thread_id);

    let items = collect_turn(&handle, first.conversation_id.clone(), "hello".to_string())
        .await
        .expect("turn response");
    assert_eq!(assistant_text(&items), "fake answer");

    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_turn_streams_durable_assistant_item_and_idle_status() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "hello".to_string()).await;
    result.expect("turn");
    handle.shutdown().await;

    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::InputReceived,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Thinking,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Idle,
            } if id == &conversation_id
        )
    }));

    let Some((assistant_item_id, assistant_turn_id)) =
        events.iter().find_map(|event| match event {
            TurnStreamEvent::ConversationItem {
                conversation_id: id,
                item_id,
                turn_id,
                item: TurnTranscriptItem::AssistantText { text },
            } if id == &conversation_id && text == "fake answer" => {
                Some((item_id.clone(), turn_id.clone()))
            }
            _ => None,
        })
    else {
        panic!("expected durable assistant conversation item, got {events:?}");
    };
    assert!(assistant_item_id.starts_with("item_"));
    assert!(assistant_turn_id.is_some());

    let repo = postgres_repo(&database).await;
    let replay = repo
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.item_id == assistant_item_id
            && item.kind == ConversationItemKind::AssistantText
            && item.status == ConversationItemStatus::Completed
    }));

    let (agent_status, turn_status) = conversation_and_turn_statuses(&repo, &conversation_id).await;
    assert_eq!(agent_status, "idle");
    assert_eq!(turn_status, "completed");
}

#[test]
fn explicit_memory_parser_accepts_only_top_level_commands() {
    assert_eq!(
        explicit_memory_content("remember this: Kevin prefers inspectable memory").as_deref(),
        Some("Kevin prefers inspectable memory")
    );
    assert_eq!(
        explicit_memory_content(" remember that: project decisions belong to projects ").as_deref(),
        Some("project decisions belong to projects")
    );
    assert_eq!(
        explicit_memory_content("/remember Kevin likes concise inspection output").as_deref(),
        Some("Kevin likes concise inspection output")
    );
    assert_eq!(
        explicit_memory_content("/remember: Kevin likes durable memory").as_deref(),
        Some("Kevin likes durable memory")
    );
    assert_eq!(explicit_memory_content("hello remember this: nope"), None);
    assert_eq!(explicit_memory_content("> remember this: quoted"), None);
    assert_eq!(explicit_memory_content("don't remember this: nope"), None);
    assert_eq!(explicit_memory_content("remember this:"), None);
}

#[test]
fn deterministic_sensitivity_classifier_fails_closed_for_common_secrets() {
    assert_eq!(
        infer_chat_sensitivity("my API key is sk-test1234567890"),
        Sensitivity::Secret
    );
    assert_eq!(
        infer_chat_sensitivity("my doctor diagnosed this last week"),
        Sensitivity::Sensitive
    );
    assert_eq!(
        infer_chat_sensitivity("Kevin prefers CLI memory inspection"),
        Sensitivity::Normal
    );
}

#[tokio::test]
async fn runtime_actor_persists_explicit_remember_confirmed() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_memory_extraction();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "remember this: Kevin prefers CLI memory inspection.".to_string(),
    )
    .await
    .expect("turn");
    assert_eq!(assistant_text(&items), "fake answer");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                summary: Some(summary),
                ..
            } if activity_kind == "memory_save"
                && title == "Memory saved"
                && summary == "saved one explicit memory"
        )
    }));
    let explicit_card = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::A2uiCard {
                schema, payload, ..
            } if schema == "memory_cards" => Some(payload),
            _ => None,
        })
        .expect("explicit memory card");
    assert_eq!(
        explicit_card["created_memory_ids"]
            .as_array()
            .expect("created ids")
            .len(),
        1
    );
    assert_eq!(
        explicit_card["memories"][0]["content"],
        "Kevin prefers CLI memory inspection."
    );
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Extracting memory proposals"
            )
        }),
        "explicit memory should not trigger automatic extraction activity: {items:?}"
    );
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Kevin prefers CLI memory inspection.");
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Confirmed);
    assert_eq!(memories[0].memory_type, MemoryType::Preference);
    assert_eq!(
        memories[0].conversation_id.as_deref(),
        Some(conversation_id.as_str())
    );

    let replay = repo
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert_eq!(replay.len(), 4);
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::Activity
            && item.status == ConversationItemStatus::Completed
            && item.payload_json["activity_kind"] == "memory_save"
    }));
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::A2uiCard
            && item.status == ConversationItemStatus::Completed
            && item.payload_json["schema"] == "memory_cards"
    }));

    let conversation_item_provenance_count: i64 = sqlx::query_scalar(
        r"
            SELECT COUNT(*)
            FROM object_provenance_edges pe
            JOIN conversation_items ci
              ON pe.source_object_type = 'conversation_item'
             AND ci.item_id = pe.source_object_id
            WHERE pe.target_object_type = 'memory_item'
              AND pe.target_object_id = $1
              AND ci.kind = 'user_text'
            ",
    )
    .bind(&memories[0].id)
    .fetch_one(repo.pool())
    .await
    .expect("conversation item provenance count");
    assert_eq!(conversation_item_provenance_count, 1);

    let conversation_item_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM conversation_items WHERE conversation_id = $1")
            .bind(&conversation_id)
            .fetch_one(repo.pool())
            .await
            .expect("conversation item count");
    assert_eq!(conversation_item_count, 4);

    let (agent_status, turn_status) = conversation_and_turn_statuses(&repo, &conversation_id).await;
    assert_eq!(agent_status, "idle");
    assert_eq!(turn_status, "completed");
}

#[tokio::test]
async fn runtime_actor_extracts_ordinary_chat_memory_in_background() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_memory_extraction();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "I prefer automatic memory extraction in chat.".to_string(),
    )
    .await
    .expect("turn");
    assert_eq!(assistant_text(&items), "fake answer");
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity { activity_kind, .. }
                    if activity_kind == "memory_extraction"
            )
        }),
        "fallback extraction should not block the turn stream: {items:?}"
    );
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(
        memories[0].content,
        "Kevin prefers automatic memory extraction in chat."
    );
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Active);
    assert_eq!(memories[0].memory_type, MemoryType::Preference);
    assert_eq!(
        memories[0].conversation_id.as_deref(),
        Some(conversation_id.as_str())
    );
}

#[tokio::test]
async fn runtime_actor_persists_provider_structured_memory_proposals_as_activity() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_memory_extraction();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "I prefer same-call memory proposals.".to_string(),
    )
    .await
    .expect("turn");
    assert_eq!(assistant_text(&items), "fake answer");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                summary: Some(summary),
                ..
            } if activity_kind == "memory_extraction"
                && summary == "created 1 memory candidate"
        )
    }));
    let proposal_card = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::A2uiCard {
                schema, payload, ..
            } if schema == "memory_proposals" => Some(payload),
            _ => None,
        })
        .expect("provider memory proposal card");
    assert_eq!(
        proposal_card["created_memory_ids"]
            .as_array()
            .expect("created ids")
            .len(),
        1
    );
    assert_eq!(
        proposal_card["proposals"][0]["proposal"]["content"],
        "Kevin prefers same-call memory proposals."
    );
    assert_eq!(proposal_card["proposals"][0]["status"], "active");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(
        memories[0].content,
        "Kevin prefers same-call memory proposals."
    );
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Active);
    assert_eq!(memories[0].memory_type, MemoryType::Preference);

    let replay = repo
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::Activity
            && item.status == ConversationItemStatus::Completed
            && item.payload_json["activity_kind"] == "memory_extraction"
    }));
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::A2uiCard
            && item.status == ConversationItemStatus::Completed
            && item.payload_json["schema"] == "memory_proposals"
    }));

    let (agent_status, turn_status) = conversation_and_turn_statuses(&repo, &conversation_id).await;
    assert_eq!(agent_status, "idle");
    assert_eq!(turn_status, "completed");
}

#[tokio::test]
async fn runtime_actor_extracts_natural_remember_through_structured_provider_output() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_memory_extraction();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "Please remember I'm a big fan of trains".to_string(),
    )
    .await
    .expect("turn");
    assert_eq!(assistant_text(&items), "fake answer");
    let proposal_card = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::A2uiCard {
                schema, payload, ..
            } if schema == "memory_proposals" => Some(payload),
            _ => None,
        })
        .expect("provider memory proposal card");
    assert_eq!(
        proposal_card["proposals"][0]["proposal"]["content"],
        "Kevin is a big fan of trains."
    );
    assert_eq!(proposal_card["proposals"][0]["status"], "active");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Kevin is a big fan of trains.");
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Active);
    assert_eq!(memories[0].memory_type, MemoryType::Preference);
}

#[tokio::test]
async fn runtime_actor_keeps_third_party_subject_conversation_scoped() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_memory_extraction();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Alice prefers decaf.".to_string(),
    )
    .await
    .expect("turn");
    assert_eq!(assistant_text(&items), "fake answer");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Alice prefers decaf.");
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Candidate);
    assert_eq!(
        memories[0].home_scope_id,
        format!("conversation:{conversation_id}")
    );

    let (linked_object_type, linked_object_id) = sqlx::query_as::<
        _,
        (Option<String>, Option<String>),
    >(
        "SELECT linked_object_type, linked_object_id FROM entities WHERE entity_id = 'human:alice'",
    )
    .fetch_one(repo.pool())
    .await
    .expect("Alice entity");
    assert_eq!(linked_object_type, None);
    assert_eq!(linked_object_id, None);
}

#[tokio::test]
async fn runtime_actor_persists_explicit_remember_before_provider_failure() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_turn_error();
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        database.url.clone(),
    )
    .await
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "/remember Kevin wants failed turns to keep explicit memory.".to_string(),
    )
    .await;
    let error = result.expect_err("provider error");
    assert!(matches!(error, DaemonError::Provider(_)));
    handle.shutdown().await;

    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Error,
            } if id == &conversation_id
        )
    }));
    for expected in ["activity", "a2ui_card", "error_notice"] {
        let Some(item_id) = events.iter().find_map(|event| match (expected, event) {
            (
                "activity",
                TurnStreamEvent::ConversationItem {
                    conversation_id: id,
                    item_id,
                    turn_id: Some(_),
                    item: TurnTranscriptItem::Activity { .. },
                },
            ) if id == &conversation_id => Some(item_id.clone()),
            (
                "a2ui_card",
                TurnStreamEvent::ConversationItem {
                    conversation_id: id,
                    item_id,
                    turn_id: Some(_),
                    item: TurnTranscriptItem::A2uiCard { .. },
                },
            ) if id == &conversation_id => Some(item_id.clone()),
            (
                "error_notice",
                TurnStreamEvent::ConversationItem {
                    conversation_id: id,
                    item_id,
                    turn_id: Some(_),
                    item: TurnTranscriptItem::ErrorNotice { .. },
                },
            ) if id == &conversation_id => Some(item_id.clone()),
            _ => None,
        }) else {
            panic!("expected durable {expected} conversation item, got {events:?}");
        };
        assert!(item_id.starts_with("item_"));
    }

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(
        memories[0].content,
        "Kevin wants failed turns to keep explicit memory."
    );
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Confirmed);

    let replay = repo
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::Activity
            && item.status == ConversationItemStatus::Completed
            && item.payload_json["activity_kind"] == "memory_save"
    }));
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::A2uiCard
            && item.status == ConversationItemStatus::Completed
            && item.payload_json["schema"] == "memory_cards"
    }));
    assert!(replay.iter().any(|item| {
        item.kind == ConversationItemKind::ErrorNotice
            && item.status == ConversationItemStatus::Failed
            && item
                .content_text
                .as_deref()
                .is_some_and(|text| text.contains("turn failed"))
    }));

    let (agent_status, turn_status) = conversation_and_turn_statuses(&repo, &conversation_id).await;
    assert_eq!(agent_status, "error");
    assert_eq!(turn_status, "failed");
}

async fn conversation_and_turn_statuses(
    repo: &PostgresMemoryRepository,
    conversation_id: &str,
) -> (String, String) {
    sqlx::query_as::<_, (String, String)>(
        r"
        SELECT c.agent_status, t.status
        FROM conversations c
        JOIN conversation_turns t ON t.conversation_id = c.conversation_id
        WHERE c.conversation_id = $1
        ",
    )
    .bind(conversation_id)
    .fetch_one(repo.pool())
    .await
    .expect("conversation statuses")
}

async fn collect_turn(
    handle: &CodexRuntimeHandle,
    conversation_id: String,
    input: String,
) -> Result<Vec<TurnTranscriptItem>, DaemonError> {
    let (result, events) = collect_turn_events(handle, conversation_id, input).await;
    result?;
    Ok(transcript_items_from_events(events))
}

async fn collect_turn_events(
    handle: &CodexRuntimeHandle,
    conversation_id: String,
    input: String,
) -> (Result<(), DaemonError>, Vec<TurnStreamEvent>) {
    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    let result = handle.turn(conversation_id, input, item_tx).await;
    let mut events = Vec::new();
    while let Ok(event) = item_rx.try_recv() {
        events.push(event);
    }
    (result, events)
}

fn transcript_items_from_events(events: Vec<TurnStreamEvent>) -> Vec<TurnTranscriptItem> {
    events
        .into_iter()
        .filter_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. }
                if !matches!(item, TurnTranscriptItem::UserText { .. }) =>
            {
                Some(item)
            }
            TurnStreamEvent::ConversationItem { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .collect()
}

fn fake_codex_app_server_script_with_turn_error() -> std::path::PathBuf {
    let dir = tempfile::tempdir().expect("temp dir").keep();
    let path = dir.join("fake-codex-error");
    std::fs::write(
        &path,
        r#"#!/usr/bin/env python3
import json
import sys

next_thread = 1

for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"id": msg["id"], "result": {"userAgent": "fake"}}), flush=True)
    elif method == "initialized":
        pass
    elif method == "thread/start":
        thread = f"thread_{next_thread}"
        next_thread += 1
        print(json.dumps({"id": msg["id"], "result": {"thread": {"id": thread}}}), flush=True)
    elif method == "turn/start":
        print(json.dumps({"id": msg["id"], "error": {"code": -32000, "message": "turn failed"}}), flush=True)
"#,
    )
    .expect("write script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod");
    }

    path
}

fn assistant_text(items: &[TurnTranscriptItem]) -> &str {
    let Some(text) = items.iter().find_map(|item| match item {
        TurnTranscriptItem::AssistantText { text } => Some(text.as_str()),
        TurnTranscriptItem::UserText { .. }
        | TurnTranscriptItem::Activity { .. }
        | TurnTranscriptItem::A2uiCard { .. }
        | TurnTranscriptItem::ErrorNotice { .. } => None,
    }) else {
        panic!("expected assistant text item, got {items:?}");
    };
    text
}

fn fake_codex_app_server_script() -> std::path::PathBuf {
    let dir = tempfile::tempdir().expect("temp dir").keep();
    let path = dir.join("fake-codex");
    std::fs::write(
        &path,
        r#"#!/usr/bin/env python3
import json
import sys

next_thread = 1

for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"id": msg["id"], "result": {"userAgent": "fake"}}), flush=True)
    elif method == "initialized":
        pass
    elif method == "thread/start":
        if "model" in msg.get("params", {}):
            print(json.dumps({"id": msg["id"], "error": {"code": -32602, "message": "model should be omitted by default"}}), flush=True)
            continue
        thread = f"thread_{next_thread}"
        next_thread += 1
        print(json.dumps({"id": msg["id"], "result": {"thread": {"id": thread}}}), flush=True)
    elif method == "turn/start":
        input_text = "".join(
            part.get("text", "")
            for part in msg.get("params", {}).get("input", [])
            if part.get("type") == "text"
        )
        text = "fake answer"
        if "noema_response" in input_text:
            text = json.dumps({
                "type": "noema_response",
                "output": [
                    {"kind": "assistant_text", "text": "fake answer"},
                    {"kind": "memory_proposals", "proposals": []}
                ]
            })
        print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
        print(json.dumps({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": text}}}), flush=True)
        print(json.dumps({"method": "turn/completed", "params": {"turn": {"status": "completed"}}}), flush=True)
"#,
    )
    .expect("write script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod");
    }

    path
}

fn fake_codex_app_server_script_with_memory_extraction() -> std::path::PathBuf {
    let dir = tempfile::tempdir().expect("temp dir").keep();
    let path = dir.join("fake-codex-memory-extraction");
    std::fs::write(
        &path,
        r#"#!/usr/bin/env python3
import json
import sys

next_thread = 1

for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"id": msg["id"], "result": {"userAgent": "fake"}}), flush=True)
    elif method == "initialized":
        pass
    elif method == "thread/start":
        thread = f"thread_{next_thread}"
        next_thread += 1
        print(json.dumps({"id": msg["id"], "result": {"thread": {"id": thread}}}), flush=True)
    elif method == "turn/start":
        input_text = "".join(
            part.get("text", "")
            for part in msg.get("params", {}).get("input", [])
            if part.get("type") == "text"
        )
        if "ordinary-chat memory proposal extractor" in input_text:
            if "Alice prefers decaf." in input_text:
                proposal = {
                    "content": "Alice prefers decaf.",
                    "memory_type": "preference",
                    "title": "Alice decaf preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [
                        {
                            "id": None,
                            "kind": "human",
                            "name": "Alice",
                            "role": "about"
                        }
                    ],
                    "retrieval_hints": {
                        "topics": ["people"],
                        "keywords": ["Alice", "decaf"],
                        "summary": "Alice prefers decaf."
                    },
                    "risk_flags": [],
                    "evidence_excerpt": "Alice prefers decaf."
                }
            else:
                proposal = {
                    "content": "Kevin prefers automatic memory extraction in chat.",
                    "memory_type": "preference",
                    "title": "Automatic memory extraction preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [
                        {
                            "id": "human:local",
                            "kind": "human",
                            "name": "Kevin",
                            "role": "about"
                        }
                    ],
                    "retrieval_hints": {
                        "topics": ["memory"],
                        "keywords": ["automatic memory extraction", "chat"],
                        "summary": "Kevin prefers automatic memory extraction in chat."
                    },
                    "risk_flags": [],
                    "evidence_excerpt": "I prefer automatic memory extraction in chat."
                }
            text = json.dumps({"proposals": [proposal]})
        elif "I prefer same-call memory proposals." in input_text and "noema_response" in input_text:
            proposal = {
                "content": "Kevin prefers same-call memory proposals.",
                "memory_type": "preference",
                "title": "Same-call memory proposal preference",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [
                    {
                        "id": "human:local",
                        "kind": "human",
                        "name": "Kevin",
                        "role": "about"
                    }
                ],
                "retrieval_hints": {
                    "topics": ["memory"],
                    "keywords": ["same-call memory proposals"],
                    "summary": "Kevin prefers same-call memory proposals."
                },
                "risk_flags": [],
                "evidence_excerpt": "I prefer same-call memory proposals."
            }
            text = json.dumps({
                "type": "noema_response",
                "output": [
                    {"kind": "assistant_text", "text": "fake answer"},
                    {"kind": "memory_proposals", "proposals": [proposal]}
                ]
            })
        elif "Please remember I'm a big fan of trains" in input_text and "noema_response" in input_text:
            proposal = {
                "content": "Kevin is a big fan of trains.",
                "memory_type": "preference",
                "title": "Train enthusiasm",
                "confidence": 0.92,
                "sensitivity": "normal",
                "subjects": [
                    {
                        "id": "human:local",
                        "kind": "human",
                        "name": "Kevin",
                        "role": "about"
                    }
                ],
                "retrieval_hints": {
                    "topics": ["interests"],
                    "keywords": ["trains"],
                    "summary": "Kevin is a big fan of trains."
                },
                "risk_flags": [],
                "evidence_excerpt": "I'm a big fan of trains"
            }
            text = json.dumps({
                "type": "noema_response",
                "output": [
                    {"kind": "assistant_text", "text": "fake answer"},
                    {"kind": "memory_proposals", "proposals": [proposal]}
                ]
            })
        elif "noema_response" in input_text:
            text = json.dumps({
                "type": "noema_response",
                "output": [
                    {"kind": "assistant_text", "text": "fake answer"},
                    {"kind": "memory_proposals", "proposals": []}
                ]
            })
        else:
            text = "fake answer"
        print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
        print(json.dumps({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": text}}}), flush=True)
        print(json.dumps({"method": "turn/completed", "params": {"turn": {"status": "completed"}}}), flush=True)
"#,
    )
    .expect("write script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod");
    }

    path
}
