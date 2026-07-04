# Codex-Like Tool Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Noema's transcript ordering robust across GPT 5.4 and 5.5 by separating assistant commentary/final text from runtime tool execution lifecycle.

**Architecture:** Keep the provider contract as Noema's structured response envelope, but add explicit assistant text phases and stop treating provider `tool_call` output order as the visible execution order. The daemon should persist assistant text from a provider phase first, then emit a tool-call `STARTED` row immediately before executing each runtime tool and a tool-result row immediately after execution, matching Codex's distinction between model output items and execution lifecycle items.

**Tech Stack:** Rust daemon/runtime, provider-neutral response contract, SurrealDB-backed conversation items, existing React transcript renderer, Bun frontend validation.

---

## Scope

This plan is one backend harness refactor with a small prompt/contract addition. It intentionally avoids schema migrations, frontend visual redesign, browser inspection, and provider API changes beyond the existing JSON schema shape. Existing `conversation_items.metadata` and `payload_json` carry the new phase and lifecycle metadata.

## Current Behavior

Noema currently asks the provider for one JSON envelope whose `output` array can contain `assistant_text`, `tool_call`, and `memory_proposals`. The daemon persists those items in provider order, then executes local tools afterward. The frontend groups tool call and result rows by call id, so if GPT 5.4 emits `tool_call` before `assistant_text`, the marker appears above the message even though runtime execution happens after the provider response is parsed.

Codex's open-source repo keeps these as separate concepts: `ResponseItem::Message` can have `MessagePhase::Commentary`, tool calls are typed response items, and visible tool rows are emitted by tool execution handlers. Noema should adopt that boundary without cloning Codex's full protocol.

## File Structure

- Modify: `crates/noema-core/src/provider/contract.rs`
  - Add `AssistantTextPhase`.
  - Add optional `phase` to `GenerateOutputItem::AssistantText`.
  - Parse `assistant_text` items with or without `phase`.
  - Add helpers for phase defaults used by the runtime.
- Modify: `crates/noema-core/src/provider/adapters/noema_response_stream.rs`
  - Keep assistant text streaming unchanged.
  - Ignore `phase` while extracting visible text.
  - Add a parser regression test that `phase` does not break streamed text extraction.
- Modify: `crates/noema-core/src/daemon/prompts.rs`
  - Teach the model to emit `phase: "commentary"` for pre-tool status/preamble and `phase: "final_answer"` when the text is the user-facing terminal answer.
  - Preserve the single-envelope response contract.
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
  - Split tool discovery from tool execution.
  - Return a typed `LocalToolCall` from provider output before execution.
  - Execute one `LocalToolCall` at a time so the daemon can emit lifecycle rows around the actual side effect.
- Create: `crates/noema-core/src/daemon/runtime/tool_lifecycle.rs`
  - Small helper module for `LocalToolCall` to `GenerateOutputItem::ToolCall` conversion and deterministic tool activity ids.
  - Keeps `turn.rs` from growing further.
- Modify: `crates/noema-core/src/daemon/runtime/mod.rs`
  - Export the new `tool_lifecycle` module privately.
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
  - Add `persist_provider_tool_call_started`.
  - Store assistant phase in item metadata.
  - Keep result persistence through existing `ToolResult` path.
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
  - Persist non-tool provider items first.
  - For each local tool call: persist `STARTED`, execute, persist result.
  - Use the same flow for continuation turns.
  - Do not emit `update_own_name` continuation tool calls.
- Modify: `crates/noema-core/src/daemon/tests.rs`
  - Update fake provider helpers for `AssistantText { phase, text }`.
  - Add regression tests for 5.4-style `tool_call` before `assistant_text` and 5.5-style `assistant_text` before `tool_call`.
  - Update existing assertions that currently expect provider tool call rows before assistant text.
- No change planned: `crates/noema-core/web/src/components/transcript/renderModel.ts`
  - Existing grouping by `action.id`/`action.call_id` remains correct once backend ordering is fixed.

---

### Task 1: Add Assistant Text Phases To Provider Contract

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`

- [ ] **Step 1: Write failing contract tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `crates/noema-core/src/provider/contract.rs`.

```rust
#[test]
fn required_noema_response_accepts_assistant_text_phase() {
    let output = required_output_items_from_text(
        r#"{"type":"noema_response","output":[{"kind":"assistant_text","phase":"commentary","text":"Checking that now."},{"kind":"memory_proposals","proposals":[]}]}"#
            .to_string(),
    )
    .expect("required structured output");

    assert_eq!(
        output[0],
        GenerateOutputItem::AssistantText {
            phase: Some(AssistantTextPhase::Commentary),
            text: "Checking that now.".to_string(),
        }
    );
}

