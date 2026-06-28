use super::*;
use super::{
    memory_pipeline::{explicit_memory_content, infer_chat_sensitivity},
    protocol::TurnStreamEvent,
    runtime::CodexRuntimeHandle,
    server::bind_listener,
};
use crate::{
    DatabaseConfig,
    memory::{MemoryStatus, Sensitivity},
    memory_persistence::{
        ConversationItemKind, ConversationItemStatus, PostgresMemoryRepository, ReplayMode,
    },
    provider::{
        GenerateInput, GenerateOutputItem, GenerateRequest, GenerateResponse, GenerateStreamEvent,
        ProviderError,
    },
};
use serde_json::json;
use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};
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
    };
    let encoded = serde_json::to_string(&response).expect("encode");
    let decoded: DaemonResponse = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded, response);

    let response = DaemonResponse::ConversationItem {
        conversation_id: "conversation_1".to_string(),
        item_id: "item_1".to_string(),
        turn_id: Some("turn_1".to_string()),
        metadata: json!({ "stream_id": "assistant_stream:turn_1:initial" }),
        item: Box::new(TurnTranscriptItem::Activity {
            id: "memory_extraction:conversation_1:1".to_string(),
            activity_kind: "memory_extraction".to_string(),
            status: TurnActivityStatus::Started,
            title: "Extracting memory proposals".to_string(),
            summary: Some("ordinary chat memory extraction is running".to_string()),
            metadata: json!({ "turn_index": 1 }),
        }),
    };
    let encoded = serde_json::to_string(&response).expect("encode");
    assert!(encoded.contains(r#""type":"conversation_item""#));
    assert!(encoded.contains(r#""item_id":"item_1""#));
    assert!(encoded.contains(r#""turn_id":"turn_1""#));
    assert!(encoded.contains(r#""stream_id":"assistant_stream:turn_1:initial""#));
    let decoded: DaemonResponse = serde_json::from_str(&encoded).expect("decode");
    assert_eq!(decoded, response);

    let response = DaemonResponse::AssistantTextDelta {
        conversation_id: "conversation_1".to_string(),
        turn_id: "turn_1".to_string(),
        stream_id: "assistant_stream:turn_1:initial".to_string(),
        delta: "fake".to_string(),
    };
    let encoded = serde_json::to_string(&response).expect("encode");
    assert!(encoded.contains(r#""type":"assistant_text_delta""#));
    assert!(encoded.contains(r#""delta":"fake""#));
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
    assert_test_database_url(&url);
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

#[test]
fn daemon_test_database_guard_rejects_non_test_database_names() {
    assert_test_database_url("postgres://noema:noema@localhost:5432/noema_test");
    assert_test_database_url("postgres://noema:noema@localhost:5432/test");
    assert_test_database_url("postgres://noema:noema@localhost:5432/noema_test?sslmode=disable");

    assert!(!is_test_database_url(
        "postgres://noema:noema@localhost:5432/noema"
    ));
    assert!(!is_test_database_url(
        "postgres://noema:noema@localhost:5432/postgres"
    ));
}

#[tokio::test]
async fn runtime_actor_allocates_distinct_conversation_ids() {
    let handle = test_runtime_handle(fake_codex_provider()).await;

    let first = handle
        .start_conversation(None, None)
        .await
        .expect("first conversation");
    let second = handle
        .start_conversation(None, None)
        .await
        .expect("second conversation");

    assert_ne!(first.conversation_id, second.conversation_id);

    let items = collect_turn(&handle, first.conversation_id.clone(), "hello".to_string())
        .await
        .expect("turn response");
    assert_eq!(assistant_text(&items), "fake answer");

    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_turn_streams_durable_assistant_item_and_idle_status() {
    let (handle, store) = test_runtime_handle_with_store(fake_codex_provider()).await;

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
                item,
                ..
            } if id == &conversation_id => match item.as_ref() {
                TurnTranscriptItem::AssistantText { text } if text == "fake answer" => {
                    Some((item_id.clone(), turn_id.clone()))
                }
                _ => None,
            },
            _ => None,
        })
    else {
        panic!("expected durable assistant conversation item, got {events:?}");
    };
    assert!(assistant_item_id.starts_with("item:"));
    assert!(assistant_turn_id.is_some());

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(replay.iter().any(|item| {
        item.item_id == assistant_item_id
            && item.kind == ConversationItemKind::AssistantText
            && item.status == ConversationItemStatus::Completed
    }));
}

#[tokio::test]
async fn runtime_turn_streams_assistant_text_deltas_before_durable_item() {
    let (handle, store) = test_runtime_handle_with_store(fake_codex_provider()).await;
    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) =
        collect_turn_events(&handle, conversation_id.clone(), "hello".to_string()).await;
    result.expect("turn");
    handle.shutdown().await;

    let first_delta = events
        .iter()
        .position(|event| matches!(event, TurnStreamEvent::AssistantTextDelta { .. }))
        .expect("assistant delta");
    let durable_assistant = events
        .iter()
        .position(|event| {
            matches!(
                event,
                TurnStreamEvent::ConversationItem { item, .. }
                    if matches!(item.as_ref(), TurnTranscriptItem::AssistantText { .. })
            )
        })
        .expect("durable assistant");
    assert!(first_delta < durable_assistant);

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");
    let assistant_items = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::AssistantText)
        .collect::<Vec<_>>();
    assert_eq!(assistant_items.len(), 1);
    assert_eq!(
        assistant_items[0].content_text.as_deref(),
        Some("fake answer")
    );
}

#[tokio::test]
async fn runtime_primary_conversation_sends_recent_durable_context_after_restart() {
    let (first_handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_restart_context_check()).await;
    let first_conversation_id = first_handle
        .start_primary_conversation(None, None)
        .await
        .expect("first primary conversation")
        .conversation_id;
    let first_items = collect_turn(
        &first_handle,
        first_conversation_id.clone(),
        "first durable question".to_string(),
    )
    .await
    .expect("first turn");
    assert_eq!(assistant_text(&first_items), "fake answer");
    first_handle.shutdown().await;

    let second_handle = CodexRuntimeHandle::spawn_with_provider(
        Arc::new(fake_codex_provider_with_restart_context_check()),
        store.clone(),
    )
    .await
    .expect("runtime");
    let restarted_conversation_id = second_handle
        .start_primary_conversation(None, None)
        .await
        .expect("restarted primary conversation")
        .conversation_id;
    assert_eq!(first_conversation_id, restarted_conversation_id);

    let second_items = collect_turn(
        &second_handle,
        restarted_conversation_id,
        "second durable question".to_string(),
    )
    .await
    .expect("second turn");
    assert_eq!(assistant_text(&second_items), "saw durable context");
    second_handle.shutdown().await;

    let replay = store
        .list_conversation_items(&first_conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let user_texts = replay
        .iter()
        .filter(|item| item.kind == ConversationItemKind::UserText)
        .map(|item| item.content_text.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        user_texts,
        vec![
            Some("first durable question"),
            Some("second durable question")
        ]
    );
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
async fn runtime_actor_reports_explicit_remember_unavailable() {
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

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
                status: TurnActivityStatus::Failed,
                title,
                summary: Some(summary),
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Explicit memory unavailable"
                && summary == "graph-claim memory writes are pending"
                && metadata["trigger"] == "explicit_remember"
                && metadata["unavailable"]["reason"] == "graph_claim_writes_pending"
        )
    }));
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::A2uiCard { schema, .. } if schema == "memory_cards"
            )
        }),
        "explicit memory should not emit a success-looking memory card: {items:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_actor_reports_repeated_explicit_memory_unavailable() {
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    let first_items = collect_turn(
        &handle,
        conversation_id.clone(),
        "remember: I like ice cream.".to_string(),
    )
    .await
    .expect("first turn");
    let second_items = collect_turn(
        &handle,
        conversation_id,
        "remember: I LIKE   ICE CREAM".to_string(),
    )
    .await
    .expect("second turn");
    handle.shutdown().await;

    for items in [first_items, second_items] {
        assert!(items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Failed,
                    title,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Explicit memory unavailable"
            )
        }));
    }
}

