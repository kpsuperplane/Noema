use super::*;
use super::{
    memory_pipeline::{
        ConversationMemoryContext, explicit_memory_claim_candidate, explicit_memory_content,
        infer_chat_sensitivity, provider_memory_claim_candidate, provider_memory_write_proposal,
    },
    protocol::TurnStreamEvent,
    runtime::CodexRuntimeHandle,
    server::bind_listener,
};
use crate::{
    EntityType,
    memory::{ClaimRetrievalRequest, MemoryStatus, Sensitivity, UseMode},
    provider::{
        GenerateInput, GenerateOutputItem, GenerateRequest, GenerateResponse, GenerateStreamEvent,
        ProviderError,
    },
    {ConversationItemKind, ConversationItemStatus, ReplayMode},
};
use serde::Deserialize;
use serde_json::json;
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    process::Command,
    sync::{Arc, Mutex},
};
use surrealdb::types::SurrealValue;
use tokio::sync::mpsc;

const RESTART_CONTEXT_TEST_PHASE_ENV: &str = "NOEMA_RESTART_CONTEXT_TEST_PHASE";
const RESTART_CONTEXT_TEST_HOME_ENV: &str = "NOEMA_RESTART_CONTEXT_TEST_HOME";
const RESTART_CONTEXT_TEST_CONVERSATION_FILE: &str = "restart_context_conversation_id";

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

fn answer_claim_request() -> ClaimRetrievalRequest {
    ClaimRetrievalRequest {
        requesting_agent_id: "agent:primary".to_string(),
        active_human_ids: vec!["human:local".to_string()],
        active_object_ids: Vec::new(),
        use_mode: UseMode::Answer,
        explicit_memory_request: true,
        sensitivity_ceiling: Sensitivity::Normal,
        approved_secret_access: false,
    }
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
async fn runtime_handle_generate_once_uses_provider_without_conversation() {
    let handle = test_runtime_handle(fake_codex_provider()).await;

    let response = handle
        .generate_once(GenerateRequest::text("hello"))
        .await
        .expect("generate once");
    handle.shutdown().await;

    assert_eq!(response.assistant_text(), "fake answer");
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
    if let Ok(phase) = std::env::var(RESTART_CONTEXT_TEST_PHASE_ENV) {
        let home = PathBuf::from(
            std::env::var(RESTART_CONTEXT_TEST_HOME_ENV).expect("restart test home env"),
        );
        match phase.as_str() {
            "write" => restart_context_write_phase(&home).await,
            "read" => restart_context_read_phase(&home).await,
            other => panic!("unknown restart context test phase: {other}"),
        }
        return;
    }

    let home = tempfile::tempdir().expect("temp noema home");
    run_restart_context_child_phase("write", home.path());
    run_restart_context_child_phase("read", home.path());
}

#[tokio::test]
async fn runtime_prompt_includes_unnamed_agent_onboarding() {
    let handle = test_runtime_handle(fake_codex_provider_with_identity_prompt_check()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw unnamed identity"
    )));
}

#[tokio::test]
async fn start_primary_conversation_generates_initial_name_onboarding_message() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_initial_name_onboarding()).await;

    let conversation_id = handle
        .start_primary_conversation(None, None)
        .await
        .expect("primary conversation")
        .conversation_id;
    handle.shutdown().await;

    let items = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        !items
            .iter()
            .any(|item| item.kind == ConversationItemKind::UserText),
        "initial onboarding should not fake a user message: {items:?}"
    );
    assert!(items.iter().any(|item| {
        item.kind == ConversationItemKind::AssistantText
            && item.content_text.as_deref()
                == Some(
                    "Hey 👋 I'm your Noema personal agent, here to help you think, plan, make, untangle, or whatever keeps your momentum going in life. Before we dive in, give me a name!",
                )
    }));
}

#[tokio::test]
async fn failed_initial_name_onboarding_recomputes_turn_index_on_retry() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_turn_error()).await;

    assert!(handle.start_primary_conversation(None, None).await.is_err());
    assert!(handle.start_primary_conversation(None, None).await.is_err());
    handle.shutdown().await;

    let conversation_id = store
        .get_or_create_primary_conversation("human:local", None, None)
        .await
        .expect("primary conversation")
        .conversation_id;
    assert_eq!(
        store
            .next_conversation_turn_index(&conversation_id)
            .await
            .expect("next turn index"),
        3
    );
}

fn run_restart_context_child_phase(phase: &str, home: &std::path::Path) {
    let output = Command::new(std::env::current_exe().expect("current test binary"))
        .arg("daemon::tests::runtime_primary_conversation_sends_recent_durable_context_after_restart")
        .arg("--exact")
        .env(RESTART_CONTEXT_TEST_PHASE_ENV, phase)
        .env(RESTART_CONTEXT_TEST_HOME_ENV, home)
        .output()
        .expect("run restart context test phase");
    assert!(
        output.status.success(),
        "restart context {phase} phase failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn restart_context_write_phase(home: &std::path::Path) {
    let paths = crate::NoemaPaths::from_noema_home(home).expect("paths");
    let config = crate::StoreConfig::from_paths(&paths);
    let first_store = crate::NoemaStore::open(&config)
        .await
        .expect("open first store");
    let first_handle = CodexRuntimeHandle::spawn_with_provider(
        Arc::new(fake_codex_provider_with_restart_context_check()),
        first_store.clone(),
    )
    .await
    .expect("first runtime");
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
    std::fs::write(
        home.join(RESTART_CONTEXT_TEST_CONVERSATION_FILE),
        &first_conversation_id,
    )
    .expect("write restart conversation id");
    first_store.close().await.expect("close first store");
}

async fn restart_context_read_phase(home: &std::path::Path) {
    let first_conversation_id =
        std::fs::read_to_string(home.join(RESTART_CONTEXT_TEST_CONVERSATION_FILE))
            .expect("read restart conversation id");
    let paths = crate::NoemaPaths::from_noema_home(home).expect("paths");
    let config = crate::StoreConfig::from_paths(&paths);
    let reopened_store = crate::NoemaStore::open(&config)
        .await
        .expect("reopen store");
    let second_handle = CodexRuntimeHandle::spawn_with_provider(
        Arc::new(fake_codex_provider_with_restart_context_check()),
        reopened_store.clone(),
    )
    .await
    .expect("second runtime");
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

    let replay = reopened_store
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
    reopened_store.close().await.expect("close reopened store");
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

#[test]
fn explicit_claim_candidate_falls_back_for_punctuation_only_relation_objects() {
    for content in ["I like !!!", "I like ---", "I like …", "I like  - … !!!  "] {
        let candidate = explicit_memory_claim_candidate(content, "item:test".to_string());
        let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
        let expected_fact = if normalized.ends_with(['.', '!', '?']) {
            normalized.clone()
        } else {
            format!("{normalized}.")
        };

        assert_eq!(candidate.predicate_id, "has_note", "{content}");
        assert_eq!(candidate.fact, expected_fact, "{content}");
        assert_eq!(candidate.object.canonical_name, normalized, "{content}");
        assert_eq!(
            candidate.retrieval_hints["keywords"],
            json!([normalized]),
            "{content}"
        );
    }
}

#[test]
fn explicit_claim_candidate_normalizes_relation_object_whitespace() {
    let candidate = explicit_memory_claim_candidate("I LIKE   ICE CREAM", "item:test".to_string());

    assert_eq!(candidate.predicate_id, "likes");
    assert_eq!(candidate.fact, "Kevin likes ice cream.");
    assert_eq!(candidate.object.canonical_name, "ice cream");
    assert_eq!(candidate.retrieval_hints["keywords"], json!(["ice cream"]));
}

#[test]
fn explicit_claim_candidate_canonicalizes_dislikes() {
    let candidate = explicit_memory_claim_candidate("I hate   ICE CREAM", "item:test".to_string());

    assert_eq!(candidate.predicate_id, "dislikes");
    assert_eq!(candidate.fact, "Kevin dislikes ice cream.");
    assert_eq!(candidate.object.canonical_name, "ice cream");
    assert_eq!(candidate.retrieval_hints["keywords"], json!(["ice cream"]));
}

#[test]
fn explicit_and_provider_dislikes_share_claim_shape() {
    let explicit = explicit_memory_claim_candidate("I hate ice cream", "item:explicit".to_string());
    let provider = provider_memory_claim_candidate(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "I hate ice cream",
                "memory_type": "preference",
                "title": "Ice cream dislike",
                "confidence": 0.81,
                "sensitivity": "normal",
                "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin dislikes ice cream."},
                "risk_flags": [],
                "evidence_excerpt": "I hate ice cream"
            })),
            status: MemoryStatus::Candidate,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "I hate ice cream".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_eq!(provider.subject.entity_id, explicit.subject.entity_id);
    assert_eq!(provider.predicate_id, explicit.predicate_id);
    assert_eq!(provider.fact, explicit.fact);
    assert_eq!(provider.object.entity_id, explicit.object.entity_id);
}

#[test]
fn provider_note_fallback_uses_content_for_object_identity() {
    let explicit =
        explicit_memory_claim_candidate("Garage keypad code is 1234.", "item:explicit".to_string());
    let provider = provider_memory_claim_candidate(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "Garage keypad code is 1234.",
                "memory_type": "note",
                "title": "Garage code",
                "confidence": 0.81,
                "sensitivity": "secret",
                "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["home"], "keywords": ["garage"], "summary": "Garage keypad code is 1234."},
                "risk_flags": [],
                "evidence_excerpt": "Garage keypad code is 1234."
            })),
            status: MemoryStatus::Candidate,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "Garage keypad code is 1234.".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_eq!(explicit.predicate_id, "has_note");
    assert_eq!(provider.predicate_id, "has_note");
    assert_eq!(provider.subject.entity_id, "human:local");
    assert_eq!(provider.object.entity_id, explicit.object.entity_id);
    assert_eq!(
        provider.object.canonical_name,
        explicit.object.canonical_name
    );
    assert_eq!(provider.object.canonical_name, "Garage keypad code is 1234");
}

#[test]
fn note_fallback_object_id_uses_opaque_content_fingerprint() {
    let first =
        explicit_memory_claim_candidate("Garage keypad code is 1234!", "item:first".to_string());
    let second =
        explicit_memory_claim_candidate("Garage keypad code is 1234.", "item:second".to_string());

    assert_eq!(first.predicate_id, "has_note");
    assert_eq!(first.object.entity_id, second.object.entity_id);
    assert!(first.object.entity_id.starts_with("concept:claim_object_"));
    assert!(
        !first
            .object
            .entity_id
            .to_ascii_lowercase()
            .contains("garage")
    );
    assert!(!first.object.entity_id.contains("1234"));
    assert_eq!(first.object.canonical_name, "Garage keypad code is 1234");
}