#[test]
fn required_noema_response_keeps_missing_assistant_text_phase_compatible() {
    let output = required_output_items_from_text(
        r#"{"type":"noema_response","output":[{"kind":"assistant_text","text":"Done."},{"kind":"memory_proposals","proposals":[]}]}"#
            .to_string(),
    )
    .expect("required structured output");

    assert_eq!(
        output[0],
        GenerateOutputItem::AssistantText {
            phase: None,
            text: "Done.".to_string(),
        }
    );
}

#[test]
fn assistant_text_phase_defaults_to_final_without_runtime_tools() {
    let phase = AssistantTextPhase::effective_for_output(
        &GenerateOutputItem::AssistantText {
            phase: None,
            text: "Done.".to_string(),
        },
        false,
    );
    assert_eq!(phase, AssistantTextPhase::FinalAnswer);
}

#[test]
fn assistant_text_phase_defaults_to_commentary_with_runtime_tools() {
    let phase = AssistantTextPhase::effective_for_output(
        &GenerateOutputItem::AssistantText {
            phase: None,
            text: "Checking that now.".to_string(),
        },
        true,
    );
    assert_eq!(phase, AssistantTextPhase::Commentary);
}
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```bash
cargo test -p noema-core provider::contract::tests::required_noema_response_accepts_assistant_text_phase provider::contract::tests::required_noema_response_keeps_missing_assistant_text_phase_compatible provider::contract::tests::assistant_text_phase_defaults_to_final_without_runtime_tools provider::contract::tests::assistant_text_phase_defaults_to_commentary_with_runtime_tools
```

Expected: compile failure because `AssistantTextPhase` and `phase` do not exist.

- [ ] **Step 3: Add the provider contract types**

In `crates/noema-core/src/provider/contract.rs`, add this enum before `GenerateOutputItem`.

```rust
/// User-visible phase for assistant text within one provider turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantTextPhase {
    /// Mid-turn assistant text such as preamble, status, or progress narration.
    Commentary,
    /// Terminal answer text for the current user-visible turn.
    FinalAnswer,
}

impl AssistantTextPhase {
    /// Return a stable storage string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commentary => "commentary",
            Self::FinalAnswer => "final_answer",
        }
    }

    /// Infer a display phase for old provider output that omitted phase.
    #[must_use]
    pub fn effective_for_output(output: &GenerateOutputItem, provider_phase_has_tools: bool) -> Self {
        match output {
            GenerateOutputItem::AssistantText {
                phase: Some(phase), ..
            } => *phase,
            GenerateOutputItem::AssistantText { phase: None, .. } if provider_phase_has_tools => {
                Self::Commentary
            }
            GenerateOutputItem::AssistantText { phase: None, .. } => Self::FinalAnswer,
            GenerateOutputItem::MemoryProposals { .. }
            | GenerateOutputItem::ToolCall { .. }
            | GenerateOutputItem::ToolResult { .. }
            | GenerateOutputItem::ApprovalRequest { .. }
            | GenerateOutputItem::ApprovalResult { .. }
            | GenerateOutputItem::Structured { .. } => Self::FinalAnswer,
        }
    }
}
```

Change `GenerateOutputItem::AssistantText` to:

```rust
    AssistantText {
        /// Whether the text is mid-turn commentary or the final answer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<AssistantTextPhase>,
        /// Text to show in the transcript.
        text: String,
    },
```

Update all pattern matches in `contract.rs` from `GenerateOutputItem::AssistantText { text }` to `GenerateOutputItem::AssistantText { text, .. }`.

- [ ] **Step 4: Update existing test constructors in `contract.rs`**

Where existing tests construct assistant text, change:

```rust
GenerateOutputItem::AssistantText {
    text: "Hello".to_string(),
}
```

to:

```rust
GenerateOutputItem::AssistantText {
    phase: None,
    text: "Hello".to_string(),
}
```

When a test expects a parsed phased item, use:

```rust
GenerateOutputItem::AssistantText {
    phase: Some(AssistantTextPhase::Commentary),
    text: "Searching Dex now.".to_string(),
}
```