#[tokio::test]
async fn runtime_actor_reports_ordinary_chat_memory_unavailable_until_graph_claims_land() {
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I prefer automatic memory extraction in chat.".to_string(),
    )
    .await
    .expect("turn");
    assert_eq!(assistant_text(&items), "fake answer");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                title,
                summary: Some(summary),
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory extraction unavailable"
                && summary == "graph-claim memory writes are pending"
                && metadata["trigger"] == "ordinary_chat"
                && metadata["unavailable"]["reason"] == "graph_claim_writes_pending"
        )
    }));
    handle.shutdown().await;
}

#[tokio::test]
#[ignore = "legacy Postgres memory consolidation awaits graph-claim store replacement"]
async fn legacy_runtime_actor_reinforces_semantic_memory_repeat_in_postgres() {
    let Some(database) = test_database().await else {
        return;
    };
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like ice cream.".to_string(),
    )
    .await
    .expect("first turn");
    collect_turn(
        &handle,
        conversation_id,
        "Ice cream is one of my favorite desserts.".to_string(),
    )
    .await
    .expect("second turn");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Kevin likes ice cream.");

    let reinforced_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM object_events WHERE event_type = 'memory_reinforced'",
    )
    .fetch_one(repo.pool())
    .await
    .expect("reinforced count");
    assert_eq!(reinforced_count, 1);
}