#[test]
fn third_party_same_name_provider_subject_stays_non_local() {
    let provider = provider_memory_claim_candidate(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "Kevin prefers decaf.",
                "memory_type": "preference",
                "title": "Kevin decaf",
                "confidence": 0.81,
                "sensitivity": "normal",
                "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["people"], "keywords": ["Kevin", "decaf"], "summary": "Kevin prefers decaf."},
                "risk_flags": [],
                "evidence_excerpt": "My friend Kevin prefers decaf."
            })),
            status: MemoryStatus::Candidate,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "My friend Kevin prefers decaf.".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_ne!(provider.subject.entity_id, "human:local");
    assert_eq!(provider.subject.entity_id, "human:kevin");
}

#[test]
fn named_same_name_provider_subject_stays_non_local() {
    let provider = provider_memory_claim_candidate(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "Kevin prefers decaf.",
                "memory_type": "preference",
                "title": "Kevin decaf",
                "confidence": 0.81,
                "sensitivity": "normal",
                "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["people"], "keywords": ["Kevin", "decaf"], "summary": "Kevin prefers decaf."},
                "risk_flags": [],
                "evidence_excerpt": "I talked to Kevin and he prefers decaf."
            })),
            status: MemoryStatus::Candidate,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "I talked to Kevin and he prefers decaf.".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_ne!(provider.subject.entity_id, "human:local");
    assert_eq!(provider.subject.entity_id, "human:kevin");
}

#[test]
fn literal_local_human_provider_subject_maps_to_local() {
    let provider = provider_memory_claim_candidate(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "The local human prefers local models.",
                "memory_type": "preference",
                "title": "Local model preference",
                "confidence": 0.81,
                "sensitivity": "normal",
                "subjects": [{"id": null, "kind": "human", "name": "local human", "role": "about"}],
                "retrieval_hints": {"topics": ["models"], "keywords": ["local models"], "summary": "The local human prefers local models."},
                "risk_flags": [],
                "evidence_excerpt": "I prefer local models."
            })),
            status: MemoryStatus::Active,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "I prefer local models.".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_eq!(provider.subject.entity_id, "human:local");
}

#[test]
fn provider_subject_uses_first_subject_without_local_participant_override() {
    let provider = provider_memory_claim_candidate(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "Noema uses graph memory.",
                "memory_type": "project",
                "title": "Graph memory",
                "confidence": 0.81,
                "sensitivity": "normal",
                "subjects": [
                    {"id": "project:noema", "kind": "project", "name": "Noema", "role": "about"},
                    {"id": "human:local", "kind": "human", "name": "Kevin", "role": "owner"}
                ],
                "retrieval_hints": {"topics": ["memory"], "keywords": ["Noema", "graph memory"], "summary": "Noema uses graph memory."},
                "risk_flags": [],
                "evidence_excerpt": "Noema uses graph memory."
            })),
            status: MemoryStatus::Candidate,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "Noema uses graph memory.".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_eq!(provider.subject.entity_id, "project:noema");
    assert_eq!(provider.subject.entity_type, EntityType::Project);
}

#[test]
fn provider_empty_retrieval_hints_fall_back_to_deterministic_hints() {
    let provider = provider_memory_claim_candidate(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "The user loves planes.",
                "memory_type": "preference",
                "title": "Plane preference",
                "confidence": 0.81,
                "sensitivity": "normal",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": [], "keywords": [], "summary": ""},
                "risk_flags": [],
                "evidence_excerpt": "I love planes."
            })),
            status: MemoryStatus::Candidate,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "I love planes.".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_eq!(provider.retrieval_hints["keywords"], json!(["planes"]));
    assert_eq!(
        provider.retrieval_hints["summary"],
        json!("Kevin likes planes.")
    );
}

#[test]
fn provider_write_proposal_risk_flags_use_stable_snake_case_labels() {
    let write_proposal = provider_memory_write_proposal(
        &crate::memory::extraction::ValidatedMemoryProposal {
            proposal: proposal(json!({
                "content": "The user has a temporary secret.",
                "memory_type": "note",
                "title": "Temporary secret",
                "confidence": 0.81,
                "sensitivity": "secret",
                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                "retrieval_hints": {"topics": ["security"], "keywords": ["temporary secret"], "summary": "The user has a temporary secret."},
                "risk_flags": ["security_risk", "temporary_context"],
                "evidence_excerpt": "This temporary secret matters."
            })),
            status: MemoryStatus::Candidate,
        },
        &ConversationMemoryContext {
            conversation_id: "conversation:test".to_string(),
            turn_id: "turn:test".to_string(),
            turn_index: 1,
            user_item_id: "item:provider".to_string(),
            assistant_item_id: None,
            assistant_items: Vec::new(),
            user_content: "This temporary secret matters.".to_string(),
            cwd: None,
        },
        0,
        "ordinary_chat",
    );

    assert_eq!(
        write_proposal.risk_flags,
        vec!["security_risk".to_string(), "temporary_context".to_string()]
    );
    assert_eq!(
        write_proposal.metadata["risk_flags"],
        json!(["security_risk", "temporary_context"])
    );
}

#[tokio::test]
async fn explicit_remember_creates_claim_with_source_evidence() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

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
    let claim_id = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                summary: Some(summary),
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Explicit memory saved"
                && summary == "saved graph claim"
                && metadata["trigger"] == "explicit_remember"
                && metadata["predicate_id"] == "prefers" =>
            {
                metadata["claim_id"].as_str().map(str::to_string)
            }
            _ => None,
        })
        .expect("completed explicit memory activity with claim id");
    assert!(claim_id.starts_with("claim:"));
    assert_no_failed_memory_extraction(&items);
    let claims = store
        .retrieve_claims(&answer_claim_request(), "CLI memory inspection", 8)
        .await
        .expect("retrieve explicit claim");
    assert!(
        claims.included.iter().any(|claim| {
            claim.claim_id == claim_id
                && claim.fact == "Kevin prefers CLI memory inspection."
                && claim.predicate_id == "prefers"
        }),
        "expected explicit claim in retrieval, got {claims:?}"
    );
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
async fn explicit_remember_write_failure_suppresses_generic_unavailable_activity() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
    store
        .db()
        .query("DELETE predicates WHERE predicate_id = 'likes';")
        .await
        .expect("delete likes predicate")
        .check()
        .expect("delete likes predicate check");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "remember: I like trains.".to_string(),
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
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Explicit memory save failed"
                && metadata["trigger"] == "explicit_remember"
        )
    }));
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Failed,
                    title,
                    metadata,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Memory extraction unavailable"
                    && metadata["trigger"] == "ordinary_chat"
            )
        }),
        "explicit write failure should not emit generic unavailable activity: {items:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn repeated_explicit_memory_reinforces_one_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    let _first_items = collect_turn(
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
    let claim_activity = second_items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction" && title == "Explicit memory saved" => {
                Some(metadata)
            }
            _ => None,
        })
        .expect("second explicit memory activity");
    assert_eq!(claim_activity["evidence_count"], 2);
    assert_eq!(
        claim_activity["claim_outcomes"][0]["outcome"],
        json!("reinforced")
    );
    assert_eq!(
        claim_activity["claim_outcomes"][0]["fact_preview"],
        json!("Kevin likes ice cream.")
    );
    assert_no_failed_memory_extraction(&second_items);

    let claims = store
        .retrieve_claims(&answer_claim_request(), "ice cream", 8)
        .await
        .expect("retrieve reinforced claim");
    assert_eq!(claims.included.len(), 1, "expected one claim: {claims:?}");
    assert_eq!(claims.included[0].fact, "Kevin likes ice cream.");
    assert_eq!(claims.included[0].predicate_id, "likes");
    handle.shutdown().await;
}

#[tokio::test]
async fn explicit_memory_saved_activity_includes_claim_outcome() {
    let handle = test_runtime_handle(fake_codex_provider_with_memory_extraction()).await;
    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");

    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "remember: I like planes.".to_string(),
    )
    .await
    .expect("turn");

    let metadata = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction" && title == "Explicit memory saved" => {
                Some(metadata)
            }
            _ => None,
        })
        .expect("explicit memory activity metadata");

    assert_eq!(
        metadata["claim_outcomes"][0]["fact_preview"],
        json!("Kevin likes planes.")
    );
    assert_eq!(metadata["claim_outcomes"][0]["outcome"], json!("created"));
    assert_eq!(
        metadata["claim_outcomes"][0]["sensitivity"],
        json!("normal")
    );
    handle.shutdown().await;
}

fn assert_no_failed_memory_extraction(items: &[TurnTranscriptItem]) {
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Failed,
                    ..
                } if activity_kind == "memory_extraction"
            )
        }),
        "explicit memory turn should not emit any failed memory_extraction activity: {items:?}"
    );
}

