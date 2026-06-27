# Agent Memory Read Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a governed `search_memory` agent tool that executes through Noema's deterministic memory retrieval path and appears as normal tool call/result transcript items.

**Architecture:** Add a focused daemon-side `memory_tool` module that validates tool arguments, builds the trusted `MemoryRetrievalRequest`, records a context packet, hydrates only approved memory ids, and formats the tool result JSON. Update the runtime to detect provider-reported `search_memory` calls, persist the call, execute the local tool, persist the result, and continue the same Noema turn with a second structured provider call containing the tool result. Keep automatic pre-turn retrieval out of scope.

**Tech Stack:** Rust, Tokio, serde/serde_json, sqlx/Postgres, existing Noema daemon runtime, existing `PostgresMemoryRepository::retrieve_memories`, existing `record_context_packet`.

---

## File Structure

- Create `crates/noema-core/src/daemon/memory_tool.rs`
  - Owns the local `search_memory` tool contract.
  - Parses and validates arguments.
  - Builds trusted retrieval requests from runtime state.
  - Calls `retrieve_memories` and `record_context_packet`.
  - Hydrates approved memory ids with `PostgresMemoryRepository::get_memory`.
  - Formats successful and failed tool-result JSON.
- Modify `crates/noema-core/src/daemon.rs`
  - Add `mod memory_tool;`.
- Modify `crates/noema-core/src/daemon/runtime.rs`
  - Import the new module.
  - Execute local `search_memory` calls while preserving existing provider action persistence.
  - Add a bounded continuation pass so the agent can answer after seeing the tool result.
  - Add search-memory tool instructions to the structured system prompt.
- Modify `crates/noema-core/src/daemon/tests.rs`
  - Add runtime tests for approved memory reads, policy omissions, context packet records, invalid arguments, and continuation.
- No schema changes are needed.
- No frontend-specific changes are needed because the result uses existing tool-event transcript rendering.

## Task 1: Add The Daemon Memory Tool Module

**Files:**
- Create: `crates/noema-core/src/daemon/memory_tool.rs`
- Modify: `crates/noema-core/src/daemon.rs`

- [ ] **Step 1: Create the module with parser, request builder, result formatter, and unit tests**

Create `crates/noema-core/src/daemon/memory_tool.rs` with this structure:

```rust
use crate::{
    daemon::memory_pipeline::project_scope_from_cwd,
    memory::{
        EligibilityReason, MemoryRetrievalRequest, Purpose, Sensitivity, TrustedRetrievalContext,
        UntrustedHints,
    },
    memory_persistence::{MemoryPersistenceError, MemorySummary, PostgresMemoryRepository},
};
use serde::Deserialize;
use serde_json::{Value, json};

const SEARCH_MEMORY_TOOL: &str = "search_memory";
const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MemoryToolRuntimeContext {
    pub conversation_id: String,
    pub turn_id: String,
    pub turn_index: u64,
    pub cwd: Option<String>,
    pub user_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MemoryToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum MemoryToolError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error(transparent)]
    Persistence(#[from] MemoryPersistenceError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchMemoryArguments {
    query: String,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

pub(super) fn is_search_memory_tool(name: &str) -> bool {
    name == SEARCH_MEMORY_TOOL
}

pub(super) async fn execute_search_memory(
    repository: &PostgresMemoryRepository,
    context: &MemoryToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> MemoryToolResult {
    match execute_search_memory_inner(repository, context, payload).await {
        Ok(payload) => MemoryToolResult {
            call_id,
            name: SEARCH_MEMORY_TOOL.to_string(),
            success: true,
            payload,
        },
        Err(error) => MemoryToolResult {
            call_id,
            name: SEARCH_MEMORY_TOOL.to_string(),
            success: false,
            payload: json!({
                "error": safe_error_message(&error),
            }),
        },
    }
}

async fn execute_search_memory_inner(
    repository: &PostgresMemoryRepository,
    context: &MemoryToolRuntimeContext,
    payload: &Value,
) -> Result<Value, MemoryToolError> {
    let arguments = parse_arguments(payload)?;
    let request = build_request(context, &arguments)?;
    let retrieval = repository.retrieve_memories(&request).await?;
    let context_packet_id = format!(
        "ctx_search_memory:{}:{}",
        context.conversation_id, context.turn_index
    );
    repository
        .record_context_packet(&context_packet_id, &context.turn_id, &request, &retrieval)
        .await?;

    let mut memories = Vec::new();
    for included in retrieval.included.iter().take(arguments.limit()) {
        if let Some(memory) = repository.get_memory(&included.memory_id).await? {
            memories.push(format_memory(&memory, eligibility_label(included.eligibility_reason)));
        }
    }

    let omissions = retrieval
        .agent_visible_omissions
        .iter()
        .map(|omission| json!({ "reason": omission.reason }))
        .collect::<Vec<_>>();

    Ok(json!({
        "memories": memories,
        "omissions": omissions,
    }))
}

fn parse_arguments(payload: &Value) -> Result<SearchMemoryArguments, MemoryToolError> {
    let argument_value = payload.get("arguments").unwrap_or(payload).clone();
    let arguments: SearchMemoryArguments = serde_json::from_value(argument_value)
        .map_err(|error| MemoryToolError::InvalidArguments(format!("invalid arguments: {error}")))?;
    if arguments.query.trim().is_empty() {
        return Err(MemoryToolError::InvalidArguments(
            "query is required".to_string(),
        ));
    }
    parse_purpose(arguments.purpose.as_deref())?;
    Ok(arguments)
}

impl SearchMemoryArguments {
    fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

fn build_request(
    context: &MemoryToolRuntimeContext,
    arguments: &SearchMemoryArguments,
) -> Result<MemoryRetrievalRequest, MemoryToolError> {
    let mut trusted =
        TrustedRetrievalContext::for_human("human:local", parse_purpose(arguments.purpose.as_deref())?);
    trusted.active_agent_ids = vec!["agent:primary".to_string()];
    trusted.active_scopes = vec![format!("conversation:{}", context.conversation_id)];
    if let Some(project_scope) = project_scope_from_cwd(context.cwd.as_deref()) {
        trusted.active_scopes.push(project_scope);
    }
    trusted.explicit_memory_request = explicit_memory_request(&context.user_input);
    trusted.sensitivity_ceiling = Sensitivity::Normal;
    trusted.include_candidate_memories = false;

    Ok(MemoryRetrievalRequest {
        requesting_principal_id: "agent:primary".to_string(),
        trusted,
        untrusted_hints: UntrustedHints {
            query_text: arguments.query.trim().to_string(),
            fuzzy_topics: Vec::new(),
            fuzzy_entities: Vec::new(),
        },
    })
}

fn parse_purpose(value: Option<&str>) -> Result<Purpose, MemoryToolError> {
    match value.unwrap_or("answer_human_question") {
        "answer_human_question" => Ok(Purpose::AnswerHumanQuestion),
        "draft_internal_content" => Ok(Purpose::DraftInternalContent),
        "general_personalization" => Ok(Purpose::GeneralPersonalization),
        "manage_task" => Ok(Purpose::ManageTask),
        "manage_calendar" => Ok(Purpose::ManageCalendar),
        "draft_external_content" => Ok(Purpose::DraftExternalContent),
        "use_tool" => Ok(Purpose::UseTool),
        "proactive_suggestion" => Ok(Purpose::ProactiveSuggestion),
        "external_action" => Ok(Purpose::ExternalAction),
        "debug_audit" => Ok(Purpose::DebugAudit),
        other => Err(MemoryToolError::InvalidArguments(format!(
            "unsupported purpose: {other}"
        ))),
    }
}

fn explicit_memory_request(input: &str) -> bool {
    let lowered = input.to_ascii_lowercase();
    lowered.contains("search memory")
        || lowered.contains("read memory")
        || lowered.contains("recall memory")
        || lowered.contains("remembered")
        || lowered.contains("what do you know about")
}

fn format_memory(memory: &MemorySummary, why: &'static str) -> Value {
    json!({
        "id": memory.id,
        "title": memory.title,
        "content": memory.content,
        "scope": memory.home_scope_id,
        "sensitivity": sensitivity_label(memory.sensitivity),
        "why": why,
    })
}

fn sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

fn eligibility_label(reason: EligibilityReason) -> &'static str {
    match reason {
        EligibilityReason::ActiveScope => "active_scope",
        EligibilityReason::ParticipantOverlap => "participant_overlap",
        EligibilityReason::ExplicitGrant => "explicit_grant",
        EligibilityReason::TrustedObjectLink => "trusted_object_link",
        EligibilityReason::PublicHint => "public_hint",
        EligibilityReason::GraphExpansion => "graph_expansion",
    }
}

fn safe_error_message(error: &MemoryToolError) -> String {
    match error {
        MemoryToolError::InvalidArguments(message) => message.clone(),
        MemoryToolError::Persistence(_) => "memory retrieval failed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> MemoryToolRuntimeContext {
        MemoryToolRuntimeContext {
            conversation_id: "conv_1".to_string(),
            turn_id: "turn_1".to_string(),
            turn_index: 7,
            cwd: Some("/Users/kpsuperplane/Documents/Projects/Noema".to_string()),
            user_input: "Can you search memory for train preferences?".to_string(),
        }
    }

    #[test]
    fn parses_nested_tool_arguments() {
        let payload = json!({
            "type": "toolCall",
            "arguments": {
                "query": "trains",
                "purpose": "answer_human_question",
                "limit": 99
            }
        });

        let arguments = parse_arguments(&payload).expect("arguments");

        assert_eq!(arguments.query, "trains");
        assert_eq!(arguments.limit(), MAX_LIMIT);
        assert_eq!(
            parse_purpose(arguments.purpose.as_deref()).expect("purpose"),
            Purpose::AnswerHumanQuestion
        );
    }

    #[test]
    fn rejects_empty_query() {
        let error = parse_arguments(&json!({"query": "  "})).unwrap_err();

        assert!(matches!(
            error,
            MemoryToolError::InvalidArguments(message) if message == "query is required"
        ));
    }

    #[test]
    fn rejects_unknown_purpose() {
        let error = parse_arguments(&json!({
            "query": "trains",
            "purpose": "dump_everything"
        }))
        .unwrap_err();

        assert!(matches!(
            error,
            MemoryToolError::InvalidArguments(message)
                if message == "unsupported purpose: dump_everything"
        ));
    }

    #[test]
    fn builds_conservative_trusted_request() {
        let arguments = parse_arguments(&json!({"query": "trains"})).expect("arguments");

        let request = build_request(&context(), &arguments).expect("request");

        assert_eq!(request.requesting_principal_id, "agent:primary");
        assert_eq!(request.trusted.active_human_ids, vec!["human:local"]);
        assert_eq!(request.trusted.active_agent_ids, vec!["agent:primary"]);
        assert!(request
            .trusted
            .active_scopes
            .contains(&"conversation:conv_1".to_string()));
        assert_eq!(request.trusted.sensitivity_ceiling, Sensitivity::Normal);
        assert!(!request.trusted.include_candidate_memories);
        assert!(request.trusted.explicit_memory_request);
        assert_eq!(request.untrusted_hints.query_text, "trains");
    }
}
```