#[tokio::test]
#[ignore = "legacy Postgres memory consolidation awaits graph-claim store replacement"]
async fn legacy_runtime_actor_creates_disputed_memory_for_semantic_conflict_in_postgres() {
    let Some(database) = test_database().await else {
        return;
    };
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like ice cream.".to_string(),
    )
    .await
    .expect("first turn");
    collect_turn(&handle, conversation_id, "I hate ice cream.".to_string())
        .await
        .expect("second turn");
    handle.shutdown().await;

    let repo = postgres_repo(&database).await;
    let memories = repo.list_recent_memories(Some(10)).await.expect("memories");
    assert_eq!(memories.len(), 2);
    assert!(
        memories
            .iter()
            .any(|memory| memory.content == "Kevin hates ice cream."
                && memory.status == MemoryStatus::Disputed),
        "expected disputed hate-ice-cream memory, got {memories:?}"
    );
}

#[tokio::test]
async fn runtime_actor_reports_provider_memory_proposals_unavailable() {
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "I prefer same-call memory proposals.".to_string(),
    )
    .await;
    result.expect("turn");
    let items = transcript_items_from_events(events.clone());
    assert_eq!(assistant_text(&items), "fake answer");
    let proposed_index =
        memory_extraction_event_index(&events, TurnActivityStatus::Started, "Memory proposed")
            .expect("started memory proposal activity");
    assert!(
        memory_proposals_card_event_index(&events).is_none(),
        "provider proposal card should be suppressed until graph-claim writes land"
    );
    let unavailable_index = memory_extraction_event_index(
        &events,
        TurnActivityStatus::Failed,
        "Memory persistence unavailable",
    )
    .expect("unavailable memory proposal activity");
    let proposed_item_id =
        memory_extraction_event_item_id(&events, TurnActivityStatus::Started, "Memory proposed")
            .expect("started memory proposal item id");
    assert!(
        proposed_item_id.starts_with("transient:"),
        "started memory proposal marker should be live-only, got {proposed_item_id}"
    );
    assert!(
        proposed_index < unavailable_index,
        "unavailable marker should stream after proposal marker: {events:?}"
    );
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                summary: Some(summary),
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && summary == "graph-claim memory writes are pending"
                && metadata["unavailable"]["reason"] == "graph_claim_writes_pending"
                && metadata["proposal_count"] == 1
        )
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_actor_reports_natural_remember_provider_proposals_unavailable() {
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

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
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                title,
                summary: Some(summary),
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persistence unavailable"
                && summary == "graph-claim memory writes are pending"
        )
    }));
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::A2uiCard { schema, .. } if schema == "memory_proposals"
            )
        }),
        "provider proposals should not emit a success-looking memory card: {items:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
