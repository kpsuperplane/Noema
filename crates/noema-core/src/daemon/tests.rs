use super::*;
use super::{
    memory_pipeline::{explicit_memory_content, infer_chat_sensitivity},
    runtime::CodexRuntimeHandle,
    server::bind_listener,
};
use crate::{
    memory::Sensitivity,
    memory_persistence::{MemoryType, SqliteMemoryRepository},
    providers::codex::CodexProviderConfig,
};
use serde_json::json;
use std::path::PathBuf;
use tokio::sync::mpsc;

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

    let response = DaemonResponse::TurnTranscriptItem {
        conversation_id: "conversation_1".to_string(),
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
    let script = fake_codex_app_server_script();
    let dir = tempfile::tempdir().expect("temp dir");
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        dir.path().join("db").join("noema.sqlite"),
    )
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
    let script = fake_codex_app_server_script_with_memory_extraction();
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("db").join("noema.sqlite");
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        db_path.clone(),
    )
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id.clone(),
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

    let repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");
    let memories = repo.list_recent_memories(Some(10)).expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Kevin prefers CLI memory inspection.");
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Confirmed);
    assert_eq!(memories[0].memory_type, MemoryType::Preference);
    assert_eq!(
        memories[0].conversation_id.as_deref(),
        Some("conversation:conversation_1")
    );

    let conn = rusqlite::Connection::open(&db_path).expect("raw conn");
    let message_provenance_count: i64 = conn
        .query_row(
            r"
            SELECT COUNT(*)
            FROM memory_provenance_edges pe
            JOIN messages m ON m.message_id = pe.source_id
            WHERE pe.memory_id = ?1 AND pe.source_type = 'message'
            ",
            rusqlite::params![memories[0].id],
            |row| row.get(0),
        )
        .expect("message provenance count");
    assert_eq!(message_provenance_count, 1);
}

#[tokio::test]
async fn runtime_actor_extracts_ordinary_chat_memory_as_activity() {
    let script = fake_codex_app_server_script_with_memory_extraction();
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("db").join("noema.sqlite");
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        db_path.clone(),
    )
    .expect("runtime");

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
                status: TurnActivityStatus::Started,
                title,
                ..
            } if activity_kind == "memory_extraction" && title == "Extracting memory proposals"
        )
    }));
    assert!(items.iter().any(|item| {
        matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Completed,
                summary: Some(summary),
                ..
            } if activity_kind == "memory_extraction" && summary == "created 1 memory candidate"
        )
    }));
    handle.shutdown().await;

    let repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");
    let memories = repo.list_recent_memories(Some(10)).expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(
        memories[0].content,
        "Kevin prefers automatic memory extraction in chat."
    );
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Active);
    assert_eq!(memories[0].memory_type, MemoryType::Preference);
    assert_eq!(
        memories[0].conversation_id.as_deref(),
        Some("conversation:conversation_1")
    );
}

#[tokio::test]
async fn runtime_actor_keeps_third_party_subject_conversation_scoped() {
    let script = fake_codex_app_server_script_with_memory_extraction();
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("db").join("noema.sqlite");
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        db_path.clone(),
    )
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let items = collect_turn(
        &handle,
        conversation.conversation_id,
        "Alice prefers decaf.".to_string(),
    )
    .await
    .expect("turn");
    assert_eq!(assistant_text(&items), "fake answer");
    handle.shutdown().await;

    let repo = SqliteMemoryRepository::open_at(&db_path).expect("repo");
    let memories = repo.list_recent_memories(Some(10)).expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Alice prefers decaf.");
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Candidate);
    assert_eq!(memories[0].home_scope_id, "conversation:conversation_1");

    let conn = rusqlite::Connection::open(&db_path).expect("raw conn");
    let linked_principal_id: Option<String> = conn
        .query_row(
            "SELECT linked_principal_id FROM entities WHERE entity_id = 'human:alice'",
            [],
            |row| row.get(0),
        )
        .expect("Alice entity");
    assert_eq!(linked_principal_id, None);
}

#[tokio::test]
async fn runtime_actor_persists_explicit_remember_before_provider_failure() {
    let script = fake_codex_app_server_script_with_turn_error();
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("db").join("noema.sqlite");
    let handle = CodexRuntimeHandle::spawn(
        CodexProviderConfig {
            command: script.to_string_lossy().to_string(),
            startup_timeout_seconds: 2,
            turn_timeout_seconds: 2,
            ..CodexProviderConfig::default()
        },
        db_path.clone(),
    )
    .expect("runtime");

    let conversation = handle
        .start_conversation(None, None)
        .await
        .expect("conversation");
    let error = collect_turn(
        &handle,
        conversation.conversation_id,
        "/remember Kevin wants failed turns to keep explicit memory.".to_string(),
    )
    .await
    .expect_err("provider error");
    assert!(matches!(error, DaemonError::Provider(_)));
    handle.shutdown().await;

    let repo = SqliteMemoryRepository::open_at(db_path).expect("repo");
    let memories = repo.list_recent_memories(Some(10)).expect("memories");
    assert_eq!(memories.len(), 1);
    assert_eq!(
        memories[0].content,
        "Kevin wants failed turns to keep explicit memory."
    );
    assert_eq!(memories[0].status, crate::memory::MemoryStatus::Confirmed);
}

async fn collect_turn(
    handle: &CodexRuntimeHandle,
    conversation_id: String,
    input: String,
) -> Result<Vec<TurnTranscriptItem>, DaemonError> {
    let (item_tx, mut item_rx) = mpsc::unbounded_channel();
    handle.turn(conversation_id, input, item_tx).await?;
    let mut items = Vec::new();
    while let Ok(item) = item_rx.try_recv() {
        items.push(item);
    }
    Ok(items)
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
    let Some(TurnTranscriptItem::AssistantText { text }) = items.first() else {
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
        print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
        print(json.dumps({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": "fake answer"}}}), flush=True)
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