- [ ] **Step 5: Run the focused tests**

Run:

```bash
cargo test -p noema-core provider::contract::tests::required_noema_response_accepts_assistant_text_phase provider::contract::tests::required_noema_response_keeps_missing_assistant_text_phase_compatible provider::contract::tests::assistant_text_phase_defaults_to_final_without_runtime_tools provider::contract::tests::assistant_text_phase_defaults_to_commentary_with_runtime_tools
```

Expected: all four tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/provider/contract.rs
git commit -m "feat: add assistant text phases"
```

---

### Task 2: Preserve Streaming With Phased Assistant Text

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/noema_response_stream.rs`

- [ ] **Step 1: Write the streaming regression test**

Add this test to the existing test module in `crates/noema-core/src/provider/adapters/noema_response_stream.rs`.

```rust
#[test]
fn noema_assistant_text_delta_extractor_ignores_assistant_text_phase() {
    let mut extractor = NoemaAssistantTextDeltaExtractor::default();
    let mut events = Vec::new();
    extractor.push_delta(
        r#"{"type":"noema_response","output":[{"kind":"assistant_text","phase":"commentary","text":"Checking"#,
        &mut |event| events.push(event),
    );
    extractor.push_delta(
        r#" now."},{"kind":"memory_proposals","proposals":[]}]} "#,
        &mut |event| events.push(event),
    );

    assert_eq!(
        assistant_text_from_events(&events),
        "Checking now.".to_string()
    );
}
```

- [ ] **Step 2: Run the focused streaming test**

Run:

```bash
cargo test -p noema-core provider::adapters::noema_response_stream::tests::noema_assistant_text_delta_extractor_ignores_assistant_text_phase
```

Expected: pass if the extractor already ignores non-text fields, or fail with a precise parser assertion if the phase field confuses text extraction.

- [ ] **Step 3: Keep the parser minimal**

If the test fails because the extractor assumes `text` immediately follows `kind`, update the assistant item parsing branch so it recognizes `kind: "assistant_text"` and continues scanning until it sees the `text` field. The final logic should continue to ignore nested payload fields. The key state transition should look like this:

```rust
if self.current_item_is_assistant_text() && key == "text" {
    self.start_visible_text_value();
}
```

Do not introduce phase-specific streaming events in this task. Durable phase metadata is enough for this slice.

- [ ] **Step 4: Run the streaming parser tests**

Run:

```bash
cargo test -p noema-core provider::adapters::noema_response_stream::tests
```

Expected: all noema response stream tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/noema-core/src/provider/adapters/noema_response_stream.rs
git commit -m "test: cover phased assistant streaming"
```

---

### Task 3: Prompt The Provider For Commentary And Final Answer Phases

**Files:**
- Modify: `crates/noema-core/src/daemon/prompts.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write prompt coverage assertions**

Add assertions to the existing prompt-oriented daemon tests that inspect `request.instructions`. In the fake-provider branch that checks the structured turn prompt, assert:

```rust
let saw_phase_contract = instructions.contains(r#""phase":"commentary""#)
    && instructions.contains(r#""phase":"final_answer""#)
    && instructions.contains("Use phase \"commentary\" for text that explains what you are about to do before a tool result is available.")
    && instructions.contains("Use phase \"final_answer\" only for the terminal answer after required tool results are available.");
```

Make the test return `"saw phase contract"` when `saw_phase_contract` is true and `"missing phase contract"` when false.

- [ ] **Step 2: Run the prompt test**

Run the daemon test that checks structured prompt content. If there is not already a narrow test name for this exact branch, run:

```bash
cargo test -p noema-core daemon::tests::runtime_provider_prompt_includes_memory_and_tool_contract
```

Expected: fail because the prompt does not mention phases yet. If the exact test name differs, run `cargo test -p noema-core daemon::tests:: -- --list | rg "prompt|identity|tool"` and use the nearest existing prompt contract test.

- [ ] **Step 3: Update the structured turn prompt**

In `build_structured_turn_system_prompt`, change the response shape example to:

```rust
Return exactly this top-level shape:
{{
  "type": "noema_response",
  "output": [
    {{"kind": "assistant_text", "phase": "final_answer", "text": "assistant reply to show the user"}},
    {{"kind": "memory_proposals", "proposals": []}}
  ]
}}
```

Add this rule block after the tool-call shape examples:

```rust
Assistant text phases:
- Use phase "commentary" for text that explains what you are about to do before a tool result is available.
- Use phase "final_answer" only for the terminal answer after required tool results are available.
- If you emit a tool_call in this response, any assistant_text in the same response should usually be commentary, because Noema has not executed the tool yet.
- After Noema sends a NOEMA_LOCAL_TOOL_RESULT message, use final_answer for the user-visible conclusion unless you need another tool first.
```

- [ ] **Step 4: Update onboarding prompt shape**

In `build_initial_name_onboarding_system_prompt`, change the example assistant text item to:

```rust
{{"kind": "assistant_text", "phase": "final_answer", "text": "a warm, concise onboarding message ending with a naming question"}}
```

- [ ] **Step 5: Run daemon prompt tests**

Run:

```bash
cargo test -p noema-core daemon::tests:: -- --nocapture
```

Expected: daemon tests compile. The broader daemon test group may take longer than a single unit test, but it should pass before committing this prompt contract change.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/daemon/prompts.rs crates/noema-core/src/daemon/tests.rs
git commit -m "prompt: request assistant text phases"
```

---

### Task 4: Split Local Tool Discovery From Execution

**Files:**
- Create: `crates/noema-core/src/daemon/runtime/tool_lifecycle.rs`
- Modify: `crates/noema-core/src/daemon/runtime/mod.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`

- [ ] **Step 1: Write focused unit tests for tool extraction**

Add a test module to the new `tool_lifecycle.rs` file with this test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::GenerateOutputItem;
    use serde_json::json;

    #[test]
    fn local_tool_calls_preserve_provider_output_indexes() {
        let output = vec![
            GenerateOutputItem::AssistantText {
                phase: Some(crate::provider::AssistantTextPhase::Commentary),
                text: "Checking memory.".to_string(),
            },
            GenerateOutputItem::ToolCall {
                id: Some("call_1".to_string()),
                name: "search_memory".to_string(),
                payload: json!({"scope_ids":["human:local"],"query":"","purpose":"answer_human_question"}),
            },
            GenerateOutputItem::MemoryProposals { proposals: vec![] },
        ];

        let calls = local_tool_calls(&output);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].output_index, 1);
        assert_eq!(calls[0].call_id.as_deref(), Some("call_1"));
        assert_eq!(calls[0].name, "search_memory");
    }
}
```

- [ ] **Step 2: Run the focused failing test**

Run:

```bash
cargo test -p noema-core daemon::runtime::tool_lifecycle::tests::local_tool_calls_preserve_provider_output_indexes
```

Expected: compile failure because `tool_lifecycle` is not wired and `local_tool_calls` does not exist.

- [ ] **Step 3: Implement `tool_lifecycle.rs`**

Create `crates/noema-core/src/daemon/runtime/tool_lifecycle.rs`:

```rust
use crate::provider::GenerateOutputItem;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LocalToolCall {
    pub(super) output_index: usize,
    pub(super) call_id: Option<String>,
    pub(super) name: String,
    pub(super) payload: Value,
}

pub(super) fn local_tool_calls(output: &[GenerateOutputItem]) -> Vec<LocalToolCall> {
    output
        .iter()
        .enumerate()
        .filter_map(|(output_index, item)| match item {
            GenerateOutputItem::ToolCall { id, name, payload } => Some(LocalToolCall {
                output_index,
                call_id: id.clone(),
                name: name.clone(),
                payload: payload.clone(),
            }),
            GenerateOutputItem::AssistantText { .. }
            | GenerateOutputItem::MemoryProposals { .. }
            | GenerateOutputItem::ToolResult { .. }
            | GenerateOutputItem::ApprovalRequest { .. }
            | GenerateOutputItem::ApprovalResult { .. }
            | GenerateOutputItem::Structured { .. } => None,
        })
        .collect()
}

pub(super) fn tool_call_output_item(call: &LocalToolCall) -> GenerateOutputItem {
    GenerateOutputItem::ToolCall {
        id: call.call_id.clone(),
        name: call.name.clone(),
        payload: call.payload.clone(),
    }
}
```

In `crates/noema-core/src/daemon/runtime/mod.rs`, add:

```rust
mod tool_lifecycle;
```

- [ ] **Step 4: Refactor local tool execution**

In `local_tools.rs`, replace the current `execute_local_tools` loop with a single-call executor:

```rust
use super::{
    actor::CodexRuntimeActor,
    tool_lifecycle::LocalToolCall,
    turn::SuccessfulProviderTurn,
};

impl CodexRuntimeActor {
    pub(super) async fn execute_local_tool(
        &self,
        turn: &SuccessfulProviderTurn,
        agent_identity: &AgentPromptIdentity,
        call: &LocalToolCall,
    ) -> LocalToolResult {
        let gateway = CapabilityGateway {
            store: &self.store,
            system_errors: &self.system_errors,
        };
        if is_search_memory_tool(&call.name) {
            let context = MemoryToolRuntimeContext {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                call_site_id: format!("output_{}", call.output_index),
                cwd: turn.cwd.clone(),
                user_input: turn.user_input.clone(),
            };
            LocalToolResult::Memory(
                execute_search_memory(&self.store, &context, call.call_id.clone(), &call.payload).await,
            )
        } else if is_update_own_name_tool(&call.name) {
            let context = AgentNameToolRuntimeContext {
                agent_id: agent_identity.agent_id.clone(),
            };
            LocalToolResult::AgentName(
                execute_update_own_name(&self.store, &context, call.call_id.clone(), &call.payload).await,
            )
        } else {
            let proposal = GatewayToolProposal {
                name: &call.name,
                payload: &call.payload,
            };
            LocalToolResult::Gateway {
                call_id: call.call_id.clone(),
                name: call.name.clone(),
                result: gateway.execute_tool_proposal(proposal).await,
            }
        }
    }
}
```

Remove the old `execute_local_tools` method after all call sites are updated in Task 5.

- [ ] **Step 5: Run the focused test**

Run:

```bash
cargo test -p noema-core daemon::runtime::tool_lifecycle::tests::local_tool_calls_preserve_provider_output_indexes
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/daemon/runtime/tool_lifecycle.rs crates/noema-core/src/daemon/runtime/mod.rs crates/noema-core/src/daemon/runtime/local_tools.rs
git commit -m "refactor: split local tool call discovery"
```

---

### Task 5: Emit Tool Lifecycle Rows At Runtime Execution Time

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add the 5.4-style ordering regression test**

Add this fake provider scenario:

```rust
FakeCodexScenario::ToolCallBeforeCommentary,
```

In the fake provider match, add:

```rust
FakeCodexScenario::ToolCallBeforeCommentary => {
    if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
        assistant_with_no_memories("The memory check is complete.")
    } else {
        vec![
            search_memory_tool_call(
                "call_1",
                json!({"arguments": {"query": "trains"}}),
            ),
            GenerateOutputItem::AssistantText {
                phase: Some(crate::provider::AssistantTextPhase::Commentary),
                text: "Checking memory.".to_string(),
            },
            GenerateOutputItem::MemoryProposals { proposals: vec![] },
        ]
    }
}
```

Then add this daemon test:

```rust
#[tokio::test]
async fn runtime_displays_commentary_before_tool_lifecycle_when_provider_orders_tool_first() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::ToolCallBeforeCommentary)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(&handle, conversation_id, "Check memory.".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let commentary_position = items
        .iter()
        .position(|item| matches!(item, TurnTranscriptItem::AssistantText { text } if text == "Checking memory."))
        .expect("commentary item");
    let tool_started_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Started,
                    title,
                    ..
                } if activity_kind == "tool_call" && title == "Tool call: search_memory"
            )
        })
        .expect("tool started item");
    let tool_result_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Completed,
                    title,
                    ..
                } if activity_kind == "tool_result" && title == "Tool result: search_memory"
            )
        })
        .expect("tool result item");

    assert!(
        commentary_position < tool_started_position,
        "commentary should describe intent before runtime tool execution starts: {items:?}"
    );
    assert!(
        tool_started_position < tool_result_position,
        "tool lifecycle should start before its result: {items:?}"
    );
}
```

- [ ] **Step 2: Run the failing ordering test**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_displays_commentary_before_tool_lifecycle_when_provider_orders_tool_first
```

Expected: fail because current persistence writes the provider `tool_call` row before assistant text.

- [ ] **Step 3: Store assistant phase in transcript metadata**

In `persist_provider_response_output_item`, compute whether the provider phase has tools in the caller and pass that boolean as an argument:

```rust
pub(super) async fn persist_provider_response_output_item(
    &mut self,
    turn: &ProviderActionTurn,
    index: usize,
    output: GenerateOutputItem,
    provider_phase_has_tools: bool,
    assistant_response: &mut ProviderAssistantResponse,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) -> Result<(), DaemonError>