#[ignore = "legacy Postgres subject scoping awaits graph-claim store replacement"]
async fn legacy_runtime_actor_keeps_third_party_subject_conversation_scoped_in_postgres() {
    let Some(database) = test_database().await else {
        return;
    };
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;

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
async fn runtime_actor_reports_explicit_remember_unavailable_before_provider_failure() {
    let handle = test_runtime_handle(fake_codex_provider_with_turn_error()).await;

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
    for expected in ["activity", "error_notice"] {
        let Some(item_id) = events.iter().find_map(|event| match (expected, event) {
            (
                "activity",
                TurnStreamEvent::ConversationItem {
                    conversation_id: id,
                    item_id,
                    turn_id: Some(_),
                    item,
                    ..
                },
            ) if id == &conversation_id
                && matches!(
                    item.as_ref(),
                    TurnTranscriptItem::Activity {
                        activity_kind,
                        status: TurnActivityStatus::Failed,
                        title,
                        ..
                    } if activity_kind == "memory_extraction"
                        && title == "Explicit memory unavailable"
                ) =>
            {
                Some(item_id.clone())
            }
            (
                "error_notice",
                TurnStreamEvent::ConversationItem {
                    conversation_id: id,
                    item_id,
                    turn_id: Some(_),
                    item,
                    ..
                },
            ) if id == &conversation_id
                && matches!(item.as_ref(), TurnTranscriptItem::ErrorNotice { .. }) =>
            {
                Some(item_id.clone())
            }
            _ => None,
        }) else {
            panic!("expected durable {expected} conversation item, got {events:?}");
        };
        assert!(item_id.starts_with("item:"));
    }
    assert!(
        !events.iter().any(|event| {
            matches!(
                event,
                TurnStreamEvent::ConversationItem {
                    item,
                    ..
                } if matches!(
                    item.as_ref(),
                    TurnTranscriptItem::A2uiCard { schema, .. } if schema == "memory_cards"
                )
            )
        }),
        "explicit memory should not emit a success-looking memory card: {events:?}"
    );
}

#[tokio::test]
async fn runtime_actor_persists_provider_tool_items_as_action_rows() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_tool_item()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));

    let started_tool_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    ..
                } if activity_kind == "tool_call"
                    && *status == TurnActivityStatus::Started
                    && id.starts_with("tool_call:")
            )
        })
        .expect("started tool call marker");
    let completed_tool_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    id,
                    activity_kind,
                    status,
                    ..
                } if activity_kind == "tool_call"
                    && *status == TurnActivityStatus::Completed
                    && id.starts_with("tool_call:")
            )
        })
        .expect("completed tool call marker");
    assert!(
        started_tool_position < completed_tool_position,
        "tool call should appear as started before it completes"
    );

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        replay.iter().any(|item| {
            item.kind == ConversationItemKind::ToolCall
                && item.status == ConversationItemStatus::Completed
                && item.payload_json["activity_kind"] == "tool_call"
                && item.payload_json["metadata"]["action"]["name"] == "search_memory"
        }),
        "expected replayed tool call item, got {replay:?}"
    );
    let tool_position = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::ToolCall)
        .expect("tool call item");
    let assistant_position = replay
        .iter()
        .position(|item| item.kind == ConversationItemKind::AssistantText)
        .expect("assistant item");
    assert!(
        tool_position < assistant_position,
        "tool call should replay before assistant text"
    );
}