#[tokio::test]
async fn runtime_actor_persists_ordinary_provider_memory_as_graph_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

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
    let claim_id = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                summary: Some(summary),
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && summary == "saved 1 graph claim"
                && metadata["source"] == "provider_structured_output"
                && metadata["proposal_count"] == 1
                && metadata["created_claim_count"] == 1
                && metadata["reinforced_claim_count"] == 0 =>
            {
                metadata["claim_ids"][0].as_str().map(str::to_string)
            }
            _ => None,
        })
        .expect("completed provider memory activity with claim id");
    assert_no_failed_memory_extraction(&items);
    let claims = store
        .retrieve_claims(&answer_claim_request(), "automatic memory extraction", 8)
        .await
        .expect("retrieve ordinary provider claim");
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::A2uiCard { schema, .. } if schema == "memory_proposals"
            )
        }),
        "provider proposals should not emit a memory_proposals card: {items:?}"
    );
    assert!(
        claims.included.iter().any(|claim| {
            claim.claim_id == claim_id
                && claim.fact == "Kevin prefers automatic memory extraction in chat."
                && claim.predicate_id == "prefers"
        }),
        "expected ordinary provider claim in retrieval, got {claims:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_mislabelled_secret_stays_candidate_and_unretrievable() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_mislabelled_secret_memory()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let (result, events) = collect_turn_events(
        &handle,
        conversation.conversation_id,
        "My API key is sk-testSecretToken123456789.".to_string(),
    )
    .await;
    result.expect("turn");
    let claim_id = memory_persisted_claim_id(&events);

    let stored = claim_status_and_sensitivity(&store, &claim_id).await;
    assert_eq!(stored.status, "candidate");
    assert_eq!(stored.sensitivity, "secret");

    let claims = store
        .retrieve_claims(&answer_claim_request(), "api key", 8)
        .await
        .expect("retrieve api key claim");
    assert!(claims.included.is_empty(), "unexpected claims: {claims:?}");
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_malformed_canonicalizer_response_fails_without_fallback_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_malformed_canonicalizer()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I prefer malformed canonicalizer tests.".to_string(),
    )
    .await
    .expect("turn should complete despite malformed canonicalizer response");

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
                && title == "Memory persistence failed"
                && summary.contains("memory canonicalization failed")
                && metadata["failed_proposal_count"] == 1
                && metadata["failed_proposals"][0]["error"]
                    .as_str()
                    .is_some_and(|error| error.contains("memory canonicalization failed"))
        )
    }));
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Memory persisted"
            )
        }),
        "malformed canonicalizer response should not persist fallback memory: {items:?}"
    );

    let claims = store
        .retrieve_claims(&answer_claim_request(), "malformed canonicalizer tests", 8)
        .await
        .expect("retrieve malformed canonicalizer claim");
    assert!(claims.included.is_empty(), "unexpected claims: {claims:?}");
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_mismatched_canonical_entity_fails_without_fallback_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_mismatched_canonical_entity())
            .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I prefer canonical entity validation.".to_string(),
    )
    .await
    .expect("turn should complete despite mismatched canonical entity");

    assert_eq!(assistant_text(&items), "fake answer");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persistence failed"
                && metadata["source"] == "provider_structured_output"
                && metadata["proposal_count"] == 1
                && metadata["failed_proposal_count"] == 1
                && metadata["failed_proposals"][0]["error"]
                    .as_str()
                    .is_some_and(|error| error.contains("invalid canonical entity metadata"))
        )
    }));
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Memory persisted"
            )
        }),
        "invalid canonical entity metadata should not persist fallback memory: {items:?}"
    );

    let claims = store
        .retrieve_claims(&answer_claim_request(), "canonical entity validation", 8)
        .await
        .expect("retrieve invalid canonical entity claim");
    assert!(claims.included.is_empty(), "unexpected claims: {claims:?}");

    if let Some(local_human) = maybe_entity_row(&store, "human:local").await {
        assert_eq!(local_human.entity_type, "human");
        assert_eq!(local_human.canonical_name, "Local human");
    }
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_unknown_promoted_predicate_fails_before_graph_write() {
    let (handle, _store) =
        test_runtime_handle_with_store(fake_codex_provider_with_unknown_canonical_predicate())
            .await;

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
    .expect("turn should complete despite unknown promoted predicate");

    assert_eq!(assistant_text(&items), "fake answer");
    assert!(
        items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Failed,
                    title,
                    metadata,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Memory persistence failed"
                    && metadata["source"] == "provider_structured_output"
                    && metadata["proposal_count"] == 1
                    && metadata["failed_proposal_count"] == 1
                    && metadata["failed_proposals"][0]["error"]
                        .as_str()
                        .is_some_and(|error| error.contains("unknown promoted predicate_id adores"))
            )
        }),
        "unexpected memory activity items: {items:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_validation_rejection_is_discarded_without_failure_activity() {
    let (handle, _store) =
        test_runtime_handle_with_store(fake_codex_provider_with_invalid_memory_proposal()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let (result, events) = collect_turn_events(
        &handle,
        conversation.conversation_id,
        "Please produce an invalid memory proposal.".to_string(),
    )
    .await;
    result.expect("turn should complete despite rejected memory proposal");
    let items = transcript_items_from_events(events);

    assert_eq!(assistant_text(&items), "fake answer");
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed | TurnActivityStatus::Failed,
                    ..
                } if activity_kind == "memory_extraction"
            )
        }),
        "fully rejected provider memory proposals should not create a terminal memory activity: {items:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_discards_invalid_extraction_proposal_and_persists_valid_one() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_mixed_invalid_memory_proposals())
            .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I like planes, but please include one bad proposal fixture.".to_string(),
    )
    .await
    .expect("turn should persist valid memory despite one rejected proposal");

    assert_eq!(assistant_text(&items), "fake answer");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && metadata["proposal_count"] == 2
                && metadata["validated_proposal_count"] == 1
                && metadata["rejected_extraction_proposal_count"] == 1
                && metadata["failed_proposal_count"] == 0
        )
    }));
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Failed,
                    ..
                } if activity_kind == "memory_extraction"
            )
        }),
        "one rejected extractor proposal should not make the memory activity fail: {items:?}"
    );

    let claims = store
        .retrieve_claims(&answer_claim_request(), "planes", 8)
        .await
        .expect("retrieve valid memory claim");
    assert!(
        claims
            .included
            .iter()
            .any(|claim| claim.fact == "Kevin likes planes."),
        "valid proposal should persist: {claims:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_discards_local_human_preference_from_assistant_status_chatter() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_assistant_status_chatter_memory())
            .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let (result, events) =
        collect_turn_events(&handle, conversation.conversation_id, "Nice".to_string()).await;
    result.expect("turn should complete despite rejected status-chatter memory proposal");
    let items = transcript_items_from_events(events.clone());

    assert_eq!(
        assistant_text(&items),
        "Tiny but important onboarding victory. Fred has a plane-shaped sticky note now."
    );
    let proposed_index =
        memory_extraction_event_index(&events, TurnActivityStatus::Started, "Memory proposed")
            .expect("streamed memory proposal marker");
    let assistant_item_index = assistant_text_item_event_index(
        &events,
        "Tiny but important onboarding victory. Fred has a plane-shaped sticky note now.",
    )
    .expect("persisted assistant text item");
    assert!(
        proposed_index < assistant_item_index,
        "streamed proposal marker should not wait for provider response persistence: {events:?}"
    );
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed | TurnActivityStatus::Failed,
                    ..
                } if activity_kind == "memory_extraction"
            )
        }),
        "assistant status chatter should not create a terminal memory activity: {items:?}"
    );

    let claims = store
        .retrieve_claims(&answer_claim_request(), "planes", 8)
        .await
        .expect("retrieve plane claims");
    assert!(claims.included.is_empty(), "unexpected claims: {claims:?}");
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_graph_write_failure_persists_failed_activity() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
    store
        .db()
        .query("DELETE predicates WHERE predicate_id = 'prefers';")
        .await
        .expect("delete prefers predicate")
        .check()
        .expect("delete prefers predicate check");

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
    .expect("turn should complete despite graph write failure");

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
                && title == "Memory persistence failed"
                && summary == "graph claim write failed"
                && metadata["source"] == "provider_structured_output"
                && metadata["proposal_count"] == 1
        )
    }));
    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Memory persisted"
            )
        }),
        "failed graph write should not emit persisted activity: {items:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_actor_persists_provider_memory_proposals_as_graph_claims() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

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
    let assistant_item_index =
        assistant_text_item_event_index(&events, "fake answer").expect("persisted assistant text");
    assert!(
        proposed_index < assistant_item_index,
        "proposal marker should stream before provider response persistence: {events:?}"
    );
    assert!(
        memory_proposals_card_event_index(&events).is_none(),
        "provider proposal card should stay suppressed for graph-claim writes"
    );
    let persisted_index =
        memory_extraction_event_index(&events, TurnActivityStatus::Completed, "Memory persisted")
            .expect("persisted memory proposal activity");
    let proposed_item_id =
        memory_extraction_event_item_id(&events, TurnActivityStatus::Started, "Memory proposed")
            .expect("started memory proposal item id");
    assert!(
        proposed_item_id.starts_with("transient:"),
        "started memory proposal marker should be live-only, got {proposed_item_id}"
    );
    assert!(
        proposed_index < persisted_index,
        "persisted marker should stream after proposal marker: {events:?}"
    );
    let claim_id = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                summary: Some(summary),
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && summary == "saved 1 graph claim"
                && metadata["source"] == "provider_structured_output"
                && metadata["proposal_count"] == 1
                && metadata["created_claim_count"] == 1 =>
            {
                metadata["claim_ids"][0].as_str().map(str::to_string)
            }
            _ => None,
        })
        .expect("persisted memory proposal claim id");
    let activity_metadata = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction" && title == "Memory persisted" => {
                Some(metadata)
            }
            _ => None,
        })
        .expect("provider memory activity metadata");
    assert_eq!(
        activity_metadata["claim_outcomes"][0]["fact_preview"],
        json!("Kevin prefers same-call memory proposals.")
    );
    assert_eq!(
        activity_metadata["claim_outcomes"][0]["outcome"],
        json!("created")
    );
    assert_eq!(
        activity_metadata["claim_outcomes"][0]["sensitivity"],
        json!("normal")
    );
    assert_no_failed_memory_extraction(&items);
    let claims = store
        .retrieve_claims(&answer_claim_request(), "same-call memory proposals", 8)
        .await
        .expect("retrieve provider claim");
    assert!(
        claims.included.iter().any(|claim| {
            claim.claim_id == claim_id
                && claim.fact == "Kevin prefers same-call memory proposals."
                && claim.predicate_id == "prefers"
        }),
        "expected provider claim in retrieval, got {claims:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn runtime_actor_persists_natural_remember_provider_proposals_as_graph_claims() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

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
    let claim_id = items
        .iter()
        .find_map(|item| match item {
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && metadata["proposal_count"] == 1 =>
            {
                metadata["claim_ids"][0].as_str().map(str::to_string)
            }
            _ => None,
        })
        .expect("persisted natural remember provider claim id");
    assert_no_failed_memory_extraction(&items);
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
        )
    }));
    let claims = store
        .retrieve_claims(&answer_claim_request(), "trains", 8)
        .await
        .expect("retrieve natural remember provider claim");
    assert!(
        claims.included.iter().any(|claim| {
            claim.claim_id == claim_id
                && claim.fact == "Kevin likes trains."
                && claim.predicate_id == "likes"
        }),
        "expected natural remember provider claim in retrieval, got {claims:?}"
    );
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
async fn provider_first_person_memory_reinforces_explicit_canonical_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "remember: I prefer dark mode.".to_string(),
    )
    .await
    .expect("explicit seed turn");

    let items = collect_turn(&handle, conversation_id, "I prefer dark mode.".to_string())
        .await
        .expect("provider proposal turn");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && metadata["created_claim_count"] == 0
                && metadata["reinforced_claim_count"] == 1
        )
    }));

    let claims = store
        .retrieve_claims(&answer_claim_request(), "dark mode", 8)
        .await
        .expect("retrieve dark mode claim");
    assert_eq!(claims.included.len(), 1, "expected one claim: {claims:?}");
    assert_eq!(claims.included[0].fact, "Kevin prefers dark mode.");
    assert_eq!(claims.included[0].predicate_id, "prefers");
    handle.shutdown().await;
}

#[tokio::test]
async fn semantic_repeat_reinforces_existing_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
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
    .expect("seed turn");
    let items = collect_turn(
        &handle,
        conversation_id,
        "Ice cream is one of my favorite desserts.".to_string(),
    )
    .await
    .expect("repeat turn");

    assert!(
        items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    metadata,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Memory persisted"
                    && metadata["reinforced_claim_count"] == 1
            )
        }),
        "expected reinforced memory activity, got {items:?}"
    );

    let claims = store
        .retrieve_claims(&answer_claim_request(), "ice cream", 8)
        .await
        .expect("retrieve ice cream");
    assert_eq!(
        claims.included.len(),
        1,
        "expected one reinforced claim: {claims:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn semantic_repeat_reinforces_existing_claim_by_id_without_duplicate() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
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
    .expect("seed turn");
    collect_turn(
        &handle,
        conversation_id,
        "Ice cream is one of my favorite desserts.".to_string(),
    )
    .await
    .expect("semantic repeat turn");

    let claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("ice cream".to_string()),
            status: Some(crate::ClaimStatus::Active),
            predicate_id: Some("likes".to_string()),
            limit: Some(10),
        })
        .await
        .expect("ice cream claims");
    assert_eq!(
        claims.len(),
        1,
        "semantic reinforce should not create a fingerprint duplicate: {claims:?}"
    );
    assert_eq!(claims[0].evidence_count, 2);
    handle.shutdown().await;
}