- [ ] **Step 2: Wire the module into the daemon**

Modify `crates/noema-core/src/daemon.rs`:

```rust
mod client;
mod memory_tool;
mod memory_pipeline;
mod protocol;
mod runtime;
mod server;
```

- [ ] **Step 3: Run the focused unit tests**

Run:

```bash
cargo test -p noema-core daemon::memory_tool --no-fail-fast
```

Expected: the new `memory_tool` unit tests pass.

- [ ] **Step 4: Commit Task 1**

Run:

```bash
git add crates/noema-core/src/daemon.rs crates/noema-core/src/daemon/memory_tool.rs
git commit -m "Add search memory tool contract"
```

## Task 2: Execute `search_memory` And Persist Tool Results

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add failing runtime test for local tool result persistence**

In `crates/noema-core/src/daemon/tests.rs`, add a test near `runtime_actor_persists_provider_tool_items_as_action_rows`:

```rust
#[tokio::test]
async fn runtime_actor_executes_search_memory_as_local_tool_result() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_search_memory_continuation();
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

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Please remember I'm a big fan of trains".to_string(),
    )
    .await
    .expect("seed turn");
    assert_eq!(assistant_text(&items), "seeded train memory");

    let items = collect_turn(
        &handle,
        conversation_id.clone(),
        "Can you search memory for trains?".to_string(),
    )
    .await
    .expect("search turn");
    assert_eq!(assistant_text(&items), "I found your train memory.");

    let repo = postgres_repo(&database).await;
    let replay = repo
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("conversation replay");
    let tool_call = replay
        .iter()
        .find(|item| item.kind == ConversationItemKind::ToolCall)
        .expect("tool call");
    assert_eq!(tool_call.payload_json["activity_kind"], "tool_call");
    assert_eq!(
        tool_call.payload_json["metadata"]["action"]["name"],
        "search_memory"
    );

    let tool_result = replay
        .iter()
        .find(|item| item.kind == ConversationItemKind::ToolResult)
        .expect("tool result");
    assert_eq!(tool_result.status, ConversationItemStatus::Completed);
    assert_eq!(tool_result.payload_json["activity_kind"], "tool_result");
    assert_eq!(
        tool_result.payload_json["metadata"]["action"]["name"],
        "search_memory"
    );
    assert_eq!(
        tool_result.payload_json["metadata"]["action"]["payload"]["memories"][0]["content"],
        "Kevin is a big fan of trains."
    );
}
```