```

In the assistant branch, add phase metadata:

```rust
GenerateOutputItem::AssistantText { phase: _, text } => {
    let effective_phase = crate::provider::AssistantTextPhase::effective_for_output(
        &GenerateOutputItem::AssistantText {
            phase,
            text: text.clone(),
        },
        provider_phase_has_tools,
    );
    let metadata = json!({
        "turn_index": turn.turn_index,
        "output_index": index,
        "stream_id": turn.stream_id,
        "phase": effective_phase.as_str(),
    });
    ...
}
```

Use a local binding so `phase` is still available:

```rust
GenerateOutputItem::AssistantText { phase, text } => {
    let effective_phase = AssistantTextPhase::effective_for_output(
        &GenerateOutputItem::AssistantText {
            phase,
            text: text.clone(),
        },
        provider_phase_has_tools,
    );
    ...
}
```

- [ ] **Step 4: Add started tool-call persistence**

In `transcript_persistence.rs`, add:

```rust
pub(super) async fn persist_provider_tool_call_started(
    &mut self,
    turn: &ProviderActionTurn,
    index: usize,
    call: &crate::daemon::runtime::tool_lifecycle::LocalToolCall,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) -> Result<(), DaemonError> {
    let display = tool_call_display(&call.name, &call.payload);
    self.persist_provider_action_output(
        turn,
        ProviderActionOutput {
            index,
            kind: ConversationItemKind::ToolCall,
            status: ConversationItemStatus::Running,
            action_kind: "tool_call",
            title: format!("Tool call: {}", call.name),
            summary: display_summary(&display, "target"),
            payload: json!({
                "id": call.call_id,
                "name": call.name,
                "payload": call.payload,
            }),
            display,
        },
        item_tx,
    )
    .await
}
```

Adjust the payload fields with `.clone()` where the compiler requires owned values:

```rust
"id": call.call_id.clone(),
"name": call.name.clone(),
"payload": call.payload.clone(),
```

- [ ] **Step 5: Refactor initial turn lifecycle in `turn.rs`**

In `persist_successful_provider_turn`, replace the initial persistence loop with:

```rust
let initial_tool_calls = local_tool_calls(&turn.response.output);
let initial_phase_has_tools = !initial_tool_calls.is_empty();
for (index, output) in turn.response.output.iter().cloned().enumerate() {
    if matches!(output, GenerateOutputItem::ToolCall { .. }) {
        continue;
    }
    self.persist_provider_response_output_item(
        &action_turn,
        index,
        output,
        initial_phase_has_tools,
        &mut initial_assistant_response,
        item_tx,
    )
    .await?;
}
```

Then replace the current `execute_local_tools` block with:

```rust
let mut local_tool_results = Vec::new();
for call in &initial_tool_calls {
    let local_action_turn = ProviderActionTurn {
        conversation_id: turn.conversation_id.clone(),
        turn_id: turn.turn_id.clone(),
        turn_index: turn.turn_index,
        user_item_id: turn.user_item_id.clone(),
        provider: "noema_local".to_string(),
        stream_id: None,
    };
    self.persist_provider_tool_call_started(
        &local_action_turn,
        call.output_index,
        call,
        item_tx,
    )
    .await?;
    let result = self.execute_local_tool(&turn, &turn.agent_identity, call).await;
    self.persist_provider_action_output_item(
        &local_action_turn,
        next_output_index,
        local_tool_result_output_item(&result),
        item_tx,
    )
    .await?;
    next_output_index += 1;
    local_tool_results.push(result);
}
let mut all_local_tool_results = local_tool_results.clone();
```

- [ ] **Step 6: Refactor continuation lifecycle in `turn.rs`**

Inside the continuation loop, after `continuation_outputs_for_tools` is built, add:

```rust
let continuation_tool_calls = local_tool_calls(&continuation_outputs_for_tools);
let continuation_phase_has_tools = !continuation_tool_calls.is_empty();
```

Persist non-tool outputs first:

```rust
for (offset, output) in continuation_outputs_for_tools.iter().cloned().enumerate() {
    if matches!(output, GenerateOutputItem::ToolCall { .. }) {
        continue;
    }
    self.persist_provider_response_output_item(
        &continuation_action_turn,
        continuation_output_base + offset,
        output,
        continuation_phase_has_tools,
        &mut continuation_assistant_response,
        item_tx,
    )
    .await?;
}
```

Execute continuation tool calls with the same started/result lifecycle:

```rust
let mut local_tool_results = Vec::new();
for call in &continuation_tool_calls {
    let local_action_turn = ProviderActionTurn {
        conversation_id: turn.conversation_id.clone(),
        turn_id: turn.turn_id.clone(),
        turn_index: turn.turn_index,
        user_item_id: turn.user_item_id.clone(),
        provider: "noema_local".to_string(),
        stream_id: None,
    };
    self.persist_provider_tool_call_started(
        &local_action_turn,
        continuation_output_base + call.output_index,
        call,
        item_tx,
    )
    .await?;
    let result = self
        .execute_local_tool(&continuation_turn, &continuation_turn.agent_identity, call)
        .await;
    self.persist_provider_action_output_item(
        &local_action_turn,
        next_output_index,
        local_tool_result_output_item(&result),
        item_tx,
    )
    .await?;
    next_output_index += 1;
    local_tool_results.push(result);
}
```

- [ ] **Step 7: Run the focused ordering test**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_displays_commentary_before_tool_lifecycle_when_provider_orders_tool_first
```

Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/daemon/runtime/transcript_persistence.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs
git commit -m "fix: emit tool lifecycle at runtime execution"
```

---

### Task 6: Preserve Existing Tool Continuation Semantics

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`