#[tokio::test]
async fn consolidation_decision_rejects_existing_claim_id_outside_bounded_matches() {
    let (provider, invalid_target) = fake_codex_provider_with_invalid_consolidation_target();
    let (handle, store) = test_runtime_handle_with_store(provider).await;
    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();

    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like planes.".to_string(),
    )
    .await
    .expect("seed plane turn");
    let plane_claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("planes".to_string()),
            status: Some(crate::ClaimStatus::Active),
            predicate_id: Some("likes".to_string()),
            limit: Some(10),
        })
        .await
        .expect("plane claims");
    assert_eq!(
        plane_claims.len(),
        1,
        "expected seeded plane claim: {plane_claims:?}"
    );
    *invalid_target.lock().expect("target lock") = Some(plane_claims[0].claim_id.clone());

    collect_turn(
        &handle,
        conversation_id.clone(),
        "I like ice cream.".to_string(),
    )
    .await
    .expect("seed ice cream turn");
    let items = collect_turn(
        &handle,
        conversation_id,
        "Ice cream is one of my favorite desserts.".to_string(),
    )
    .await
    .expect("turn should complete despite rejected consolidation decision");

    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Failed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persistence failed"
                && metadata["failed_proposal_count"] == 1
                && metadata["failed_proposals"][0]["error"]
                    .as_str()
                    .is_some_and(|error| error.contains("not in consolidation match set"))
        )
    }));

    let ice_cream_claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("ice cream".to_string()),
            status: Some(crate::ClaimStatus::Active),
            predicate_id: Some("likes".to_string()),
            limit: Some(10),
        })
        .await
        .expect("ice cream claims");
    assert_eq!(
        ice_cream_claims.len(),
        1,
        "out-of-set relate decision should not create another ice cream claim: {ice_cream_claims:?}"
    );
    assert_eq!(ice_cream_claims[0].evidence_count, 1);
    handle.shutdown().await;
}

#[tokio::test]
async fn contradiction_becomes_reviewable_dispute() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
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
    .expect("seed turn");
    let items = collect_turn(&handle, conversation_id, "I hate ice cream.".to_string())
        .await
        .expect("conflict turn");

    assert!(
        items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    metadata,
                    ..
                } if activity_kind == "memory_extraction"
                    && title == "Memory needs review"
                    && metadata["disputed_claim_count"] == 1
            )
        }),
        "expected disputed memory activity, got {items:?}"
    );

    let claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("ice cream".to_string()),
            status: Some(crate::ClaimStatus::Disputed),
            predicate_id: None,
            limit: Some(10),
        })
        .await
        .expect("disputed claims");
    assert_eq!(claims.len(), 1);
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_mixed_active_and_disputed_claims_needs_review() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;
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
    .expect("seed turn");
    let items = collect_turn(
        &handle,
        conversation_id,
        "I like planes. I hate ice cream.".to_string(),
    )
    .await
    .expect("mixed provider batch turn");

    let memory_activity = items
        .iter()
        .rev()
        .find(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    ..
                } if activity_kind == "memory_extraction" && title != "Memory proposed"
            )
        })
        .expect("memory extraction activity");
    assert!(
        matches!(
            memory_activity,
            TurnTranscriptItem::Activity {
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if title == "Memory needs review"
                && metadata["created_claim_count"].as_u64().unwrap_or_default() > 0
                && metadata["active_saved_claim_count"].as_u64().unwrap_or_default() > 0
                && metadata["disputed_claim_count"].as_u64().unwrap_or_default() > 0
        ),
        "unexpected memory activity: {memory_activity:?}"
    );

    let active_claims = store
        .retrieve_claims(&answer_claim_request(), "planes", 8)
        .await
        .expect("retrieve planes claim");
    assert_eq!(
        active_claims.included.len(),
        1,
        "expected one active planes claim: {active_claims:?}"
    );

    let disputed_claims = store
        .list_claims(crate::MemoryClaimFilter {
            query: Some("ice cream".to_string()),
            status: Some(crate::ClaimStatus::Disputed),
            predicate_id: None,
            limit: Some(10),
        })
        .await
        .expect("disputed claims");
    assert_eq!(
        disputed_claims.len(),
        1,
        "expected one disputed ice cream claim: {disputed_claims:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_user_loves_planes_canonicalizes_to_likes_claim() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I love planes.".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && metadata["created_claim_count"] == 1
        )
    }));

    let claims = store
        .retrieve_claims(&answer_claim_request(), "planes", 8)
        .await
        .expect("retrieve planes claim");
    assert_eq!(claims.included.len(), 1, "expected one claim: {claims:?}");
    assert_eq!(claims.included[0].predicate_id, "likes");
    assert_eq!(claims.included[0].fact, "Kevin likes planes.");
    handle.shutdown().await;
}

#[tokio::test]
async fn unknown_memory_relationship_creates_predicate_proposal() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I collect model aircraft.".to_string(),
    )
    .await
    .expect("turn");

    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory needs review"
                && metadata["predicate_proposal_count"] == 1
        )
    }));

    let proposals = store
        .list_predicate_proposals(crate::store::PredicateProposalFilter {
            status: Some("candidate".to_string()),
            limit: Some(10),
        })
        .await
        .expect("predicate proposals");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].label, "collects");

    let claims = store
        .retrieve_claims(&answer_claim_request(), "model aircraft", 8)
        .await
        .expect("retrieve claims");
    assert!(
        claims.included.is_empty(),
        "unpromoted predicate should not retrieve: {claims:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_candidate_claim_with_predicate_proposal_needs_review() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I hate ice cream. I collect model aircraft.".to_string(),
    )
    .await
    .expect("turn");

    let memory_activity = items
        .iter()
        .rev()
        .find(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    ..
                } if activity_kind == "memory_extraction" && title != "Memory proposed"
            )
        })
        .expect("memory extraction activity");
    assert!(
        matches!(
            memory_activity,
            TurnTranscriptItem::Activity {
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if title == "Memory needs review"
                && metadata["predicate_proposal_count"] == 1
                && metadata["created_claim_count"] == 1
                && metadata["active_saved_claim_count"] == 0
        ),
        "unexpected memory activity: {memory_activity:?}"
    );

    let claims = store
        .list_claims(crate::MemoryClaimFilter {
            status: Some(crate::ClaimStatus::Candidate),
            predicate_id: Some("dislikes".to_string()),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .expect("candidate dislike claims");
    assert_eq!(claims.len(), 1, "expected one candidate claim: {claims:?}");
    assert_eq!(claims[0].fact, "Kevin dislikes ice cream.");

    let proposals = store
        .list_predicate_proposals(crate::store::PredicateProposalFilter {
            status: Some("candidate".to_string()),
            limit: Some(10),
        })
        .await
        .expect("predicate proposals");
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].label, "collects");
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_active_claim_with_predicate_proposal_persists_memory() {
    let (handle, _store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I like ice cream. I collect model aircraft.".to_string(),
    )
    .await
    .expect("turn");

    let memory_activity = items
        .iter()
        .rev()
        .find(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    title,
                    ..
                } if activity_kind == "memory_extraction" && title != "Memory proposed"
            )
        })
        .expect("memory extraction activity");
    assert!(
        matches!(
            memory_activity,
            TurnTranscriptItem::Activity {
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if title == "Memory needs review"
                && metadata["predicate_proposal_count"] == 1
                && metadata["created_claim_count"] == 1
                && metadata["active_saved_claim_count"] == 1
        ),
        "unexpected memory activity: {memory_activity:?}"
    );
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_first_person_local_name_memory_persists_without_explicit_seed() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_memory_extraction()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I prefer dark mode.".to_string(),
    )
    .await
    .expect("provider proposal turn");

    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                title,
                metadata,
                ..
            } if activity_kind == "memory_extraction"
                && title == "Memory persisted"
                && metadata["created_claim_count"] == 1
                && metadata["reinforced_claim_count"] == 0
        )
    }));
    assert_no_failed_memory_extraction(&items);

    let claims = store
        .retrieve_claims(&answer_claim_request(), "dark mode", 8)
        .await
        .expect("retrieve dark mode claim");
    assert_eq!(claims.included.len(), 1, "expected one claim: {claims:?}");
    assert_eq!(claims.included[0].fact, "Kevin prefers dark mode.");
    assert_eq!(claims.included[0].predicate_id, "prefers");
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_proposal_uses_initial_assistant_context_before_continuation() {
    let (handle, store) = test_runtime_handle_with_store(
        fake_codex_provider_with_initial_assistant_memory_continuation(),
    )
    .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "Search before saving the assistant note.".to_string(),
    )
    .await;
    result.expect("turn");

    let initial_assistant_item_id = assistant_item_id_for_text(
        &events,
        &conversation_id,
        "I will search memory before saving a note.",
    );
    let claim_id = memory_persisted_claim_id(&events);
    let source_item_id = claim_evidence_source_item_id(&store, &claim_id).await;
    assert_eq!(source_item_id, initial_assistant_item_id);
    assert!(events.iter().any(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem { item, .. }
                if matches!(
                    item.as_ref(),
                    TurnTranscriptItem::Activity {
                        activity_kind,
                        status: TurnActivityStatus::Completed,
                        title,
                        metadata,
                        ..
                    } if activity_kind == "memory_extraction"
                        && title == "Memory persisted"
                        && metadata["proposal_count"] == 1
                )
        )
    }));
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_proposal_uses_matching_assistant_item_within_phase() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_multi_assistant_memory()).await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    let (result, events) = collect_turn_events(
        &handle,
        conversation_id.clone(),
        "Emit two assistant notes and save the second.".to_string(),
    )
    .await;
    result.expect("turn");

    let second_assistant_item_id = assistant_item_id_for_text(
        &events,
        &conversation_id,
        "Second assistant item contains the durable note.",
    );
    let claim_id = memory_persisted_claim_id(&events);
    let source_item_id = claim_evidence_source_item_id(&store, &claim_id).await;
    assert_eq!(source_item_id, second_assistant_item_id);
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_proposal_discards_assistant_evidence_spanning_items() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_split_assistant_evidence_memory())
            .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let (result, events) = collect_turn_events(
        &handle,
        conversation.conversation_id,
        "Emit split assistant evidence and try to save it.".to_string(),
    )
    .await;
    result.expect("turn");
    let items = transcript_items_from_events(events);

    assert!(
        !items.iter().any(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed | TurnActivityStatus::Failed,
                    ..
                } if activity_kind == "memory_extraction"
            )
        }),
        "invalid assistant evidence should be discarded without terminal memory activity: {items:?}"
    );
    let claims = store
        .retrieve_claims(&answer_claim_request(), "split assistant note", 8)
        .await
        .expect("retrieve split assistant claims");
    assert!(claims.included.is_empty(), "unexpected claims: {claims:?}");
    handle.shutdown().await;
}