#[tokio::test]
async fn runtime_actor_persists_provider_tool_items_before_turn_failure() {
    let handle = test_runtime_handle(fake_codex_provider_with_tool_item_then_failure()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await;
    assert!(matches!(result, Err(DaemonError::Provider(_))));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::AgentStatusChanged {
                conversation_id: id,
                status: AgentStatus::Error,
            } if id == &conversation_id
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem {
                conversation_id: id,
                item_id,
                turn_id: Some(_),
                item,
                ..
            } if id == &conversation_id
                && item_id.starts_with("item:")
                && matches!(
                    item.as_ref(),
                    TurnTranscriptItem::Activity {
                        activity_kind,
                        status: TurnActivityStatus::Completed,
                        title,
                        ..
                    } if activity_kind == "tool_call"
                        && title == "Tool call: search_memory"
                )
        )
    }));
    let items = transcript_items_from_events(events);
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));
}

#[tokio::test]
async fn runtime_actor_executes_search_memory_as_local_tool_result() {
    let handle = test_runtime_handle(fake_codex_provider_with_search_memory_continuation()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "Please remember I'm a big fan of trains".to_string(),
    )
    .await
    .expect("seed turn");

    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("search turn");
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Completed,
            title,
            metadata,
            ..
        } if activity_kind == "tool_result"
            && title == "Tool result: search_memory"
            && metadata["action"]["success"] == true
            && metadata["action"]["payload"]["memories"]
                .as_array()
                .is_some_and(|memories| memories.is_empty())
            && metadata["action"]["payload"]["unavailable"]["reason"]
                == "graph_retrieval_pending"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I found your train memory."
    )));
    handle.shutdown().await;
}

#[tokio::test]
async fn search_memory_tool_returns_structured_unavailable_result() {
    let handle = test_runtime_handle(fake_codex_provider_with_search_memory_continuation()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                metadata,
                ..
            } if activity_kind == "tool_result" => Some(&metadata["action"]["payload"]),
            _ => None,
        })
        .expect("tool result payload");
    assert_eq!(
        payload["omissions"],
        json!([{ "reason": "graph_retrieval_unavailable" }])
    );
    assert_eq!(payload["unavailable"]["reason"], "graph_retrieval_pending");
    assert!(
        payload["memories"]
            .as_array()
            .is_some_and(|memories| memories.is_empty())
    );
}

#[tokio::test]
async fn search_memory_tool_invalid_arguments_are_failed_tool_result() {
    let handle =
        test_runtime_handle(fake_codex_provider_with_invalid_search_memory_tool_item()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Use your tool".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let payload = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                metadata,
                ..
            } if activity_kind == "tool_result" => Some(&metadata["action"]["payload"]),
            _ => None,
        })
        .expect("failed tool result payload");
    assert_eq!(payload["error"], "unsupported purpose: dump_everything");
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
                if !matches!(item.as_ref(), TurnTranscriptItem::UserText { .. }) =>
            {
                Some(*item)
            }
            TurnStreamEvent::ConversationItem { .. }
            | TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .collect()
}

fn memory_extraction_event_index(
    events: &[TurnStreamEvent],
    expected_status: TurnActivityStatus,
    expected_title: &str,
) -> Option<usize> {
    events.iter().position(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem { item, .. }
                if matches!(
                    item.as_ref(),
                    TurnTranscriptItem::Activity {
                        activity_kind,
                        status,
                        title,
                        ..
                    } if activity_kind == "memory_extraction"
                        && *status == expected_status
                        && title == expected_title
                )
        )
    })
}

fn memory_extraction_event_item_id(
    events: &[TurnStreamEvent],
    expected_status: TurnActivityStatus,
    expected_title: &str,
) -> Option<String> {
    events.iter().find_map(|event| match event {
        TurnStreamEvent::ConversationItem { item_id, item, .. }
            if matches!(
                item.as_ref(),
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status,
                    title,
                    ..
                } if activity_kind == "memory_extraction"
                    && *status == expected_status
                    && title == expected_title
            ) =>
        {
            Some(item_id.clone())
        }
        TurnStreamEvent::ConversationItem { .. }
        | TurnStreamEvent::AssistantTextDelta { .. }
        | TurnStreamEvent::AgentStatusChanged { .. } => None,
    })
}