Add this fake app-server helper:

```rust
fn fake_codex_app_server_script_with_search_memory_continuation() -> std::path::PathBuf {
    let dir = tempfile::tempdir().expect("temp dir").keep();
    let path = dir.join("fake-codex-search-memory");
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
        if "Please remember I'm a big fan of trains" in input_text:
            text = json.dumps({
                "type": "noema_response",
                "output": [
                    {"kind": "assistant_text", "text": "seeded train memory"},
                    {"kind": "memory_proposals", "proposals": [{
                        "content": "Kevin is a big fan of trains.",
                        "memory_type": "preference",
                        "title": "Train enthusiasm",
                        "confidence": 0.92,
                        "sensitivity": "normal",
                        "subjects": [{
                            "id": "human:local",
                            "kind": "human",
                            "name": "Kevin",
                            "role": "about"
                        }],
                        "retrieval_hints": {
                            "topics": ["interests"],
                            "keywords": ["trains"],
                            "summary": "Kevin is a big fan of trains."
                        },
                        "risk_flags": [],
                        "evidence_excerpt": "I'm a big fan of trains"
                    }]}
                ]
            })
            print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
            print(json.dumps({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": text}}}), flush=True)
            print(json.dumps({"method": "turn/completed", "params": {"turn": {"status": "completed"}}}), flush=True)
        elif "NOEMA_LOCAL_TOOL_RESULT" in input_text:
            text = json.dumps({
                "type": "noema_response",
                "output": [
                    {"kind": "assistant_text", "text": "I found your train memory."},
                    {"kind": "memory_proposals", "proposals": []}
                ]
            })
            print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_2"}}}), flush=True)
            print(json.dumps({"method": "item/completed", "params": {"item": {"type": "agentMessage", "text": text}}}), flush=True)
            print(json.dumps({"method": "turn/completed", "params": {"turn": {"status": "completed"}}}), flush=True)
        else:
            print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
            print(json.dumps({"method": "item/completed", "params": {"item": {"type": "toolCall", "id": "call_1", "name": "search_memory", "arguments": {"query": "trains"}}}}), flush=True)
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
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p noema-core runtime_actor_executes_search_memory_as_local_tool_result --no-fail-fast
```

Expected: FAIL because the runtime persists provider tool calls but does not execute `search_memory` or persist a local tool result.