#[tokio::test]
async fn provider_memory_partial_write_reports_partial_failure() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_partial_memory_write()).await;
    delete_predicate(&store, "has_note").await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "I like partial write trains and need one failing note.".to_string(),
    )
    .await
    .expect("turn");

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
                && title == "Memory persistence partially failed"
                && summary == "saved 1 graph claim; 1 proposal failed"
                && metadata["proposal_count"] == 2
                && metadata["created_claim_count"] == 1
                && metadata["failed_proposal_count"] == 1
        )
    }));
    let claims = store
        .retrieve_claims(&answer_claim_request(), "partial write trains", 8)
        .await
        .expect("retrieve partial write claim");
    assert_eq!(
        claims.included.len(),
        1,
        "expected persisted claim: {claims:?}"
    );
    assert_eq!(claims.included[0].fact, "Kevin likes partial write trains.");
    handle.shutdown().await;
}

#[tokio::test]
async fn explicit_remember_is_saved_before_provider_failure() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_turn_error()).await;

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
                        status: TurnActivityStatus::Completed,
                        title,
                        ..
                    } if activity_kind == "memory_extraction"
                        && title == "Explicit memory saved"
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
    let claims = store
        .retrieve_claims(&answer_claim_request(), "failed turns", 8)
        .await
        .expect("retrieve pre-failure explicit claim");
    assert!(
        claims
            .included
            .iter()
            .any(|claim| claim.fact == "Kevin prefers failed turns to keep explicit memory."),
        "expected explicit memory saved before provider failure, got {claims:?}"
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
        "remember: I'm a big fan of trains".to_string(),
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
                .is_some_and(|memories| memories.iter().any(|memory| {
                    memory["kind"] == "claim"
                        && memory["fact"] == "Kevin likes trains."
                        && memory["predicate_id"] == "likes"
                }))
            && metadata["action"]["payload"].get("unavailable").is_none()
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I found your train memory."
    )));
    handle.shutdown().await;
}

#[tokio::test]
async fn update_own_name_tool_updates_agent_without_continuation_turn() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_update_own_name_continuation())
            .await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "Hey! How about Fred?".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Fred"));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
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
            && title == "Tool result: update_own_name"
            && metadata["action"]["success"] == true
            && metadata["action"]["payload"]["display_name"] == "Fred"
    )));
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "Fred it is."
    )));
}

#[tokio::test]
async fn update_own_name_tool_does_not_start_repeated_continuation_tool_calls() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_repeated_update_own_name()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "Hey! How about Fred?".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Fred"));

    let update_name_tool_calls = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
            )
        })
        .count();
    assert_eq!(update_name_tool_calls, 1, "{items:?}");
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "Fred it is."
    )));
}

#[tokio::test]
async fn ambiguous_name_suggestion_asks_confirmation_without_tool_call() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_ambiguous_update_own_name()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id,
        "Maybe you could be Mira?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("agent")
        .expect("agent exists");
    assert_eq!(agent.display_name, None);
    assert!(!items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text }
            if text == "Please confirm what you'd like to call me."
    )));
}

#[tokio::test]
async fn runtime_prompt_includes_stored_agent_name_after_update() {
    let handle =
        test_runtime_handle(fake_codex_provider_with_update_own_name_then_identity_check()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(
        &handle,
        conversation_id.clone(),
        "Your name is Mira.".to_string(),
    )
    .await
    .expect("name turn");
    let items = collect_turn(&handle, conversation_id, "What is your name?".to_string())
        .await
        .expect("identity turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw stored identity"
    )));
}

#[tokio::test]
async fn search_memory_profile_continuation_uses_scoped_empty_query() {
    let (handle, store) = test_runtime_handle_with_store(
        fake_codex_provider_with_search_memory_profile_continuation(),
    )
    .await;

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let conversation_id = conversation.conversation_id.clone();
    collect_turn(
        &handle,
        conversation_id.clone(),
        "remember: I like planes.".to_string(),
    )
    .await
    .expect("seed turn");

    collect_turn(
        &handle,
        conversation_id.clone(),
        "What memories do you have of me?".to_string(),
    )
    .await
    .expect("profile search turn");
    handle.shutdown().await;

    let replay = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    assert!(
        replay.iter().any(|item| {
            item.kind == ConversationItemKind::ToolResult
                && item.status == ConversationItemStatus::Completed
                && item.payload_json["metadata"]["action"]["success"] == true
                && item.payload_json["metadata"]["action"]["payload"]["scope_ids"]
                    == json!(["human:local"])
                && item.payload_json["metadata"]["action"]["payload"]["memories"]
                    .as_array()
                    .is_some_and(|memories| {
                        memories.iter().any(|memory| {
                            memory["kind"] == "claim"
                                && memory["fact"] == "Kevin likes planes."
                                && memory["predicate_id"] == "likes"
                        })
                    })
        }),
        "expected persisted successful scoped profile tool result, got {replay:?}"
    );
}