fn memory_proposals_card_event_index(events: &[TurnStreamEvent]) -> Option<usize> {
    events.iter().position(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem { item, .. }
                if matches!(
                    item.as_ref(),
                    TurnTranscriptItem::A2uiCard { schema, .. } if schema == "memory_proposals"
                )
        )
    })
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

async fn test_runtime_handle(provider: FakeCodexProvider) -> CodexRuntimeHandle {
    test_runtime_handle_with_store(provider).await.0
}

async fn test_runtime_handle_with_store(
    provider: FakeCodexProvider,
) -> (CodexRuntimeHandle, crate::NoemaStore) {
    let home = tempfile::tempdir().expect("temp noema home");
    let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
    let store = crate::NoemaStore::open(&crate::StoreConfig::from_paths(&paths))
        .await
        .expect("store");
    std::mem::forget(home);
    let handle = CodexRuntimeHandle::spawn_with_provider(Arc::new(provider), store.clone())
        .await
        .expect("runtime");
    (handle, store)
}

#[derive(Debug, Clone)]
struct FakeCodexProvider {
    scenario: FakeCodexScenario,
}

#[derive(Debug, Clone, Copy)]
enum FakeCodexScenario {
    Simple,
    RestartContext,
    TurnError,
    ToolItem,
    ToolItemThenFailure,
    InvalidSearchMemory,
    SearchMemoryContinuation,
    MemoryExtraction,
}

impl FakeCodexProvider {
    fn new(scenario: FakeCodexScenario) -> Self {
        Self { scenario }
    }

    fn generate_response(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, ProviderError> {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| "fake-model".to_string());
        let GenerateInput::Text(input) = request.input;
        let instructions = request.instructions.unwrap_or_default();
        let output = match self.scenario {
            FakeCodexScenario::Simple => assistant_with_no_memories("fake answer"),
            FakeCodexScenario::RestartContext => {
                let saw_context = instructions
                    .contains("Recent durable transcript from embedded Noema store:")
                    && instructions.contains("User: first durable question")
                    && instructions.contains("Noema: fake answer")
                    && input.contains("second durable question");
                assistant_with_no_memories(if saw_context {
                    "saw durable context"
                } else {
                    "fake answer"
                })
            }
            FakeCodexScenario::TurnError => {
                return Err(ProviderError::ApiError {
                    status: 500,
                    message: "turn failed".to_string(),
                    request_id: None,
                });
            }
            FakeCodexScenario::ToolItem => vec![
                search_memory_tool_call("call_1", json!({"arguments": {"query": "trains"}})),
                GenerateOutputItem::AssistantText {
                    text: "fake answer".to_string(),
                },
                GenerateOutputItem::MemoryProposals { proposals: vec![] },
            ],
            FakeCodexScenario::ToolItemThenFailure => {
                return Err(ProviderError::PartialResponse {
                    provider: "codex".to_string(),
                    model,
                    message: "tool failed later".to_string(),
                    output: vec![search_memory_tool_call(
                        "call_1",
                        json!({"arguments": {"query": "trains"}}),
                    )],
                });
            }
            FakeCodexScenario::InvalidSearchMemory => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("invalid tool result received")
                } else {
                    vec![
                        search_memory_tool_call(
                            "call_bad",
                            json!({"arguments": {"query": "trains", "purpose": "dump_everything"}}),
                        ),
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                }
            }
            FakeCodexScenario::SearchMemoryContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "I found your train memory.".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Noema found Kevin's train memory.",
                                "memory_type": "note",
                                "title": "Train memory recall",
                                "confidence": 0.72,
                                "sensitivity": "normal",
                                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                "retrieval_hints": {"topics": ["trains"], "keywords": ["train memory"], "summary": "Noema found Kevin's train memory."},
                                "risk_flags": [],
                                "evidence_excerpt": "I found your train memory."
                            }))],
                        },
                    ]
                } else if input.contains("Please remember I'm a big fan of trains") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "fake answer".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![train_preference_proposal()],
                        },
                    ]
                } else if input.contains("What do you remember about trains?") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "Searching memory.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::MemoryExtraction => memory_extraction_output(&input),
        };

        Ok(GenerateResponse {
            output,
            provider: "codex".to_string(),
            model,
            response_id: Some("fake-response".to_string()),
            usage: None,
        })
    }
}