- [ ] **Step 3: Import the memory tool module in runtime**

At the top of `crates/noema-core/src/daemon/runtime.rs`, extend the `super` imports:

```rust
use super::{
    memory_pipeline::{
        ConversationMemoryContext, explicit_memory_content, extracted_proposal_to_candidate,
        infer_chat_memory_type, infer_chat_sensitivity, memory_activity, memory_activity_failed,
        project_scope_from_cwd, title_from_memory_content, typed_memory_activity,
    },
    memory_tool::{
        MemoryToolResult, MemoryToolRuntimeContext, execute_search_memory, is_search_memory_tool,
    },
    protocol::{
        AgentStatus, DaemonError, StartedConversation, TurnActivityStatus, TurnStreamEvent,
        TurnTranscriptItem,
    },
};
```

- [ ] **Step 4: Add local tool continuation helpers**

Add these helpers in `impl CodexRuntimeActor` near `persist_successful_provider_turn`:

```rust
    async fn execute_local_search_memory_tools(
        &mut self,
        action_turn: &ProviderActionTurn,
        context: &ConversationMemoryContext,
        output: &[GenerateOutputItem],
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<Vec<MemoryToolResult>, DaemonError> {
        let mut results = Vec::new();
        for (index, item) in output.iter().enumerate() {
            let GenerateOutputItem::ToolCall { id, name, payload } = item else {
                continue;
            };
            if !is_search_memory_tool(name) {
                continue;
            }
            self.update_conversation_agent_status(
                &context.conversation_id,
                PersistedAgentStatus::ToolRunning,
                item_tx,
            )
            .await?;
            let result = execute_search_memory(
                &self.memory_repository,
                &MemoryToolRuntimeContext {
                    conversation_id: context.conversation_id.clone(),
                    turn_id: context.turn_id.clone(),
                    turn_index: context.turn_index,
                    cwd: context.cwd.clone(),
                    user_input: context.user_content.clone(),
                },
                id.clone(),
                payload,
            )
            .await;
            self.persist_local_tool_result(action_turn, index, &result, item_tx)
                .await?;
            results.push(result);
        }
        Ok(results)
    }

    async fn persist_local_tool_result(
        &mut self,
        turn: &ProviderActionTurn,
        source_index: usize,
        result: &MemoryToolResult,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        self.persist_provider_action_output(
            turn,
            ProviderActionOutput {
                index: source_index + 10_000,
                kind: ConversationItemKind::ToolResult,
                status: if result.success {
                    ConversationItemStatus::Completed
                } else {
                    ConversationItemStatus::Failed
                },
                action_kind: "tool_result",
                title: format!("Tool result: {}", result.name),
                summary: result.call_id.as_deref().map(|id| format!("call id {id}")),
                payload: json!({
                    "call_id": result.call_id,
                    "name": result.name,
                    "success": result.success,
                    "payload": result.payload,
                    "source": "noema_local_tool",
                }),
            },
            item_tx,
        )
        .await
    }

    async fn continue_after_local_tool_results(
        &mut self,
        provider: &CodexAppServerConversation,
        structured_instructions: String,
        results: &[MemoryToolResult],
    ) -> Result<GenerateResponse, DaemonError> {
        let tool_result_input = json!({
            "type": "NOEMA_LOCAL_TOOL_RESULT",
            "results": results.iter().map(|result| {
                json!({
                    "call_id": result.call_id,
                    "name": result.name,
                    "success": result.success,
                    "payload": result.payload,
                })
            }).collect::<Vec<_>>(),
        })
        .to_string();
        self.runtime
            .turn_structured(provider, tool_result_input, structured_instructions)
            .await
            .map_err(DaemonError::from)
    }
```

- [ ] **Step 5: Call local tool execution from `persist_successful_provider_turn`**

First extend `SuccessfulProviderTurn` in `crates/noema-core/src/daemon/runtime.rs` with the provider conversation needed for the continuation call:

```rust
struct SuccessfulProviderTurn {
    conversation_id: String,
    turn_id: String,
    turn_index: u64,
    user_item_id: String,
    user_input: String,
    cwd: Option<String>,
    provider_conversation: CodexAppServerConversation,
    response: GenerateResponse,
    saved_memory_id: Option<String>,
}
```

When constructing `SuccessfulProviderTurn` in `turn()`, set:

```rust
provider_conversation: conversation.provider.clone(),
```

In `persist_successful_provider_turn`, after the loop that persists provider output items and after `memory_context` is built, insert:

```rust
        let local_tool_results = self
            .execute_local_search_memory_tools(&action_turn, &memory_context, &turn.response.output, item_tx)
            .await?;
        if !local_tool_results.is_empty() && !turn.response.assistant_text().trim().is_empty() {
            self.update_conversation_agent_status(
                &turn.conversation_id,
                PersistedAgentStatus::Thinking,
                item_tx,
            )
            .await?;
        }
```

Then adjust the function so the continuation call happens before memory extraction and before completing the turn:

```rust
        let final_assistant_text = if local_tool_results.is_empty() {
            memory_context.assistant_content.clone()
        } else {
            let continued = self
                .continue_after_local_tool_results(
                    &turn.provider_conversation,
                    build_structured_turn_system_prompt(
                        &turn.conversation_id,
                        turn.turn_index,
                        memory_context.cwd.as_deref(),
                        "",
                    ),
                    &local_tool_results,
                )
                .await?;
            self.persist_continuation_output_items(
                &action_turn,
                continued.output.clone(),
                item_tx,
            )
            .await?;
            continued.assistant_text()
        };
```

Use `final_assistant_text` when building the later `ConversationMemoryContext` for provider memory proposal validation or ordinary chat extraction, so extraction sees the final visible answer after tool continuation.

- [ ] **Step 6: Add a helper to persist continuation output**

Add:

```rust
    async fn persist_continuation_output_items(
        &mut self,
        action_turn: &ProviderActionTurn,
        output: Vec<GenerateOutputItem>,
        item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
    ) -> Result<(), DaemonError> {
        for (index, item) in output.into_iter().enumerate() {
            match item {
                GenerateOutputItem::AssistantText { text } => {
                    let item = self
                        .memory_repository
                        .append_conversation_item(NewConversationItem {
                            conversation_id: action_turn.conversation_id.clone(),
                            turn_id: Some(action_turn.turn_id.clone()),
                            parent_item_id: Some(action_turn.user_item_id.clone()),
                            kind: ConversationItemKind::AssistantText,
                            status: ConversationItemStatus::Completed,
                            author: ActorRef::agent("agent:primary"),
                            content_text: Some(text.clone()),
                            payload_json: json!({}),
                            metadata: json!({
                                "turn_index": action_turn.turn_index,
                                "output_index": index + 20_000,
                                "source": "local_tool_continuation",
                            }),
                        })
                        .await?;
                    send_conversation_item(
                        item_tx,
                        item,
                        TurnTranscriptItem::AssistantText { text },
                    );
                }
                GenerateOutputItem::MemoryProposals { .. } => {}
                other => {
                    self.persist_provider_action_output_item(action_turn, index + 20_000, other, item_tx)
                        .await?;
                }
            }
        }
        Ok(())
    }
```

- [ ] **Step 7: Run the focused test**

Run:

```bash
cargo test -p noema-core runtime_actor_executes_search_memory_as_local_tool_result --no-fail-fast
```