#[tokio::test]
async fn search_memory_tool_returns_empty_graph_result_without_unavailable() {
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
    assert_eq!(payload["omissions"], json!([]));
    assert!(payload.get("unavailable").is_none());
    assert!(payload["context_packet_id"].as_str().is_some());
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

#[tokio::test]
async fn uncalibrated_mcp_tool_call_returns_failed_tool_result() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_mcp_tool_call()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(&handle, conversation_id.clone(), "read my doc".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let items = store
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation items");
    assert!(
        items.iter().any(|item| {
            item.kind == ConversationItemKind::ToolResult
                && item.status == ConversationItemStatus::Failed
                && item.payload_json["metadata"]["action"]["name"] == "mcp.docs.read"
                && item.payload_json["metadata"]["action"]["payload"]["error"]
                    == "mcp_tool_not_calibrated"
        }),
        "expected failed uncalibrated MCP tool result, got {items:?}"
    );
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

fn assistant_text_item_event_index(
    events: &[TurnStreamEvent],
    expected_text: &str,
) -> Option<usize> {
    events.iter().position(|event| {
        matches!(
            event,
            TurnStreamEvent::ConversationItem { item, .. }
                if matches!(
                    item.as_ref(),
                    TurnTranscriptItem::AssistantText { text } if text == expected_text
                )
        )
    })
}

fn assistant_item_id_for_text(
    events: &[TurnStreamEvent],
    conversation_id: &str,
    expected_text: &str,
) -> String {
    events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem {
                conversation_id: id,
                item_id,
                item,
                ..
            } if id == conversation_id => match item.as_ref() {
                TurnTranscriptItem::AssistantText { text } if text == expected_text => {
                    Some(item_id.clone())
                }
                _ => None,
            },
            TurnStreamEvent::ConversationItem { .. }
            | TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .unwrap_or_else(|| panic!("expected assistant item `{expected_text}`, got {events:?}"))
}

fn memory_persisted_claim_id(events: &[TurnStreamEvent]) -> String {
    events
        .iter()
        .find_map(|event| match event {
            TurnStreamEvent::ConversationItem { item, .. } => match item.as_ref() {
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    metadata,
                    ..
                } if activity_kind == "memory_extraction" && title == "Memory persisted" => {
                    metadata["claim_ids"][0].as_str().map(str::to_string)
                }
                _ => None,
            },
            TurnStreamEvent::AssistantTextDelta { .. }
            | TurnStreamEvent::AgentStatusChanged { .. } => None,
        })
        .unwrap_or_else(|| panic!("expected persisted memory claim id, got {events:?}"))
}

async fn claim_evidence_source_item_id(store: &crate::NoemaStore, claim_id: &str) -> String {
    let mut response = store
        .db()
        .query(
            r#"
            SELECT source_item_id
            FROM supported_by
            WHERE claim_id = $claim_id
            LIMIT 1;
            "#,
        )
        .bind(("claim_id", claim_id.to_string()))
        .await
        .expect("claim evidence query");
    let rows: Vec<ClaimEvidenceSourceRow> = response.take(0).expect("claim evidence rows");
    rows.into_iter()
        .next()
        .and_then(|row| row.source_item_id)
        .expect("claim evidence source item id")
}

async fn claim_status_and_sensitivity(
    store: &crate::NoemaStore,
    claim_id: &str,
) -> ClaimStatusAndSensitivityRow {
    let mut response = store
        .db()
        .query(
            r#"
            SELECT status, sensitivity
            FROM claims
            WHERE claim_id = $claim_id
            LIMIT 1;
            "#,
        )
        .bind(("claim_id", claim_id.to_string()))
        .await
        .expect("claim sensitivity query");
    let rows: Vec<ClaimStatusAndSensitivityRow> = response.take(0).expect("claim sensitivity rows");
    rows.into_iter()
        .next()
        .expect("claim status and sensitivity row")
}

async fn maybe_entity_row(store: &crate::NoemaStore, entity_id: &str) -> Option<EntityRow> {
    let mut response = store
        .db()
        .query(
            r#"
            SELECT entity_id, entity_type, canonical_name
            FROM entities
            WHERE entity_id = $entity_id
            LIMIT 1;
            "#,
        )
        .bind(("entity_id", entity_id.to_string()))
        .await
        .expect("entity query");
    let rows: Vec<EntityRow> = response.take(0).expect("entity rows");
    rows.into_iter().next()
}

async fn delete_predicate(store: &crate::NoemaStore, predicate_id: &str) {
    store
        .db()
        .query("DELETE predicates WHERE predicate_id = $predicate_id;")
        .bind(("predicate_id", predicate_id.to_string()))
        .await
        .expect("delete predicate")
        .check()
        .expect("predicate deletion should succeed");
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ClaimEvidenceSourceRow {
    source_item_id: Option<String>,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct ClaimStatusAndSensitivityRow {
    status: String,
    sensitivity: String,
}

#[derive(Debug, Deserialize, SurrealValue)]
struct EntityRow {
    entity_id: String,
    entity_type: String,
    canonical_name: String,
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
    invalid_consolidation_target_id: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, Copy)]
enum FakeCodexScenario {
    Simple,
    RestartContext,
    IdentityPromptCheck,
    InitialNameOnboarding,
    TurnError,
    ToolItem,
    ToolItemThenFailure,
    UncalibratedMcpToolCall,
    InvalidSearchMemory,
    SearchMemoryContinuation,
    SearchMemoryProfileContinuation,
    UpdateOwnNameContinuation,
    RepeatedUpdateOwnNameContinuation,
    AmbiguousUpdateOwnName,
    UpdateOwnNameThenIdentityCheck,
    InitialAssistantMemoryContinuation,
    MultiAssistantMemory,
    SplitAssistantEvidenceMemory,
    MislabelledSecretMemory,
    MalformedCanonicalizer,
    MismatchedCanonicalEntity,
    UnknownCanonicalPredicate,
    PartialMemoryWrite,
    InvalidMemoryProposal,
    MixedInvalidMemoryProposal,
    AssistantStatusChatterMemory,
    MemoryExtraction,
}

impl FakeCodexProvider {
    fn new(scenario: FakeCodexScenario) -> Self {
        Self {
            scenario,
            invalid_consolidation_target_id: Arc::new(Mutex::new(None)),
        }
    }

    fn with_invalid_consolidation_target(
        invalid_consolidation_target_id: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            scenario: FakeCodexScenario::MemoryExtraction,
            invalid_consolidation_target_id,
        }
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
        if input.contains("Noema's memory claim canonicalizer") {
            let text = canonicalization_response_text(&input, self.scenario);
            return Ok(GenerateResponse {
                output: vec![GenerateOutputItem::AssistantText { text }],
                provider: "codex".to_string(),
                model,
                response_id: Some("fake-response".to_string()),
                usage: None,
            });
        }
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
            FakeCodexScenario::IdentityPromptCheck => {
                let saw_identity = instructions.contains("Agent identity:")
                    && instructions.contains(r#"agent_id: "agent:primary""#)
                    && instructions.contains("display_name: null")
                    && instructions.contains("Onboarding prompt:")
                    && instructions.contains("update_own_name");
                assistant_with_no_memories(if saw_identity {
                    "saw unnamed identity"
                } else {
                    "missing unnamed identity"
                })
            }
            FakeCodexScenario::InitialNameOnboarding => {
                let saw_onboarding = instructions.contains("Agent identity:")
                    && instructions.contains("display_name: null")
                    && instructions.contains("Onboarding prompt:")
                    && instructions.contains("Ask the user what they would like to name you.")
                    && instructions.contains("warm and welcoming")
                    && instructions.contains("energy")
                    && instructions.contains("Noema personal agent")
                    && instructions.contains("momentum going in life")
                    && instructions.contains("give you a name")
                    && instructions.contains("think, plan, make, untangle")
                    && instructions.contains("1-2 warm, energetic sentences")
                    && !input.contains("Your name is");
                assistant_with_no_memories(if saw_onboarding {
                    "Hey 👋 I'm your Noema personal agent, here to help you think, plan, make, untangle, or whatever keeps your momentum going in life. Before we dive in, give me a name!"
                } else {
                    "missing warm onboarding prompt"
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
            FakeCodexScenario::UncalibratedMcpToolCall => vec![
                mcp_tool_call(
                    "call_mcp_1",
                    "mcp.docs.read",
                    json!({"arguments": {"document_id": "doc_1"}}),
                ),
                GenerateOutputItem::MemoryProposals { proposals: vec![] },
            ],
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
                                "subjects": [{"id": null, "kind": "conversation", "name": "current conversation", "role": "about"}],
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
            FakeCodexScenario::SearchMemoryProfileContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "I remember that you like planes.".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                } else if input.contains("What memories do you have of me?") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "Searching memory.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_profile",
                            json!({"arguments": {
                                "scope_ids": ["human:local"],
                                "query": "",
                                "purpose": "answer_human_question",
                                "limit": 8
                            }}),
                        ),
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::UpdateOwnNameContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    let expected_name = if instructions.contains(r#"display_name: "Fred""#) {
                        "Fred"
                    } else {
                        "Mira"
                    };
                    let display_name_marker = format!(r#"display_name: "{expected_name}""#);
                    let saw_updated_identity = instructions.contains("Agent identity:")
                        && instructions.contains(&display_name_marker)
                        && !instructions.contains("You do not have a name yet.");
                    if saw_updated_identity {
                        let reply = format!("{expected_name} it is.");
                        assistant_with_no_memories(&reply)
                    } else {
                        assistant_with_no_memories("same-turn identity was stale")
                    }
                } else {
                    let name = if input.contains("Fred") {
                        "Fred"
                    } else {
                        "Mira"
                    };
                    vec![
                        update_own_name_tool_call("call_name_1", json!({"name": name})),
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                }
            }
            FakeCodexScenario::RepeatedUpdateOwnNameContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    vec![
                        update_own_name_tool_call("call_name_2", json!({"name": "Fred"})),
                        GenerateOutputItem::AssistantText {
                            text: "Fred it is.".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                } else {
                    vec![
                        update_own_name_tool_call("call_name_1", json!({"name": "Fred"})),
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                }
            }
            FakeCodexScenario::AmbiguousUpdateOwnName => {
                assistant_with_no_memories("Please confirm what you'd like to call me.")
            }
            FakeCodexScenario::UpdateOwnNameThenIdentityCheck => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("Mira it is.")
                } else if input.contains("Your name is Mira.") {
                    vec![
                        update_own_name_tool_call("call_name_1", json!({"name": "Mira"})),
                        GenerateOutputItem::MemoryProposals { proposals: vec![] },
                    ]
                } else {
                    let saw_identity = instructions.contains("Agent identity:")
                        && instructions.contains(r#"display_name: "Mira""#)
                        && !instructions.contains("You do not have a name yet.");
                    assistant_with_no_memories(if saw_identity {
                        "saw stored identity"
                    } else {
                        "missing stored identity"
                    })
                }
            }
            FakeCodexScenario::InitialAssistantMemoryContinuation => {
                if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
                    assistant_with_no_memories("Continuation answer without the initial evidence.")
                } else if input.contains("Search before saving the assistant note.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "I will search memory before saving a note.".to_string(),
                        },
                        search_memory_tool_call(
                            "call_1",
                            json!({"arguments": {"query": "trains"}}),
                        ),
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Noema should remember the initial assistant note.",
                                "memory_type": "note",
                                "title": "Initial assistant note",
                                "confidence": 0.74,
                                "sensitivity": "normal",
                                "subjects": [{"id": null, "kind": "conversation", "name": "current conversation", "role": "about"}],
                                "retrieval_hints": {"topics": ["memory"], "keywords": ["initial assistant note"], "summary": "Noema should remember the initial assistant note."},
                                "risk_flags": [],
                                "evidence_excerpt": "I will search memory before saving a note."
                            }))],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::MultiAssistantMemory => {
                if input.contains("Emit two assistant notes and save the second.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "First assistant item should not own the evidence.".to_string(),
                        },
                        GenerateOutputItem::AssistantText {
                            text: "Second assistant item contains the durable note.".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Noema should remember the second assistant note.",
                                "memory_type": "note",
                                "title": "Second assistant note",
                                "confidence": 0.78,
                                "sensitivity": "normal",
                                "subjects": [{"id": null, "kind": "conversation", "name": "current conversation", "role": "about"}],
                                "retrieval_hints": {"topics": ["memory"], "keywords": ["second assistant note"], "summary": "Noema should remember the second assistant note."},
                                "risk_flags": [],
                                "evidence_excerpt": "Second assistant item contains the durable note."
                            }))],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::SplitAssistantEvidenceMemory => {
                if input.contains("Emit split assistant evidence and try to save it.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "first assistant text".to_string(),
                        },
                        GenerateOutputItem::AssistantText {
                            text: "second assistant text".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Noema should remember the split assistant note.",
                                "memory_type": "note",
                                "title": "Split assistant note",
                                "confidence": 0.78,
                                "sensitivity": "normal",
                                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                "retrieval_hints": {"topics": ["memory"], "keywords": ["split assistant note"], "summary": "Noema should remember the split assistant note."},
                                "risk_flags": [],
                                "evidence_excerpt": "first assistant text\n\nsecond assistant text"
                            }))],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::MislabelledSecretMemory => {
                if input.contains("My API key is sk-testSecretToken123456789.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "fake answer".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Kevin's API key is sk-testSecretToken123456789.",
                                "memory_type": "note",
                                "title": "API key",
                                "confidence": 0.98,
                                "sensitivity": "normal",
                                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                "retrieval_hints": {"topics": ["credentials"], "keywords": ["api key"], "summary": "Kevin's API key is sk-testSecretToken123456789."},
                                "risk_flags": [],
                                "evidence_excerpt": "My API key is sk-testSecretToken123456789."
                            }))],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::MalformedCanonicalizer => {
                if input.contains("I prefer malformed canonicalizer tests.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "fake answer".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Kevin prefers malformed canonicalizer tests.",
                                "memory_type": "preference",
                                "title": "Malformed canonicalizer test preference",
                                "confidence": 0.91,
                                "sensitivity": "normal",
                                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                "retrieval_hints": {"topics": ["tests"], "keywords": ["malformed canonicalizer tests"], "summary": "Kevin prefers malformed canonicalizer tests."},
                                "risk_flags": [],
                                "evidence_excerpt": "I prefer malformed canonicalizer tests."
                            }))],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::MismatchedCanonicalEntity => {
                if input.contains("I prefer canonical entity validation.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "fake answer".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Kevin prefers canonical entity validation.",
                                "memory_type": "preference",
                                "title": "Canonical entity validation preference",
                                "confidence": 0.91,
                                "sensitivity": "normal",
                                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                "retrieval_hints": {"topics": ["memory"], "keywords": ["canonical entity validation"], "summary": "Kevin prefers canonical entity validation."},
                                "risk_flags": [],
                                "evidence_excerpt": "I prefer canonical entity validation."
                            }))],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::PartialMemoryWrite => {
                if input.contains("I like partial write trains and need one failing note.") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "fake answer".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![
                                proposal(json!({
                                    "content": "Kevin likes partial write trains.",
                                    "memory_type": "preference",
                                    "title": "Partial write train preference",
                                    "confidence": 0.91,
                                    "sensitivity": "normal",
                                    "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                                    "retrieval_hints": {"topics": ["trains"], "keywords": ["partial write trains"], "summary": "Kevin likes partial write trains."},
                                    "risk_flags": [],
                                    "evidence_excerpt": "I like partial write trains and need one failing note."
                                })),
                                proposal(json!({
                                    "content": "Provider note requiring the deleted note predicate.",
                                    "memory_type": "note",
                                    "title": "Deleted predicate note",
                                    "confidence": 0.91,
                                    "sensitivity": "normal",
                                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                    "retrieval_hints": {"topics": ["memory"], "keywords": ["deleted note predicate"], "summary": "Provider note requiring the deleted note predicate."},
                                    "risk_flags": [],
                                    "evidence_excerpt": "I like partial write trains and need one failing note."
                                })),
                            ],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::InvalidMemoryProposal => vec![
                GenerateOutputItem::AssistantText {
                    text: "fake answer".to_string(),
                },
                GenerateOutputItem::MemoryProposals {
                    proposals: vec![proposal(json!({
                        "content": "Kevin prefers invalid memory fixtures.",
                        "memory_type": "preference",
                        "title": "Invalid memory fixture",
                        "confidence": 0.91,
                        "sensitivity": "normal",
                        "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                        "retrieval_hints": {"topics": ["tests"], "keywords": ["invalid memory fixtures"], "summary": "Kevin prefers invalid memory fixtures."},
                        "risk_flags": [],
                        "evidence_excerpt": "this text is not in the turn"
                    }))],
                },
            ],
            FakeCodexScenario::MixedInvalidMemoryProposal => {
                if input.contains("one bad proposal fixture") {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "fake answer".to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![
                                proposal(json!({
                                    "content": "Kevin likes planes.",
                                    "memory_type": "preference",
                                    "title": "Plane preference",
                                    "confidence": 0.91,
                                    "sensitivity": "normal",
                                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                    "retrieval_hints": {"topics": ["aviation"], "keywords": ["planes"], "summary": "Kevin likes planes."},
                                    "risk_flags": [],
                                    "evidence_excerpt": "I like planes"
                                })),
                                proposal(json!({
                                    "content": "Kevin likes helicopters.",
                                    "memory_type": "preference",
                                    "title": "Helicopter preference",
                                    "confidence": 0.91,
                                    "sensitivity": "normal",
                                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                    "retrieval_hints": {"topics": ["aviation"], "keywords": ["helicopters"], "summary": "Kevin likes helicopters."},
                                    "risk_flags": [],
                                    "evidence_excerpt": "I like helicopters"
                                })),
                            ],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::AssistantStatusChatterMemory => {
                if input == "Nice" {
                    vec![
                        GenerateOutputItem::AssistantText {
                            text: "Tiny but important onboarding victory. Fred has a plane-shaped sticky note now."
                                .to_string(),
                        },
                        GenerateOutputItem::MemoryProposals {
                            proposals: vec![proposal(json!({
                                "content": "Kevin likes planes.",
                                "memory_type": "preference",
                                "title": "Plane preference",
                                "confidence": 0.91,
                                "sensitivity": "normal",
                                "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                                "retrieval_hints": {"topics": ["aviation"], "keywords": ["planes"], "summary": "Kevin likes planes."},
                                "risk_flags": [],
                                "evidence_excerpt": "Fred has a plane-shaped sticky note now."
                            }))],
                        },
                    ]
                } else {
                    assistant_with_no_memories("fake answer")
                }
            }
            FakeCodexScenario::UnknownCanonicalPredicate => memory_extraction_output(
                &input,
                self.invalid_consolidation_target_id
                    .lock()
                    .expect("invalid consolidation target lock")
                    .as_deref(),
            ),
            FakeCodexScenario::MemoryExtraction => memory_extraction_output(
                &input,
                self.invalid_consolidation_target_id
                    .lock()
                    .expect("invalid consolidation target lock")
                    .as_deref(),
            ),
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

fn update_own_name_tool_call(id: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        name: "update_own_name".to_string(),
        payload,
    }
}

fn mcp_tool_call(id: &str, name: &str, payload: serde_json::Value) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: Some(id.to_string()),
        name: name.to_string(),
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

fn canonicalization_response_text(input: &str, scenario: FakeCodexScenario) -> String {
    if matches!(scenario, FakeCodexScenario::MalformedCanonicalizer) {
        return "{not valid canonicalization json".to_string();
    }

    let response = if input.contains("Kevin's API key is sk-testSecretToken123456789.") {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:api_key", "entity_type": "concept", "canonical_name": "API key"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "has_note"},
                "fact": "Kevin's API key is sk-testSecretToken123456789.",
                "sensitivity": "normal",
                "status": "active",
                "confidence": 0.98,
                "retrieval_hints": {"keywords": ["api key"], "summary": "Kevin's API key is sk-testSecretToken123456789."},
                "rationale": "Unsafe fake canonicalizer promotion used to verify deterministic safety clamps."
            }]
        })
    } else if matches!(scenario, FakeCodexScenario::MismatchedCanonicalEntity)
        && input.contains("Kevin prefers canonical entity validation.")
    {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "person", "canonical_name": "Provider person"},
                "object": {"entity_id": "concept:canonical_entity_validation", "entity_type": "concept", "canonical_name": "canonical entity validation"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "prefers"},
                "fact": "Kevin prefers unsafe canonical entity metadata.",
                "sensitivity": "normal",
                "status": "active",
                "confidence": 0.9,
                "retrieval_hints": {"keywords": ["canonical entity validation"], "summary": "Kevin prefers unsafe canonical entity metadata."},
                "rationale": "Unsafe fake canonicalizer promotion used to verify entity identity validation."
            }]
        })
    } else if matches!(scenario, FakeCodexScenario::UnknownCanonicalPredicate)
        && input.contains("Kevin prefers automatic memory extraction in chat.")
    {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:automatic_memory_extraction", "entity_type": "concept", "canonical_name": "automatic memory extraction in chat"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "adores"},
                "fact": "Kevin adores automatic memory extraction in chat.",
                "sensitivity": "normal",
                "status": "active",
                "confidence": 0.9,
                "retrieval_hints": {"keywords": ["automatic memory extraction", "chat"], "summary": "Kevin adores automatic memory extraction in chat."},
                "rationale": "Unsafe fake canonicalizer promotion used to verify catalog validation."
            }]
        })
    } else if input.contains("Kevin collects model aircraft.") {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:model_aircraft", "entity_type": "concept", "canonical_name": "model aircraft"},
                "predicate": {
                    "kind": "predicate_proposal",
                    "proposal": {
                        "label": "collects",
                        "description": "The subject collects the object.",
                        "allowed_subject_types": ["human", "person"],
                        "allowed_object_types": ["concept", "other"],
                        "allowed_use_modes": ["answer", "personalize"],
                        "default_sensitivity": "normal",
                        "conflict_policy": "allow_many",
                        "review_policy": "auto_candidate",
                        "inverse_behavior": "none",
                        "inverse_predicate_id": null,
                        "proactivity_default": 1,
                        "merge_hints": {"strategy": "object_identity"},
                        "synonym_hints": ["keeps a collection of"],
                        "extraction_hints": {"examples": ["I collect model aircraft"]},
                        "rationale": "No promoted predicate represents collecting."
                    }
                },
                "fact": "Kevin collects model aircraft.",
                "sensitivity": "normal",
                "status": "candidate",
                "confidence": 0.9,
                "retrieval_hints": {"keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
                "rationale": "The source states a durable collecting relationship."
            }]
        })
    } else if input.contains("The user loves planes.") || input.contains("Kevin likes planes.") {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:claim_object_likes_planes", "entity_type": "concept", "canonical_name": "planes"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
                "fact": "Kevin likes planes.",
                "sensitivity": "normal",
                "status": "active",
                "confidence": 0.9,
                "retrieval_hints": {"keywords": ["planes"], "summary": "Kevin likes planes."},
                "rationale": "The source states a durable plane preference."
            }]
        })
    } else if input.contains("Kevin enjoys ice cream desserts.")
        || input.contains("Kevin likes ice cream.")
    {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:claim_object_likes_ice_cream", "entity_type": "concept", "canonical_name": "ice cream"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
                "fact": "Kevin likes ice cream.",
                "sensitivity": "normal",
                "status": "active",
                "confidence": 0.9,
                "retrieval_hints": {"keywords": ["ice cream"], "summary": "Kevin likes ice cream."},
                "rationale": "The source states a durable ice cream preference."
            }]
        })
    } else if input.contains("Kevin hates ice cream.") {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:claim_object_dislikes_ice_cream", "entity_type": "concept", "canonical_name": "ice cream"},
                "predicate": {"kind": "promoted_predicate", "predicate_id": "dislikes"},
                "fact": "Kevin dislikes ice cream.",
                "sensitivity": "normal",
                "status": "candidate",
                "confidence": 0.91,
                "retrieval_hints": {"keywords": ["ice cream"], "summary": "Kevin dislikes ice cream."},
                "rationale": "The source directly states a dislike that may conflict with an existing like."
            }]
        })
    } else {
        json!({
            "candidates": [{
                "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                "object": {"entity_id": "concept:canonicalizer_fallback", "entity_type": "concept", "canonical_name": "canonicalizer fallback"},
                "predicate": {"kind": "fallback_note"},
                "fact": "Canonicalizer fallback note.",
                "sensitivity": "normal",
                "status": "candidate",
                "confidence": 0.5,
                "retrieval_hints": {"keywords": ["canonicalizer fallback"], "summary": "Canonicalizer fallback note."},
                "rationale": "Fake provider fallback for tests."
            }]
        })
    };
    serde_json::to_string(&response).expect("canonicalizer json")
}