- [ ] **Step 1: Add 5.5-style regression coverage**

Add this test next to the 5.4-style ordering test:

```rust
#[tokio::test]
async fn runtime_keeps_commentary_before_tool_lifecycle_when_provider_orders_text_first() {
    let handle =
        test_runtime_handle(fake_provider(FakeCodexScenario::SearchMemoryContinuation)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id,
        "What do you remember about trains?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let commentary_position = items
        .iter()
        .position(|item| matches!(item, TurnTranscriptItem::AssistantText { text } if text == "Searching memory."))
        .expect("commentary item");
    let tool_started_position = items
        .iter()
        .position(|item| {
            matches!(
                item,
                TurnTranscriptItem::Activity {
                    activity_kind,
                    status: TurnActivityStatus::Started,
                    title,
                    ..
                } if activity_kind == "tool_call" && title == "Tool call: search_memory"
            )
        })
        .expect("tool started item");
    let final_position = items
        .iter()
        .rposition(|item| matches!(item, TurnTranscriptItem::AssistantText { text } if text == "I found your train memory."))
        .expect("final answer item");

    assert!(commentary_position < tool_started_position, "{items:?}");
    assert!(tool_started_position < final_position, "{items:?}");
}
```

- [ ] **Step 2: Update existing tests with old ordering assumptions**

In `runtime_actor_persists_provider_tool_items_as_action_rows`, replace:

```rust
assert!(
    tool_position < assistant_position,
    "tool call should replay before assistant text"
);
```

with:

```rust
assert!(
    assistant_position < tool_position,
    "assistant commentary should replay before runtime tool execution: {replay:?}"
);
```

Only make this replacement for tests where assistant text is pre-tool commentary. Keep final-answer-after-tool assertions for continuation answers.

- [ ] **Step 3: Run continuation and name tests**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_actor_executes_search_memory_as_local_tool_result daemon::tests::runtime_actor_continues_after_continuation_tool_call daemon::tests::update_own_name_tool_updates_agent_without_continuation_turn daemon::tests::update_own_name_tool_does_not_start_repeated_continuation_tool_calls daemon::tests::runtime_keeps_commentary_before_tool_lifecycle_when_provider_orders_text_first
```

Expected: all tests pass. The `update_own_name` tests must still show one tool result and no repeated continuation name call.

- [ ] **Step 4: Fix continuation indexes if needed**

If tests fail because result rows reuse an output index already used by started rows, set `next_output_index` after non-tool persistence like this:

```rust
let mut next_output_index = initial_output_count;
...
let continuation_output_base = next_output_index;
...
next_output_index = next_output_index
    .saturating_add(continuation_output_count)
    .saturating_add(local_tool_results.len());
```

The key invariant is:

```rust
assert!(tool_started_position < tool_result_position);
assert!(tool_result_position < final_position);
```

- [ ] **Step 5: Run daemon tests**

Run:

```bash
cargo test -p noema-core daemon::tests:: -- --no-fail-fast
```

Expected: daemon tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/tests.rs
git commit -m "test: preserve tool continuation ordering"
```