Expected: PASS.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Execute search memory tool calls"
```

## Task 3: Add Policy Omission And Invalid Argument Tests

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add test for sensitive omission redaction**

Add:

```rust
#[tokio::test]
async fn search_memory_tool_redacts_policy_omissions() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_tool_item();
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

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let repo = postgres_repo(&database).await;
    let source_item = repo
        .append_conversation_item(NewConversationItem {
            conversation_id: conversation_id.clone(),
            turn_id: None,
            parent_item_id: None,
            kind: ConversationItemKind::UserText,
            status: ConversationItemStatus::Completed,
            author: ActorRef::human("human:local"),
            content_text: Some("medical train memory source".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({}),
        })
        .await
        .expect("source");
    let mut memory = NewMemoryCandidate::confirmed_note(
        ObjectRef::new(ObjectType::Conversation, conversation_id.as_str())
            .expect("conversation object"),
        "Kevin has a sensitive train-related medical appointment.",
        ActorRef::human("human:local"),
        ObjectRef::conversation_item(source_item.item_id.as_str()),
    );
    memory.status = crate::memory::MemoryStatus::Active;
    memory.sensitivity = Sensitivity::Sensitive;
    memory.participants = vec![NewMemoryParticipant::new(
        ActorRef::human("human:local"),
        ParticipantRole::HumanInScope,
    )];
    let denied = repo
        .append_memory_candidate(memory)
        .await
        .expect("memory");

    let _ = collect_turn(&handle, conversation_id.clone(), "Use your tool".to_string())
        .await
        .expect("turn");

    let replay = repo
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");
    let tool_result = replay
        .iter()
        .find(|item| item.kind == ConversationItemKind::ToolResult)
        .expect("tool result");
    let payload = &tool_result.payload_json["metadata"]["action"]["payload"];
    assert_eq!(
        payload["omissions"],
        serde_json::json!([{ "reason": "policy_restricted_context" }])
    );
    assert!(!payload.to_string().contains(denied.id.as_str()));
    assert!(!payload.to_string().contains("medical appointment"));
}
```

- [ ] **Step 2: Add test for invalid arguments as failed tool result**

Add a fake helper that emits an unsupported purpose:

```rust
fn fake_codex_app_server_script_with_invalid_search_memory_tool_item() -> std::path::PathBuf {
    let dir = tempfile::tempdir().expect("temp dir").keep();
    let path = dir.join("fake-codex-invalid-search-memory");
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
        print(json.dumps({"id": msg["id"], "result": {"turn": {"id": "turn_1"}}}), flush=True)
        print(json.dumps({"method": "item/completed", "params": {"item": {"type": "toolCall", "id": "call_bad", "name": "search_memory", "arguments": {"query": "trains", "purpose": "dump_everything"}}}}), flush=True)
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
```

Add the test:

```rust
#[tokio::test]
async fn search_memory_tool_invalid_arguments_are_failed_tool_result() {
    let Some(database) = test_database().await else {
        return;
    };
    let script = fake_codex_app_server_script_with_invalid_search_memory_tool_item();
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

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(&handle, conversation_id.clone(), "Use your tool".to_string())
        .await
        .expect("turn");

    let repo = postgres_repo(&database).await;
    let replay = repo
        .list_conversation_items(&conversation_id, ReplayMode::Visible)
        .await
        .expect("replay");
    let tool_result = replay
        .iter()
        .find(|item| item.kind == ConversationItemKind::ToolResult)
        .expect("tool result");
    assert_eq!(tool_result.status, ConversationItemStatus::Failed);
    assert_eq!(
        tool_result.payload_json["metadata"]["action"]["payload"]["error"],
        "unsupported purpose: dump_everything"
    );
}
```

- [ ] **Step 3: Run the focused tests**

Run:

```bash
cargo test -p noema-core search_memory_tool_ --no-fail-fast
```

Expected: PASS for both tests.

- [ ] **Step 4: Commit Task 3**

Run:

```bash
git add crates/noema-core/src/daemon/tests.rs
git commit -m "Test search memory tool policy failures"
```

## Task 4: Add Tool Instructions And Full Runtime Validation

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Update the structured prompt**

In `build_structured_turn_system_prompt`, add this paragraph after the top-level shape:

```text
You may emit a search_memory tool call when memory would help answer the user's current message.
Use this output item shape:
{"kind":"tool_call","id":"call_memory_1","name":"search_memory","payload":{"query":"short search query","purpose":"answer_human_question","limit":8}}
Only Noema supplies trusted memory policy fields. Do not invent memory results.
After Noema sends a NOEMA_LOCAL_TOOL_RESULT message, answer using only the returned memories.
```

- [ ] **Step 2: Lock in required structured output support for tool calls**

Add this provider unit test in `crates/noema-core/src/provider.rs`:

```rust
#[test]
fn required_noema_response_accepts_tool_calls_with_memory_proposals() {
    let output = required_output_items_from_text(
        r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"I will check memory."},{"kind":"tool_call","id":"call_1","name":"search_memory","payload":{"query":"trains"}},{"kind":"memory_proposals","proposals":[]}]}"#
            .to_string(),
    )
    .expect("required structured output");

    assert!(matches!(
        &output[1],
        GenerateOutputItem::ToolCall { id: Some(id), name, .. }
            if id == "call_1" && name == "search_memory"
    ));
}
```

- [ ] **Step 3: Run provider and daemon focused tests**

Run:

```bash
cargo test -p noema-core required_noema_response_accepts_tool_calls_with_memory_proposals runtime_actor_executes_search_memory_as_local_tool_result --no-fail-fast
```

Expected: PASS.

- [ ] **Step 4: Commit Task 4**

Run:

```bash
git add crates/noema-core/src/provider.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Advertise search memory tool to agents"
```

## Task 5: Final Validation And Documentation Context

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Add durable context note**

Append this bullet under `## Settled Decisions` in `docs/context/current.md`:

```markdown
- Agent memory reads start as an explicit `search_memory` tool-only slice:
  Noema validates arguments, builds the trusted retrieval envelope, records a
  context packet, returns approved memories plus generic omissions as a normal
  tool result, and does not inject memories automatically before turns.
```

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS. If it fails with formatting diffs, run `cargo fmt --all`, inspect the diff, and rerun `cargo fmt --all --check`.

- [ ] **Step 3: Run focused Rust tests**

Run:

```bash
cargo test -p noema-core daemon::memory_tool runtime_actor_executes_search_memory_as_local_tool_result search_memory_tool_ required_noema_response_accepts_tool_calls_with_memory_proposals --no-fail-fast
```

Expected: PASS.

- [ ] **Step 4: Run workspace checks**

Run:

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: PASS. If daemon/Postgres tests skip because `NOEMA_TEST_DATABASE_URL` is unset, record that distinction in the final handoff.

- [ ] **Step 5: Inspect final diff**

Run:

```bash
git status --short --branch
git diff --check
git diff --stat
```

Expected:

- `git diff --check` prints no whitespace errors.
- The changed files are limited to:
  - `crates/noema-core/src/daemon.rs`
  - `crates/noema-core/src/daemon/memory_tool.rs`
  - `crates/noema-core/src/daemon/runtime.rs`
  - `crates/noema-core/src/daemon/tests.rs`
  - `crates/noema-core/src/provider.rs`
  - `docs/context/current.md`

- [ ] **Step 6: Commit Task 5**

Run:

```bash
git add crates/noema-core/src/daemon.rs crates/noema-core/src/daemon/memory_tool.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs crates/noema-core/src/provider.rs docs/context/current.md
git commit -m "Validate search memory tool"
```

## Self-Review

- Spec coverage:
  - Ordinary `search_memory` tool call: Tasks 1, 2, and 4.
  - Governed retrieval through `retrieve_memories`: Task 1.
  - Trusted envelope from runtime state: Task 1.
  - Normal tool call/result transcript items: Task 2.
  - Policy-approved memories plus generic omissions: Tasks 1 and 3.
  - Context packet and memory-use recording: Tasks 1 and 3.
  - Invalid arguments as failed tool result: Task 3.
  - No automatic pre-turn retrieval: Tasks 2 and 4 keep execution tool-triggered only.
- Placeholder scan: no placeholders, incomplete task descriptions, or unspecified validation commands.
- Type consistency: plan uses existing `GenerateOutputItem`, `PostgresMemoryRepository`, `MemoryRetrievalRequest`, `TrustedRetrievalContext`, `MemorySummary`, `ConversationItemKind`, `ConversationItemStatus`, and `TurnTranscriptItem` names.