fn memory_extraction_output(
    input: &str,
    invalid_consolidation_target_id: Option<&str>,
) -> Vec<GenerateOutputItem> {
    if input.contains("Noema's memory consolidation comparator") {
        let existing_claim_id =
            first_memory_id_from_consolidation_prompt(input).expect("existing memory id");
        let decision = if let Some(invalid_target_id) = invalid_consolidation_target_id {
            json!({
                "decision": "relate",
                "existing_claim_id": invalid_target_id,
                "confidence": 0.92,
                "rationale": "maliciously references an unshown target",
            })
        } else if input.contains("Kevin hates ice cream.")
            || input.contains("Kevin dislikes ice cream.")
        {
            json!({
                "decision": "dispute",
                "existing_claim_id": existing_claim_id,
                "confidence": 0.93,
                "rationale": "opposite ice cream preference",
            })
        } else {
            json!({
                "decision": "reinforce",
                "existing_claim_id": existing_claim_id,
                "confidence": 0.92,
                "rationale": "same ice cream preference",
            })
        };
        return vec![GenerateOutputItem::AssistantText {
            text: serde_json::to_string(&decision).expect("semantic decision json"),
        }];
    }

    if input.contains("Noema's memory claim canonicalizer") {
        let response = if input.contains("Kevin collects model aircraft.") {
            json!({
                "candidates": [{
                    "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                    "object": {"entity_id": "concept:model_aircraft", "entity_type": "concept", "canonical_name": "model aircraft"},
                    "predicate": {
                        "kind": "predicate_proposal",
                        "proposal": {
                            "label": "collects",
                            "description": "The subject collects the object.",
                            "allowed_subject_types": ["human", "person"],
                            "allowed_object_types": ["concept", "other"],
                            "allowed_use_modes": ["answer", "personalize"],
                            "default_sensitivity": "normal",
                            "conflict_policy": "allow_many",
                            "review_policy": "auto_candidate",
                            "inverse_behavior": "none",
                            "inverse_predicate_id": null,
                            "proactivity_default": 1,
                            "merge_hints": {"strategy": "object_identity"},
                            "synonym_hints": ["keeps a collection of"],
                            "extraction_hints": {"examples": ["I collect model aircraft"]},
                            "rationale": "No promoted predicate represents collecting."
                        }
                    },
                    "fact": "Kevin collects model aircraft.",
                    "sensitivity": "normal",
                    "status": "candidate",
                    "confidence": 0.9,
                    "retrieval_hints": {"keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
                    "rationale": "The source states a durable collecting relationship."
                }]
            })
        } else if input.contains("The user loves planes.") || input.contains("Kevin likes planes.")
        {
            json!({
                "candidates": [{
                    "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                    "object": {"entity_id": "concept:claim_object_likes_planes", "entity_type": "concept", "canonical_name": "planes"},
                    "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
                    "fact": "Kevin likes planes.",
                    "sensitivity": "normal",
                    "status": "active",
                    "confidence": 0.9,
                    "retrieval_hints": {"keywords": ["planes"], "summary": "Kevin likes planes."},
                    "rationale": "The source states a durable plane preference."
                }]
            })
        } else if input.contains("Kevin enjoys ice cream desserts.")
            || input.contains("Kevin likes ice cream.")
        {
            json!({
                "candidates": [{
                    "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                    "object": {"entity_id": "concept:claim_object_likes_ice_cream", "entity_type": "concept", "canonical_name": "ice cream"},
                    "predicate": {"kind": "promoted_predicate", "predicate_id": "likes"},
                    "fact": "Kevin likes ice cream.",
                    "sensitivity": "normal",
                    "status": "active",
                    "confidence": 0.9,
                    "retrieval_hints": {"keywords": ["ice cream"], "summary": "Kevin likes ice cream."},
                    "rationale": "The source states a durable ice cream preference."
                }]
            })
        } else if input.contains("Kevin hates ice cream.") {
            json!({
                "candidates": [{
                    "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                    "object": {"entity_id": "concept:claim_object_dislikes_ice_cream", "entity_type": "concept", "canonical_name": "ice cream"},
                    "predicate": {"kind": "promoted_predicate", "predicate_id": "dislikes"},
                    "fact": "Kevin dislikes ice cream.",
                    "sensitivity": "normal",
                    "status": "candidate",
                    "confidence": 0.91,
                    "retrieval_hints": {"keywords": ["ice cream"], "summary": "Kevin dislikes ice cream."},
                    "rationale": "The source directly states a dislike that may conflict with an existing like."
                }]
            })
        } else {
            json!({
                "candidates": [{
                    "subject": {"entity_id": "human:local", "entity_type": "human", "canonical_name": "Local human"},
                    "object": {"entity_id": "concept:canonicalizer_fallback", "entity_type": "concept", "canonical_name": "canonicalizer fallback"},
                    "predicate": {"kind": "fallback_note"},
                    "fact": "Canonicalizer fallback note.",
                    "sensitivity": "normal",
                    "status": "candidate",
                    "confidence": 0.5,
                    "retrieval_hints": {"keywords": ["canonicalizer fallback"], "summary": "Canonicalizer fallback note."},
                    "rationale": "Fake provider fallback for tests."
                }]
            })
        };
        return vec![GenerateOutputItem::AssistantText {
            text: serde_json::to_string(&response).expect("canonicalizer json"),
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

    if input.contains("I like planes. I hate ice cream.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![
                    proposal(json!({
                        "content": "The user loves planes.",
                        "memory_type": "preference",
                        "title": "Plane preference",
                        "confidence": 0.91,
                        "sensitivity": "normal",
                        "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                        "retrieval_hints": {"topics": ["aviation"], "keywords": ["planes"], "summary": "The user loves planes."},
                        "risk_flags": [],
                        "evidence_excerpt": "I like planes."
                    })),
                    proposal(json!({
                        "content": "Kevin hates ice cream.",
                        "memory_type": "preference",
                        "title": "Ice cream dislike",
                        "confidence": 0.91,
                        "sensitivity": "normal",
                        "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                        "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin hates ice cream."},
                        "risk_flags": ["contradiction"],
                        "evidence_excerpt": "I hate ice cream."
                    })),
                ],
            },
        ];
    }

    if input.contains("I like planes.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "Kevin likes planes.",
                    "memory_type": "preference",
                    "title": "Plane preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["aviation"], "keywords": ["planes"], "summary": "Kevin likes planes."},
                    "risk_flags": [],
                    "evidence_excerpt": "I like planes."
                }))],
            },
        ];
    }

    if input.contains("I like ice cream. I collect model aircraft.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![
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
                    })),
                    proposal(json!({
                        "content": "Kevin collects model aircraft.",
                        "memory_type": "preference",
                        "title": "Model aircraft collection",
                        "confidence": 0.91,
                        "sensitivity": "normal",
                        "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                        "retrieval_hints": {"topics": ["hobbies"], "keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
                        "risk_flags": [],
                        "evidence_excerpt": "I collect model aircraft."
                    })),
                ],
            },
        ];
    }

    if input.contains("I like ice cream.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "Kevin likes ice cream.",
                    "memory_type": "preference",
                    "title": "Ice cream preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin likes ice cream."},
                    "risk_flags": [],
                    "evidence_excerpt": "I like ice cream."
                }))],
            },
        ];
    }

    if input.contains("I hate ice cream. I collect model aircraft.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![
                    proposal(json!({
                        "content": "Kevin hates ice cream.",
                        "memory_type": "preference",
                        "title": "Ice cream dislike",
                        "confidence": 0.91,
                        "sensitivity": "normal",
                        "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                        "retrieval_hints": {"topics": ["food"], "keywords": ["ice cream"], "summary": "Kevin hates ice cream."},
                        "risk_flags": ["contradiction"],
                        "evidence_excerpt": "I hate ice cream."
                    })),
                    proposal(json!({
                        "content": "Kevin collects model aircraft.",
                        "memory_type": "preference",
                        "title": "Model aircraft collection",
                        "confidence": 0.91,
                        "sensitivity": "normal",
                        "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                        "retrieval_hints": {"topics": ["hobbies"], "keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
                        "risk_flags": [],
                        "evidence_excerpt": "I collect model aircraft."
                    })),
                ],
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
                    "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
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

    if input.contains("I prefer dark mode.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "I prefer dark mode.",
                    "memory_type": "preference",
                    "title": "Dark mode preference",
                    "confidence": 0.92,
                    "sensitivity": "normal",
                    "subjects": [{"id": null, "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["display"], "keywords": ["dark mode"], "summary": "Kevin prefers dark mode."},
                    "risk_flags": [],
                    "evidence_excerpt": "I prefer dark mode."
                }))],
            },
        ];
    }

    if input.contains("I love planes.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "The user loves planes.",
                    "memory_type": "preference",
                    "title": "Plane preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["aviation"], "keywords": ["planes"], "summary": "The user loves planes."},
                    "risk_flags": [],
                    "evidence_excerpt": "I love planes."
                }))],
            },
        ];
    }

    if input.contains("I collect model aircraft.") && !input.contains("memory claim canonicalizer")
    {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "Kevin collects model aircraft.",
                    "memory_type": "preference",
                    "title": "Model aircraft collection",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["hobbies"], "keywords": ["model aircraft"], "summary": "Kevin collects model aircraft."},
                    "risk_flags": [],
                    "evidence_excerpt": "I collect model aircraft."
                }))],
            },
        ];
    }

    if input.contains("I prefer automatic memory extraction in chat.") {
        return vec![
            GenerateOutputItem::AssistantText {
                text: "fake answer".to_string(),
            },
            GenerateOutputItem::MemoryProposals {
                proposals: vec![proposal(json!({
                    "content": "Kevin prefers automatic memory extraction in chat.",
                    "memory_type": "preference",
                    "title": "Automatic memory extraction preference",
                    "confidence": 0.91,
                    "sensitivity": "normal",
                    "subjects": [{"id": "human:local", "kind": "human", "name": "Kevin", "role": "about"}],
                    "retrieval_hints": {"topics": ["memory"], "keywords": ["automatic memory extraction", "chat"], "summary": "Kevin prefers automatic memory extraction in chat."},
                    "risk_flags": [],
                    "evidence_excerpt": "I prefer automatic memory extraction in chat."
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

fn fake_codex_provider_with_identity_prompt_check() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::IdentityPromptCheck)
}

fn fake_codex_provider_with_initial_name_onboarding() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::InitialNameOnboarding)
}

fn fake_codex_provider_with_tool_item() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::ToolItem)
}

fn fake_codex_provider_with_tool_item_then_failure() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::ToolItemThenFailure)
}