impl super::runtime::RuntimeModelProvider for FakeCodexProvider {
    fn generate<'a>(
        &'a self,
        request: GenerateRequest,
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move { self.generate_response(request) })
    }

    fn generate_streaming<'a>(
        &'a self,
        request: GenerateRequest,
        on_event: &'a mut (dyn FnMut(GenerateStreamEvent) + Send),
    ) -> Pin<Box<dyn Future<Output = Result<GenerateResponse, ProviderError>> + Send + 'a>> {
        Box::pin(async move {
            let response = self.generate_response(request)?;
            for (index, output) in response.output.iter().enumerate() {
                match output {
                    GenerateOutputItem::AssistantText { text } => {
                        let mut chunk = String::new();
                        for character in text.chars() {
                            chunk.push(character);
                            if chunk.chars().count() == 4 {
                                on_event(GenerateStreamEvent::AssistantTextDelta { delta: chunk });
                                chunk = String::new();
                            }
                        }
                        if !chunk.is_empty() {
                            on_event(GenerateStreamEvent::AssistantTextDelta { delta: chunk });
                        }
                    }
                    GenerateOutputItem::MemoryProposals { proposals } if !proposals.is_empty() => {
                        on_event(GenerateStreamEvent::MemoryProposalsStarted);
                    }
                    GenerateOutputItem::ToolCall { name, .. } => {
                        on_event(GenerateStreamEvent::ToolCallStarted {
                            output_index: index,
                            name: name.clone(),
                        });
                    }
                    GenerateOutputItem::MemoryProposals { .. }
                    | GenerateOutputItem::ToolResult { .. }
                    | GenerateOutputItem::ApprovalRequest { .. }
                    | GenerateOutputItem::ApprovalResult { .. }
                    | GenerateOutputItem::Structured { .. } => {}
                }
            }
            Ok(response)
        })
    }
}

fn assistant_with_no_memories(text: &str) -> Vec<GenerateOutputItem> {
    vec![
        GenerateOutputItem::AssistantText {
            text: text.to_string(),
        },
        GenerateOutputItem::MemoryProposals { proposals: vec![] },
    ]
}

fn search_memory_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        name: "search_memory".to_string(),
        payload,
    }
}

fn proposal(value: serde_json::Value) -> crate::ExtractorMemoryProposal {
    serde_json::from_value(value).expect("valid fake memory proposal")
}

fn train_preference_proposal() -> crate::ExtractorMemoryProposal {
    proposal(json!({
        "content": "Kevin is a big fan of trains.",
        "memory_type": "preference",
        "title": "Train enthusiasm",
        "confidence": 0.92,
        "sensitivity": "normal",
        "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
        "retrieval_hints": {"topics": ["interests"], "keywords": ["trains"], "summary": "Kevin is a big fan of trains."},
        "risk_flags": [],
        "evidence_excerpt": "I'm a big fan of trains"
    }))
}