---

### Task 7: Keep Frontend Rendering Stable

**Files:**
- Modify only if needed: `crates/noema-core/web/src/components/transcript/renderModel.ts`
- Modify only if needed: `crates/noema-core/web/src/components/transcript/markerModel.ts`

- [ ] **Step 1: Verify the existing render model handles backend ordering**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/renderModel.test.ts
```

Expected: existing render model tests pass. The non-adjacent call/result grouping should remain valid because backend rows still carry `metadata.action.id` and `metadata.action.call_id`.

- [ ] **Step 2: Avoid frontend edits when backend ordering is sufficient**

If the test passes, do not edit frontend transcript code. The backend now controls semantic ordering and the frontend should continue rendering the transcript stream as provided.

- [ ] **Step 3: If a frontend edit is required, keep it limited**

Only if the existing test fails because a `STARTED` tool call plus later `COMPLETED` result no longer groups, change `toolActivityCorrelationId` in `renderModel.ts` to read both direct and nested action ids:

```ts
function toolActivityCorrelationId(entry: ActivityTranscriptEntry): string | undefined {
  const action = recordValue(entry.item.metadata)?.action;
  if (!isRecord(action)) {
    return undefined;
  }
  const id = stringValue(action.id) ?? stringValue(action.call_id);
  if (!id) {
    return undefined;
  }
  return [entry.turnId ?? "", id].join(":");
}
```

This is the current desired shape. If the file already matches it, leave it unchanged.

- [ ] **Step 4: Run frontend validation without browser tools**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: lint and build pass. Do not run browser inspection for this non-visual harness change.

- [ ] **Step 5: Commit only if frontend files changed**

If no frontend files changed, skip this commit. If `renderModel.ts` or `markerModel.ts` changed, run:

```bash
git add crates/noema-core/web/src/components/transcript/renderModel.ts crates/noema-core/web/src/components/transcript/markerModel.ts
git commit -m "fix: keep transcript tool marker grouping stable"
```

---

### Task 8: Full Validation And Durable Context Update

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update durable context**

Add this bullet under Settled Decisions in `docs/context/current.md`:

```markdown
- Assistant text and runtime tool execution are distinct transcript concepts.
  Provider `assistant_text` items may carry `commentary` or `final_answer`
  phase metadata, while visible tool markers are emitted from Noema runtime
  execution start/result timing rather than from provider output array order.
  This keeps GPT 5.4 and GPT 5.5 provider ordering differences from changing
  whether a tool appears to have run before the assistant's pre-tool message.
```

- [ ] **Step 2: Run formatting and checks**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
```

Expected: both pass.

- [ ] **Step 3: Run clippy and tests**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: both pass. If a failure is unrelated to this branch, capture the exact failing command and error summary before deciding whether to fix or report it.

- [ ] **Step 4: Run frontend validation if any web files changed**

Run only when files under `crates/noema-core/web` changed:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: both pass.

- [ ] **Step 5: Ship checklist**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected:
- `git diff --check` prints no whitespace errors.
- No unrelated dirty files are staged.
- Any remaining unstaged files are explicitly reported.

- [ ] **Step 6: Commit**

```bash
git add docs/context/current.md
git commit -m "docs: record tool lifecycle transcript model"
```

---

## Self-Review Notes

Spec coverage:
- Explains the GPT 5.4/5.5 ordering issue by removing provider-array order as the visible execution authority.
- Aligns with Codex's model of commentary/final answer phases and execution-owned tool lifecycle.
- Keeps Noema's current structured envelope and storage schema.
- Preserves bounded same-turn continuations and `update_own_name` one-shot behavior.

Placeholder scan:
- The plan contains concrete file paths, commands, expected outcomes, and code snippets for each implementation task.
- No schema migration or frontend redesign is hidden behind an unspecified step.

Type consistency:
- `AssistantTextPhase`, `GenerateOutputItem::AssistantText { phase, text }`, `LocalToolCall`, `local_tool_calls`, `tool_call_output_item`, `execute_local_tool`, and `persist_provider_tool_call_started` are introduced before later tasks use them.
- Rust snippets use existing project names: `GenerateOutputItem`, `TurnTranscriptItem`, `TurnActivityStatus`, `ProviderActionTurn`, `ConversationItemStatus`, and `ReplayMode`.