fn fake_codex_provider_with_mcp_tool_call() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::UncalibratedMcpToolCall)
}

fn fake_codex_provider_with_invalid_search_memory_tool_item() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::InvalidSearchMemory)
}

fn fake_codex_provider_with_search_memory_continuation() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::SearchMemoryContinuation)
}

fn fake_codex_provider_with_search_memory_profile_continuation() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::SearchMemoryProfileContinuation)
}

fn fake_codex_provider_with_update_own_name_continuation() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::UpdateOwnNameContinuation)
}

fn fake_codex_provider_with_repeated_update_own_name() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::RepeatedUpdateOwnNameContinuation)
}

fn fake_codex_provider_with_ambiguous_update_own_name() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::AmbiguousUpdateOwnName)
}

fn fake_codex_provider_with_update_own_name_then_identity_check() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::UpdateOwnNameThenIdentityCheck)
}

fn fake_codex_provider_with_initial_assistant_memory_continuation() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::InitialAssistantMemoryContinuation)
}

fn fake_codex_provider_with_multi_assistant_memory() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::MultiAssistantMemory)
}

fn fake_codex_provider_with_split_assistant_evidence_memory() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::SplitAssistantEvidenceMemory)
}

fn fake_codex_provider_with_mislabelled_secret_memory() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::MislabelledSecretMemory)
}

fn fake_codex_provider_with_malformed_canonicalizer() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::MalformedCanonicalizer)
}

fn fake_codex_provider_with_mismatched_canonical_entity() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::MismatchedCanonicalEntity)
}

fn fake_codex_provider_with_unknown_canonical_predicate() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::UnknownCanonicalPredicate)
}

fn fake_codex_provider_with_partial_memory_write() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::PartialMemoryWrite)
}

fn fake_codex_provider_with_invalid_memory_proposal() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::InvalidMemoryProposal)
}

fn fake_codex_provider_with_mixed_invalid_memory_proposals() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::MixedInvalidMemoryProposal)
}

fn fake_codex_provider_with_assistant_status_chatter_memory() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::AssistantStatusChatterMemory)
}

fn fake_codex_provider_with_invalid_consolidation_target()
-> (FakeCodexProvider, Arc<Mutex<Option<String>>>) {
    let invalid_target_id = Arc::new(Mutex::new(None));
    (
        FakeCodexProvider::with_invalid_consolidation_target(invalid_target_id.clone()),
        invalid_target_id,
    )
}

fn fake_codex_provider_with_memory_extraction() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::MemoryExtraction)
}