fn memory_extraction_output(input: &str) -> Vec<GenerateOutputItem> {
    if input.contains("Noema's memory consolidation comparator") {
        let existing_memory_id =
            first_memory_id_from_consolidation_prompt(input).expect("existing memory id");
        let decision = if input.contains("Kevin hates ice cream.") {
            json!({
                "decision": "conflict",
                "existing_memory_id": existing_memory_id,
                "confidence": 0.93,
                "rationale": "opposite ice cream preference",
            })
        } else {
            json!({
                "decision": "reinforce",
                "existing_memory_id": existing_memory_id,
                "confidence": 0.92,
                "rationale": "same ice cream preference",
            })
        };
        return vec![GenerateOutputItem::AssistantText {
            text: serde_json::to_string(&decision).expect("semantic decision json"),
        }];
    }

    if input.contains("ordinary-chat memory proposal extractor") {
        let proposal = if input.contains("Alice prefers decaf.") {
            proposal(json!({
                "content": "Alice prefers decaf.",
                "memory_type": "preference",
                "title": "Alice decaf preference",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [{"id": null, "kind": "human", "name": "Alice", "role": "about"}],
                "retrieval_hints": {"topics": ["people"], "keywords": ["Alice", "decaf"], "summary": "Alice prefers decaf."},
                "risk_flags": [],
                "evidence_excerpt": "Alice prefers decaf."
            }))
        } else if input.contains("I like ice cream.") {
            proposal(json!({
                "content": "Kevin likes ice cream.",
                "memory_type": "preference",
                "title": "Ice cream preference",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin likes ice cream."},
                "risk_flags": [],
                "evidence_excerpt": "I like ice cream."
            }))
        } else {
            proposal(json!({
                "content": "Kevin prefers automatic memory extraction in chat.",
                "memory_type": "preference",
                "title": "Automatic memory extraction preference",
                "confidence": 0.91,
                "sensitivity": "normal",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["memory"], "keywords": ["automatic memory extraction", "chat"], "summary": "Kevin prefers automatic memory extraction in chat."},
                "risk_flags": [],
                "evidence_excerpt": "I prefer automatic memory extraction in chat."
            }))
        };
        return vec![GenerateOutputItem::AssistantText {
            text: serde_json::to_string(&json!({"proposals": [proposal]})).expect("extractor json"),
        }];
    }

    if input.contains("Ice cream is one of my favorite desserts.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "Kevin enjoys ice cream desserts.",
                    "memory_type": "preference",
                    "title": "Ice cream dessert preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream", "dessert"], "summary": "Kevin enjoys ice cream desserts."},
                    "risk_flags": [],
                    "evidence_excerpt": "Ice cream is one of my favorite desserts."
                }))],
            },
        ];
    }

    if input.contains("I hate ice cream.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "Kevin hates ice cream.",
                    "memory_type": "preference",
                    "title": "Ice cream dislike",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin hates ice cream."},
                    "risk_flags": ["contradiction"],
                    "evidence_excerpt": "I hate ice cream."
                }))],
            },
        ];
    }

    if input.contains("I prefer same-call memory proposals.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "Kevin prefers same-call memory proposals.",
                    "memory_type": "preference",
                    "title": "Same-call memory proposal preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["memory"], "keywords": ["same-call memory proposals"], "summary": "Kevin prefers same-call memory proposals."},
                    "risk_flags": [],
                    "evidence_excerpt": "I prefer same-call memory proposals."
                }))],
            },
        ];
    }

    if input.contains("Please remember I'm a big fan of trains") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![train_preference_proposal()],
            },
        ];
    }

    assistant_with_no_memories("fake answer")
}

fn first_memory_id_from_consolidation_prompt(input: &str) -> Option<String> {
    let payload = input.split("Input JSON payload:").nth(1)?.trim();
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    value["existing_memories"]
        .as_array()?
        .first()?
        .get("memory_id")?
        .as_str()
        .map(str::to_string)
}

fn fake_codex_provider_with_turn_error() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::TurnError)
}

fn fake_codex_provider() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::Simple)
}

fn fake_codex_provider_with_restart_context_check() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::RestartContext)
}

fn fake_codex_provider_with_tool_item() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::ToolItem)
}

fn fake_codex_provider_with_tool_item_then_failure() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::ToolItemThenFailure)
}

fn fake_codex_provider_with_invalid_search_memory_tool_item() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::InvalidSearchMemory)
}

fn fake_codex_provider_with_search_memory_continuation() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::SearchMemoryContinuation)
}

fn fake_codex_provider_with_memory_extraction() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::MemoryExtraction)
}
