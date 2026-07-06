# Stable Prompt Cache And Encrypted Reasoning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make normal Noema chat turns reuse a stable prompt prefix while preserving encrypted reasoning continuity in stateless Responses API mode.

**Architecture:** Keep stable policy in provider `instructions`, move conversation-specific context into replayed provider input, and extend the existing `conversation_items` model-context replay path to carry opaque encrypted reasoning items. OpenAI and Codex adapters must be verified independently because their Responses API dialects can differ.

**Tech Stack:** Rust 2024, async Rust, SurrealDB-backed `conversation_items`, OpenAI-compatible Responses adapters, existing `cargo test` unit tests.

## Global Constraints

- Work on `main` and preserve unrelated dirty worktree changes.
- Use TDD: write a failing unit test, verify it fails, then implement.
- Do not add schema migrations or backward-compatibility layers unless explicitly requested.
- Keep `instructions` stable for a fixed provider, model profile, Noema prompt contract version, and model-visible tool contract.
- Do not put conversation id, turn index, cwd/project hint, active retrieval ids, compacted summary text, current user text, prior transcript text, timestamps, or request ids in stable `instructions`.
- Do not add a general volatile context packet before transcript items.
- Compaction is an intentional cache reset point.
- Do not expose raw reasoning to users; store and replay only opaque encrypted reasoning content plus minimal provider metadata.
- Verify OpenAI Responses and Codex Responses request/response behavior independently.
- Never modify `CARGO_BUILD_RUSTC_WRAPPER` or otherwise interfere with `sccache`.
- Default validation for this plan is focused unit tests during tasks, then `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace --no-fail-fast` before final commit.

---

## File Structure

- `crates/noema-core/src/provider/contract.rs`
  - Add provider-neutral encrypted reasoning history and response item types.
  - Add provider capability flags for encrypted reasoning and prompt cache key support.
  - Add request options for provider include fields only if the adapter needs provider-neutral control.

- `crates/noema-core/src/provider/tools.rs`
  - Extend `ProviderToolCapabilities` with explicit Responses-history feature gates.

- `crates/noema-core/src/provider/adapters/responses.rs`
  - Add shared OpenAI-compatible serialization/parsing for reasoning input/output items.
  - Add optional `include` and `prompt_cache_key` request fields.
  - Preserve opaque `reasoning.encrypted_content` without exposing it.

- `crates/noema-core/src/provider/adapters/openai.rs`
  - Advertise OpenAI support for prompt cache retention, prompt cache key, and encrypted reasoning only after adapter tests cover request shape.
  - Send `prompt_cache_key` for conversation requests.

- `crates/noema-core/src/provider/adapters/codex_responses.rs`
  - Verify Codex request fields independently.
  - Gate encrypted reasoning include/replay and cache retention by Codex-specific capability.

- `crates/noema-core/src/daemon/prompts.rs`
  - Split stable instructions from volatile conversation context.
  - Remove conversation id, turn index, cwd/project hint, active retrieval ids, and compacted summary from stable instructions.

- `crates/noema-core/src/daemon/runtime/prompt_context.rs`
  - Build append-only provider input from post-checkpoint `conversation_items`.
  - Add compacted summary as an input item only when an active summary exists.
  - Replay encrypted reasoning history items in provider order.

- `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
  - Persist encrypted reasoning output items as non-visible `reasoning` conversation items.

- `crates/noema-core/src/daemon/runtime/turn.rs`
  - Persist reasoning items from provider responses before subsequent continuation planning.
  - Preserve same-turn ordering relative to assistant text and native tool calls.

- `crates/noema-core/src/daemon/tests.rs`
  - Add runtime regression tests for stable instructions, append-only replay, compaction boundary behavior, and reasoning persistence/replay.

---

### Task 1: Stable Instructions And Append-Only Prompt Replay

**Files:**
- Modify: `crates/noema-core/src/daemon/prompts.rs`
- Modify: `crates/noema-core/src/daemon/runtime/prompt_context.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Consumes: existing `build_structured_turn_system_prompt(...) -> String`, `plan_prompt_context(...) -> PlannedPromptContext`, `GenerateInput`.
- Produces: stable `instructions` for normal turns and append-only `GenerateInput` replay where the previous turn input is a prefix of the next turn input.

- [ ] **Step 1: Write the failing stable instructions test**

Add this test to `crates/noema-core/src/daemon/tests.rs` near the existing prompt-context tests:

```rust
#[tokio::test]
async fn normal_turn_instructions_are_stable_across_turns() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id.clone(), "first durable question".to_string(), first_tx)
        .await
        .expect("first turn");
    while first_rx.recv().await.is_some() {}

    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "second durable question".to_string(), second_tx)
        .await
        .expect("second turn");
    while second_rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let agent_requests = requests
        .iter()
        .filter(|request| request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert_eq!(agent_requests.len(), 2);

    let first_instructions = agent_requests[0]
        .instructions
        .as_deref()
        .expect("first instructions");
    let second_instructions = agent_requests[1]
        .instructions
        .as_deref()
        .expect("second instructions");
    assert_eq!(first_instructions, second_instructions);
    assert!(!first_instructions.contains("conversation_id:"));
    assert!(!first_instructions.contains("turn_index:"));
    assert!(!first_instructions.contains("cwd_project_hint:"));
    assert!(!first_instructions.contains("Recent durable transcript"));
    assert!(!first_instructions.contains("first durable question"));
    assert!(!second_instructions.contains("second durable question"));
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p noema-core normal_turn_instructions_are_stable_across_turns
```

Expected: FAIL because current `instructions` include `turn_index` and conversation metadata.

- [ ] **Step 3: Write the failing append-only replay test**

Add this second test to `crates/noema-core/src/daemon/tests.rs` near the same section:

```rust
#[tokio::test]
async fn normal_turn_input_replays_previous_turn_as_prefix() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider {
        context_window_tokens: 20_000,
        fail_compaction: false,
        fail_token_count: false,
        enforce_context_window: false,
        requests: Mutex::new(Vec::new()),
    });
    let runtime = CodexRuntimeHandle::spawn_with_provider(provider.clone(), store)
        .await
        .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    let (first_tx, mut first_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id.clone(), "first durable question".to_string(), first_tx)
        .await
        .expect("first turn");
    while first_rx.recv().await.is_some() {}

    let (second_tx, mut second_rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "second durable question".to_string(), second_tx)
        .await
        .expect("second turn");
    while second_rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let agent_requests = requests
        .iter()
        .filter(|request| request.options.require_noema_response)
        .collect::<Vec<_>>();
    assert_eq!(agent_requests.len(), 2);

    let first_items = input_message_texts(&agent_requests[0].input);
    let second_items = input_message_texts(&agent_requests[1].input);
    assert_eq!(first_items, vec!["first durable question".to_string()]);
    assert!(second_items.starts_with(&[
        "first durable question".to_string(),
        "fake answer".to_string(),
    ]));
    assert_eq!(second_items.last().map(String::as_str), Some("second durable question"));
}
```

Add this helper at the bottom of `daemon/tests.rs` if it is not already present:

```rust
fn input_message_texts(input: &GenerateInput) -> Vec<String> {
    match input {
        GenerateInput::Text(text) => vec![text.clone()],
        GenerateInput::Messages(messages) => messages
            .iter()
            .map(|message| message.content.clone())
            .collect(),
        GenerateInput::Items(items) => items
            .iter()
            .filter_map(|item| match item {
                GenerateInputItem::Message(message) => Some(message.content.clone()),
                GenerateInputItem::ToolCall(_) | GenerateInputItem::ToolResult(_) => None,
            })
            .collect(),
        GenerateInput::NativeToolResults(_) => Vec::new(),
    }
}
```

- [ ] **Step 4: Run the append-only test**

Run:

```bash
cargo test -p noema-core normal_turn_input_replays_previous_turn_as_prefix
```

Expected: PASS if replay is already append-only, or FAIL if current prompt context inserts volatile content before replay.

- [ ] **Step 5: Move volatile metadata out of stable instructions**

Modify `crates/noema-core/src/daemon/prompts.rs`.

Change `build_structured_turn_system_prompt` to no longer accept volatile parameters:

```rust
pub(super) fn build_structured_turn_system_prompt(
    agent_identity: &AgentPromptIdentity,
    available_tools: &str,
    tool_exposure: PromptToolExposure<'_>,
) -> String {
    let agent_identity_prompt = agent_identity_prompt(agent_identity);
    let tool_instructions = tool_exposure_instructions(tool_exposure);

    format!(
        r#"{AGENT_PERSONALITY_PROMPT}

{agent_identity_prompt}

Reply to the user and emit any durable memory proposals in one structured response.

Return strict JSON only. Do not include Markdown, code fences, comments, or prose outside the JSON.
Never emit a top-level tool response, raw tool JSON, or plain text outside the envelope.
Multiple chat bubbles are multiple responses[] text items inside this one JSON object. Never split them into multiple top-level JSON objects or blank-line paragraphs inside one text item.

Return exactly this top-level shape:
{{
  "response_status": "final",
  "responses": [
    {{"kind":"text","phase":"final_answer","text":"assistant reply to show the user"}}
  ],
  "tool_calls": [],
  "memory_proposals": []
}}

Casual option-picking example:
{{
  "response_status": "final",
  "responses": [
    {{"kind":"text","phase":"final_answer","text":"my pick: option A"}},
    {{"kind":"text","phase":"final_answer","text":"short reason, no paragraph"}},
    {{"kind":"text","phase":"final_answer","text":"option B can wait"}}
  ],
  "tool_calls": [],
  "memory_proposals": []
}}

Available tools:
{available_tools}

{tool_instructions}

Assistant text phases:
- Use phase "commentary" for text that explains what you are about to do before a tool result is available.
- Use phase "final_answer" only for the terminal answer after required tool results are available.
- User-visible assistant text may use Markdown when it makes the answer clearer.
- Keep Markdown inside responses[].text; the outer response must remain strict JSON.
- If you emit a legacy builtin tool call in this JSON response, any text response in the same response should usually be commentary, because Noema has not executed the tool yet.
- After Noema sends a NOEMA_LOCAL_TOOL_RESULT message, use final_answer for the user-visible conclusion unless you need another tool first.
Example pre-tool text response: {{"kind":"text","phase":"commentary","text":"Checking that now."}}
Use response_status "needs_tools" whenever legacy JSON tool_calls is non-empty. responses may be empty only in a needs_tools response with at least one legacy JSON tool call.
Use response_status "final" only when tool_calls is empty and responses contains at least one text response.

Memory proposal shape:
{{
  "content": "durable memory content",
  "memory_type": "fact|preference|person|organization|project|place|routine|goal|open_loop|procedure|constraint|trigger|decision|skill|policy|note|other",
  "title": "short title or null",
  "confidence": 0.0,
  "sensitivity": "public|normal|private|sensitive|secret",
  "subjects": [
    {{
      "id": "optional canonical id or null",
      "kind": "human|agent|conversation|workspace|project|task|cron|relationship|tool|organization|place|concept|other",
      "name": "subject name",
      "role": "about|owner|affected|assignee|source|target|participant"
    }}
  ],
  "retrieval_hints": {{
    "topics": [],
    "keywords": [],
    "summary": null
  }},
  "risk_flags": [],
  "evidence_excerpt": "exact contiguous quote from the user or assistant source message"
}}

Rules:
- Always include responses, tool_calls, memory_proposals, and response_status.
- Include at least one text response for final answers.
- You may include zero text responses only when response_status is "needs_tools" and tool_calls is non-empty.
- Use an empty memory_proposals array when there are no durable memories.
- Propose only durable facts, preferences, constraints, decisions, routines, goals, procedures, or notes that could matter later.
- Do not propose jokes, speculation, transient task chatter, or generic world facts.
- Do not propose memories from assistant acknowledgements, status commentary, celebratory/meta commentary, or statements that something was saved, recorded, remembered, updated, or available in memory.
- Assistant evidence may support durable assistant, conversation, project, or workspace notes, but human-subject memories require direct user evidence.
- evidence_excerpt must be an exact contiguous quote from the original turn/source message and directly support the proposal.
- For assistant-supported proposals, evidence_excerpt must exactly quote the assistant text that generated the proposal in the same provider response phase.
- subjects must be non-empty and must show a human subject or participant when the memory affects a person.
- Use id "human:local" only for the current human/user/me. Do not use it for third-party people.
- confidence must be between 0.0 and 1.0. Use at least 0.70 only when evidence directly supports the proposal.
- Use an empty risk_flags array only for low-risk direct ordinary facts and preferences.
- Add risk_flags for inferred, sensitive, secret, action-triggering, contradiction-prone, third-party, risk-bearing, temporary, or external-egress proposals."#
    )
}
```

Update callers in `prompt_context.rs` and continuation prompt construction to use the new signature.

- [ ] **Step 6: Preserve active retrieval scope without volatile prompt text**

If legacy `search_memory` still needs active retrieval IDs, keep a stable rule in `tool_exposure_instructions`:

```rust
"Use scope_ids only from stable Noema-owned scopes you know are valid for this conversation. The default current human scope is human:local. If a project scope is required but not present in the user's request or tool result context, ask one blocking question instead of inventing a scope."
```

Do not add conversation ids or cwd-derived project ids to instructions.

- [ ] **Step 7: Keep compaction summary as input, not instructions**

Modify `render_prompt_context` in `prompt_context.rs` to return only summary text suitable for a replay input item:

```rust
fn render_prompt_context(summary: Option<&ConversationContextSummaryRecord>) -> Option<String> {
    summary.map(|summary| {
        format!(
            "Compacted conversation context:\n{}\n\nRecent transcript after this compacted checkpoint follows in subsequent messages.",
            summary.summary_text
        )
    })
}
```

Modify `PromptContext` so `rendered_context` is `Option<String>`, and modify `build_turn_input` to insert the summary as the first assistant message only when present:

```rust
fn build_turn_input(
    compacted_context: Option<&str>,
    transcript_items: &[ConversationItemRecord],
    current_input: &str,
) -> GenerateInput {
    let mut has_structured_items = false;
    let mut items = Vec::new();
    if let Some(context) = compacted_context.filter(|context| !context.trim().is_empty()) {
        items.push(GenerateInputItem::Message(GenerateMessage {
            role: GenerateMessageRole::Assistant,
            content: context.to_string(),
        }));
    }
    items.extend(transcript_items.iter().filter_map(input_item_from_transcript_item));
    if !current_input.trim().is_empty() {
        items.push(GenerateInputItem::Message(GenerateMessage {
            role: GenerateMessageRole::User,
            content: current_input.to_string(),
        }));
    }
    for item in &items {
        if !matches!(item, GenerateInputItem::Message(_)) {
            has_structured_items = true;
            break;
        }
    }
    if has_structured_items {
        GenerateInput::Items(items)
    } else {
        GenerateInput::Messages(
            items
                .into_iter()
                .filter_map(|item| match item {
                    GenerateInputItem::Message(message) => Some(message),
                    GenerateInputItem::ToolCall(_) | GenerateInputItem::ToolResult(_) => None,
                })
                .collect(),
        )
    }
}
```

Task 4 extends this match for `GenerateInputItem::Reasoning`.

- [ ] **Step 8: Run focused tests**

Run:

```bash
cargo test -p noema-core normal_turn_instructions_are_stable_across_turns
cargo test -p noema-core normal_turn_input_replays_previous_turn_as_prefix
cargo test -p noema-core prompt_context_uses_active_summary_and_post_checkpoint_items
cargo test -p noema-core native_provider_turn_request_includes_builtin_tools
```

Expected: all PASS.

- [ ] **Step 9: Commit Task 1**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/prompts.rs crates/noema-core/src/daemon/runtime/prompt_context.rs crates/noema-core/src/daemon/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "Stabilize chat instructions"
```

Expected: commit contains only Task 1 files. Preserve unrelated dirty files.

---

### Task 2: OpenAI Prompt Cache Key Parity

**Files:**
- Modify: `crates/noema-core/src/provider/tools.rs`
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`

**Interfaces:**
- Consumes: `GenerateRequest.conversation_id`.
- Produces: OpenAI Responses requests with `prompt_cache_key` for conversation-scoped calls.

- [ ] **Step 1: Write the failing OpenAI request test**

In `crates/noema-core/src/provider/adapters/openai.rs`, add a test next to `sends_expected_request_and_extracts_text`:

```rust
#[tokio::test]
async fn sends_prompt_cache_key_for_conversation_requests() {
    let (base_url, request_rx) = spawn_server(
        200,
        r#"{
          "id": "resp_test",
          "model": "gpt-test",
          "output": [{
            "type": "message",
            "content": [{"type": "output_text", "text": "Hello"}]
          }]
        }"#,
    )
    .await;
    let provider = test_provider(base_url);

    let response = provider
        .generate(GenerateRequest {
            conversation_id: Some("conversation:cacheable".to_string()),
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("Hello?".to_string()),
            ..GenerateRequest::text("ignored")
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert_eq!(body["prompt_cache_key"], "conversation:cacheable");
    assert_eq!(response.assistant_text(), "Hello");
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p noema-core sends_prompt_cache_key_for_conversation_requests
```

Expected: FAIL because `ResponsesRequest` does not serialize `prompt_cache_key`.

- [ ] **Step 3: Add shared cache-key helper**

In `responses.rs`, add:

```rust
pub(crate) fn prompt_cache_key_from_conversation_id(conversation_id: Option<&str>) -> Option<String> {
    let conversation_id = conversation_id?.trim();
    (!conversation_id.is_empty()).then(|| conversation_id.to_string())
}
```

Add a field to `ResponsesRequest`:

```rust
/// Provider prompt-cache routing key when supported.
#[serde(skip_serializing_if = "Option::is_none")]
pub prompt_cache_key: Option<String>,
```

- [ ] **Step 4: Wire OpenAI request construction**

In `openai.rs`, import `prompt_cache_key_from_conversation_id` from `responses.rs` and set:

```rust
prompt_cache_key: prompt_cache_key_from_conversation_id(request.conversation_id.as_deref()),
```

inside the `ResponsesRequest` literal.

Update any `ResponsesRequest` test literals in `responses.rs` or adapter tests to include:

```rust
prompt_cache_key: None,
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p noema-core sends_prompt_cache_key_for_conversation_requests
cargo test -p noema-core sends_expected_request_and_extracts_text
cargo test -p noema-core sends_codex_prompt_cache_key_for_conversation_requests
```

Expected: all PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider/adapters/responses.rs crates/noema-core/src/provider/adapters/openai.rs crates/noema-core/src/provider/adapters/codex_responses.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "Add OpenAI prompt cache key"
```

Expected: commit contains only provider adapter cache-key changes.

---

### Task 3: Provider Capability Gates For Encrypted Reasoning

**Files:**
- Modify: `crates/noema-core/src/provider/tools.rs`
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
- Test: adapter unit tests in the same files.

**Interfaces:**
- Produces: explicit provider capability flags so unsupported optional Responses fields are not sent optimistically.

- [ ] **Step 1: Write failing capability tests**

In `openai.rs`, extend `advertises_openai_responses_native_tool_capabilities`:

```rust
assert!(capabilities.prompt_cache_key);
assert!(capabilities.encrypted_reasoning);
```

In `codex_responses.rs`, extend `advertises_codex_responses_native_tool_capabilities`:

```rust
assert!(capabilities.prompt_cache_key);
assert!(!capabilities.prompt_cache_retention);
assert_eq!(capabilities.encrypted_reasoning, codex_encrypted_reasoning_supported());
```

Add a private helper in `codex_responses.rs` for the initial conservative gate:

```rust
fn codex_encrypted_reasoning_supported() -> bool {
    false
}
```

This helper makes the Codex behavior explicit until independent live/API verification proves support.

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test -p noema-core advertises_openai_responses_native_tool_capabilities
cargo test -p noema-core advertises_codex_responses_native_tool_capabilities
```

Expected: FAIL because `ProviderToolCapabilities` does not yet expose the new fields.

- [ ] **Step 3: Add fields to provider capabilities**

In `provider/tools.rs`, extend `ProviderToolCapabilities`:

```rust
/// Whether provider requests support a stable prompt cache key.
pub prompt_cache_key: bool,
/// Whether provider requests support encrypted reasoning include/replay.
pub encrypted_reasoning: bool,
```

Update `Default` to set both to `false`.

Update every `ProviderToolCapabilities` literal in tests and providers with explicit values. Use:

```rust
prompt_cache_key: false,
encrypted_reasoning: false,
```

unless the adapter is OpenAI or Codex and the test specifically covers support.

- [ ] **Step 4: Advertise OpenAI and Codex gates**

In `openai.rs`, set:

```rust
prompt_cache_key: true,
encrypted_reasoning: true,
```

In `codex_responses.rs`, set:

```rust
prompt_cache_key: true,
encrypted_reasoning: codex_encrypted_reasoning_supported(),
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p noema-core advertises_openai_responses_native_tool_capabilities
cargo test -p noema-core advertises_codex_responses_native_tool_capabilities
cargo test -p noema-core native_provider_turn_request_includes_builtin_tools
```

Expected: all PASS.

- [ ] **Step 6: Commit Task 3**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider/tools.rs crates/noema-core/src/provider/adapters/openai.rs crates/noema-core/src/provider/adapters/codex_responses.rs crates/noema-core/src/daemon/tests.rs crates/noema-core/src/daemon/runtime/model_tools.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "Gate encrypted reasoning by provider capability"
```

Expected: commit contains capability-field additions and literal updates only.

---

### Task 4: Responses Reasoning Request And Adapter Parsing

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`

**Interfaces:**
- Produces:
  - `GenerateInputItem::Reasoning(GenerateReasoningInput)`
  - `GenerateResponse.reasoning_items: Vec<GenerateReasoningItem>`
  - Responses request `include: ["reasoning.encrypted_content"]` when capability says supported.

- [ ] **Step 1: Write failing OpenAI include/parsing test**

In `openai.rs`, add:

```rust
#[tokio::test]
async fn openai_requests_and_parses_encrypted_reasoning_items() {
    let (base_url, request_rx) = spawn_server(
        200,
        r#"{
          "id": "resp_test",
          "model": "gpt-test",
          "output": [
            {
              "type": "reasoning",
              "id": "rs_1",
              "encrypted_content": "opaque-openai-reasoning"
            },
            {
              "type": "message",
              "content": [{"type": "output_text", "text": "Done"}]
            }
          ]
        }"#,
    )
    .await;
    let provider = test_provider(base_url);

    let response = provider
        .generate(GenerateRequest {
            conversation_id: Some("conversation:reasoning".to_string()),
            model: Some("gpt-test".to_string()),
            input: GenerateInput::Text("Think privately.".to_string()),
            ..GenerateRequest::text("ignored")
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert_eq!(body["include"][0], "reasoning.encrypted_content");
    assert_eq!(response.reasoning_items.len(), 1);
    assert_eq!(response.reasoning_items[0].id.as_deref(), Some("rs_1"));
    assert_eq!(
        response.reasoning_items[0].encrypted_content.as_deref(),
        Some("opaque-openai-reasoning")
    );
}
```

- [ ] **Step 2: Write failing shared replay serialization test**

In `responses.rs`, add a test near the typed history tests:

```rust
#[test]
fn serializes_reasoning_history_item_for_replay() {
    let input = GenerateInput::Items(vec![GenerateInputItem::Reasoning(
        GenerateReasoningInput {
            id: Some("rs_1".to_string()),
            encrypted_content: "opaque-openai-reasoning".to_string(),
        },
    )]);

    let ResponsesInput::Items(items) = ResponsesInput::from(&input) else {
        panic!("expected items");
    };
    let value = serde_json::to_value(&items[0]).expect("json");
    assert_eq!(value["type"], "reasoning");
    assert_eq!(value["id"], "rs_1");
    assert_eq!(value["encrypted_content"], "opaque-openai-reasoning");
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p noema-core openai_requests_and_parses_encrypted_reasoning_items
cargo test -p noema-core serializes_reasoning_history_item_for_replay
```

Expected: FAIL because reasoning types and include fields do not exist.

- [ ] **Step 4: Add provider-neutral reasoning types**

In `provider/contract.rs`, add:

```rust
/// Provider-neutral encrypted reasoning item for stateless replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateReasoningInput {
    /// Provider reasoning item id, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Opaque provider-encrypted reasoning payload.
    pub encrypted_content: String,
}

/// Encrypted reasoning item returned by a provider response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateReasoningItem {
    /// Provider reasoning item id, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Opaque provider-encrypted reasoning payload.
    pub encrypted_content: Option<String>,
}
```

Add `Reasoning(GenerateReasoningInput)` to `GenerateInputItem`.

Add to `GenerateResponse`:

```rust
pub reasoning_items: Vec<GenerateReasoningItem>,
```

Update constructors such as `GenerateResponse::from_parsed` and `GenerateResponse::final_text` to use `Vec::new()`.

Update `GenerateInputItem::is_empty`:

```rust
Self::Reasoning(reasoning) => reasoning.encrypted_content.trim().is_empty(),
```

Update `GenerateInputItem::render_for_token_count`:

```rust
Self::Reasoning(reasoning) => serde_json::json!({
    "type": "reasoning",
    "id": reasoning.id,
    "encrypted_content": reasoning.encrypted_content,
}).to_string(),
```

- [ ] **Step 5: Add shared Responses request include and item serialization**

In `responses.rs`, add to `ResponsesRequest`:

```rust
/// Additional provider output fields to include in responses.
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub include: Vec<&'static str>,
```

Add `ResponsesInputItem::Reasoning(ResponsesReasoningItem)` and conversion from `GenerateInputItem::Reasoning`.

Use:

```rust
#[derive(Debug, Clone, Serialize)]
struct ResponsesReasoningItem {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    encrypted_content: String,
}
```

- [ ] **Step 6: Parse reasoning output items**

Extend `ResponsesOutputItem`:

```rust
#[serde(rename = "reasoning")]
Reasoning {
    id: Option<String>,
    encrypted_content: Option<String>,
},
```

Add a method on `ResponsesApiResponse`:

```rust
fn reasoning_items(&self) -> Vec<GenerateReasoningItem> {
    self.output
        .iter()
        .filter_map(|item| match item {
            ResponsesOutputItem::Reasoning { id, encrypted_content } => {
                encrypted_content.as_ref().map(|encrypted_content| GenerateReasoningItem {
                    id: id.clone(),
                    encrypted_content: Some(encrypted_content.clone()),
                })
            }
            _ => None,
        })
        .collect()
}
```

When constructing `GenerateResponse`, set `reasoning_items` from the parsed response.

- [ ] **Step 7: Wire OpenAI include**

In `openai.rs`, set:

```rust
include: if self.tool_capabilities(Some(&model)).encrypted_reasoning {
    vec!["reasoning.encrypted_content"]
} else {
    Vec::new()
},
```

inside `ResponsesRequest`.

Keep Codex `include` disabled until Task 7 verifies Codex support independently.

- [ ] **Step 8: Run focused tests**

Run:

```bash
cargo test -p noema-core openai_requests_and_parses_encrypted_reasoning_items
cargo test -p noema-core serializes_reasoning_history_item_for_replay
cargo test -p noema-core sends_expected_request_and_extracts_text
cargo test -p noema-core sends_codex_typed_history_items_as_native_response_items
```

Expected: all PASS.

- [ ] **Step 9: Commit Task 4**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider/contract.rs crates/noema-core/src/provider/adapters/responses.rs crates/noema-core/src/provider/adapters/openai.rs crates/noema-core/src/provider/adapters/codex_responses.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "Add encrypted reasoning response support"
```

Expected: commit contains provider-neutral reasoning types and OpenAI include/parsing support.

---

### Task 5: Persist And Replay Encrypted Reasoning Items

**Files:**
- Modify: `crates/noema-core/src/conversation/status.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/conversations.rs`
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/daemon/runtime/prompt_context.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Consumes: `GenerateResponse.reasoning_items`.
- Produces: durable non-visible `conversation_items` rows that replay as `GenerateInputItem::Reasoning`.

- [ ] **Step 1: Write failing runtime replay test**

Add a new fake-provider scenario in `daemon/tests.rs`:

```rust
ReasoningReplay,
```

In `FakeCodexProvider::generate_response`, add:

```rust
FakeCodexScenario::ReasoningReplay => {
    let saw_reasoning_replay = match &request.input {
        GenerateInput::Items(items) => items.iter().any(|item| {
            matches!(
                item,
                GenerateInputItem::Reasoning(reasoning)
                    if reasoning.encrypted_content == "opaque-turn-one"
            )
        }),
        _ => false,
    };
    let mut response = fake_generate_response(
        vec![GenerateOutputItem::AssistantText {
            phase: None,
            text: if saw_reasoning_replay {
                "saw encrypted reasoning"
            } else {
                "first answer"
            }
            .to_string(),
        }],
        "codex",
        model,
    );
    if !saw_reasoning_replay {
        response.reasoning_items.push(GenerateReasoningItem {
            id: Some("rs_fake_1".to_string()),
            encrypted_content: Some("opaque-turn-one".to_string()),
        });
    }
    response
}
```

Add this test:

```rust
#[tokio::test]
async fn runtime_persists_and_replays_encrypted_reasoning_items() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::ReasoningReplay)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let first_items = collect_turn(&handle, conversation_id.clone(), "first".to_string())
        .await
        .expect("first turn");
    assert!(first_items.iter().any(|item| {
        matches!(item.as_ref(), TurnTranscriptItem::AssistantText { text, .. } if text == "first answer")
    }));

    let second_items = collect_turn(&handle, conversation_id, "second".to_string())
        .await
        .expect("second turn");
    assert!(second_items.iter().any(|item| {
        matches!(item.as_ref(), TurnTranscriptItem::AssistantText { text, .. } if text == "saw encrypted reasoning")
    }));

    handle.shutdown().await;
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
cargo test -p noema-core runtime_persists_and_replays_encrypted_reasoning_items
```

Expected: FAIL because reasoning items are not persisted or replayed.

- [ ] **Step 3: Add conversation item kind**

In `conversation/status.rs`, add:

```rust
/// Provider-encrypted reasoning state used only for stateless provider replay.
Reasoning,
```

Map it to storage string:

```rust
Self::Reasoning => "reasoning",
```

Parse it:

```rust
"reasoning" => Ok(Self::Reasoning),
```

In `store/schema.rs`, add `'reasoning'` to the `conversation_items.kind` enum assertion.

- [ ] **Step 4: Include reasoning rows in model context query**

In `store/conversations.rs`, update `list_all_conversation_items_after_sequence_for_context`:

```sql
AND kind IN ['user_text', 'assistant_text', 'tool_call', 'tool_result', 'reasoning']
```

- [ ] **Step 5: Persist reasoning rows**

In `transcript_persistence.rs`, add a method on `CodexRuntimeActor`:

```rust
pub(super) async fn persist_provider_reasoning_items(
    &self,
    conversation_id: &str,
    turn_id: &str,
    reasoning_items: &[GenerateReasoningItem],
) -> Result<(), DaemonError> {
    for reasoning in reasoning_items {
        let Some(encrypted_content) = reasoning
            .encrypted_content
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            continue;
        };
        self.store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation_id.to_string(),
                turn_id: Some(turn_id.to_string()),
                parent_item_id: None,
                kind: ConversationItemKind::Reasoning,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary"),
                content_text: None,
                payload_json: serde_json::json!({
                    "provider_reasoning": {
                        "id": reasoning.id,
                        "encrypted_content": encrypted_content
                    }
                }),
                metadata: serde_json::json!({}),
            })
            .await?;
    }
    Ok(())
}
```

- [ ] **Step 6: Call persistence after provider responses**

In `turn.rs`, after each successful provider response is received and before later planning/replay can occur, call:

```rust
self.persist_provider_reasoning_items(
    &conversation_id,
    &turn.turn_id,
    &response.reasoning_items,
)
.await?;
```

Do this for initial provider responses and continuation responses that can produce reasoning items.

- [ ] **Step 7: Replay reasoning rows**

In `prompt_context.rs`, add to `input_item_from_transcript_item`:

```rust
ConversationItemKind::Reasoning => reasoning_input_item(item),
```

Add:

```rust
fn reasoning_input_item(item: &ConversationItemRecord) -> Option<GenerateInputItem> {
    let value = item.payload_json.pointer("/provider_reasoning")?;
    let encrypted_content = action_string(value, "encrypted_content")?;
    Some(GenerateInputItem::Reasoning(GenerateReasoningInput {
        id: action_string(value, "id"),
        encrypted_content,
    }))
}
```

Use a helper name other than `action_string` if the existing helper is too action-specific after this change.

- [ ] **Step 8: Hide reasoning from visible replay**

In `daemon/web/replay.rs`, ensure `ConversationItemKind::Reasoning` returns `Ok(None)` like tool internals and approvals:

```rust
ConversationItemKind::Reasoning
| ConversationItemKind::ToolCall
| ConversationItemKind::ToolResult
| ConversationItemKind::ApprovalRequest
| ConversationItemKind::ApprovalResult => Ok(None),
```

- [ ] **Step 9: Run focused tests**

Run:

```bash
cargo test -p noema-core runtime_persists_and_replays_encrypted_reasoning_items
cargo test -p noema-core prompt_context_uses_active_summary_and_post_checkpoint_items
cargo test -p noema-core transcript_replay
```

If `transcript_replay` matches no tests, run:

```bash
cargo test -p noema-core replay
```

Expected: all matching tests PASS.

- [ ] **Step 10: Commit Task 5**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/conversation/status.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/conversations.rs crates/noema-core/src/daemon/runtime/transcript_persistence.rs crates/noema-core/src/daemon/runtime/prompt_context.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/web/replay.rs crates/noema-core/src/daemon/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "Replay encrypted reasoning history"
```

Expected: commit contains reasoning persistence and replay only.

---

### Task 6: Compaction Boundary Regression

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`
- Modify: `crates/noema-core/src/daemon/runtime/context_compaction.rs` only if the test reveals ordering bugs.

**Interfaces:**
- Consumes: active context summary rows and post-checkpoint context replay.
- Produces: regression coverage that compaction is the only expected prefix reset.

- [ ] **Step 1: Write failing or confirming compaction test**

Add this test near `prompt_context_uses_active_summary_and_post_checkpoint_items`:

```rust
#[tokio::test]
async fn compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text() {
    let store = crate::store::tests::test_store().await;
    store.ensure_default_actors().await.expect("actors");
    let provider = Arc::new(MetadataCapturingProvider::default());
    let runtime = CodexRuntimeHandle::spawn_with_provider_kind(
        provider.clone(),
        store.clone(),
        "foundation_local",
    )
    .await
    .expect("runtime");
    let started = runtime
        .start_conversation(None)
        .await
        .expect("conversation");

    append_test_text_item(&store, &started.conversation_id, "covered user").await;
    append_test_text_item(&store, &started.conversation_id, "covered assistant").await;
    store
        .insert_conversation_context_summary(crate::NewConversationContextSummary {
            conversation_id: started.conversation_id.clone(),
            provider_kind: "foundation_local".to_string(),
            model_profile: None,
            summary_text: "Summary: compacted checkpoint facts.".to_string(),
            covered_item_start_sequence: 1,
            covered_item_end_sequence: 2,
            source_item_ids: vec!["item:1".to_string(), "item:2".to_string()],
            input_token_estimate: 400,
            summary_token_estimate: 16,
            compaction_provider_kind: "foundation_local".to_string(),
            compaction_model_profile: None,
            status: crate::ConversationContextSummaryStatus::Active,
            error_code: None,
            error_message: None,
        })
        .await
        .expect("summary");
    append_test_text_item(&store, &started.conversation_id, "post checkpoint user").await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    runtime
        .turn(started.conversation_id, "current turn".to_string(), tx)
        .await
        .expect("turn");
    while rx.recv().await.is_some() {}
    runtime.shutdown().await;

    let requests = provider.requests.lock().expect("requests");
    let request = requests
        .iter()
        .find(|request| request.options.require_noema_response)
        .expect("agent request");
    let instructions = request.instructions.as_deref().expect("instructions");
    assert!(!instructions.contains("compacted checkpoint facts"));
    let input_texts = input_message_texts(&request.input);
    assert_eq!(
        input_texts.first().map(String::as_str),
        Some("Compacted conversation context:\nSummary: compacted checkpoint facts.\n\nRecent transcript after this compacted checkpoint follows in subsequent messages.")
    );
    assert!(input_texts.iter().any(|text| text == "post checkpoint user"));
    assert!(input_texts.iter().any(|text| text == "current turn"));
    assert!(!input_texts.iter().any(|text| text == "covered user"));
}
```

- [ ] **Step 2: Run the test**

Run:

```bash
cargo test -p noema-core compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text
```

Expected: PASS after Task 1.

- [ ] **Step 3: Inspect summary placement if the test fails**

If this test fails, stop and inspect `prompt_context.rs`; the required fix is to keep active summaries as the first replay input item and keep them out of `instructions`.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p noema-core compacted_summary_is_replayed_as_input_checkpoint_not_instruction_text
cargo test -p noema-core prompt_context_uses_active_summary_and_post_checkpoint_items
```

Expected: all PASS.

- [ ] **Step 5: Commit Task 6**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/daemon/tests.rs crates/noema-core/src/daemon/runtime/context_compaction.rs crates/noema-core/src/daemon/runtime/prompt_context.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "Cover compaction prompt cache boundary"
```

Expected: commit contains only compaction boundary test/fix files.

---

### Task 7: Codex Responses Independent Verification

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
- Modify: `docs/context/current.md`

**Interfaces:**
- Consumes: Codex provider behavior from local mock tests first, and live verification only if explicitly requested and credentials are available.
- Produces: documented Codex-specific gate for encrypted reasoning include/replay.

- [ ] **Step 1: Add Codex unsupported-field fallback test**

In `codex_responses.rs`, add a test:

```rust
#[tokio::test]
async fn codex_omits_encrypted_reasoning_include_until_verified() {
    let (base_url, request_rx) = spawn_server(
        200,
        "event: response.output_text.delta\n\
         data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\n\
         \n\
         event: response.completed\n\
         data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\"}}\n\
         \n",
    )
    .await;
    let (provider, _dir) = provider_with_tokens(base_url);

    provider
        .generate(GenerateRequest {
            conversation_id: Some("conversation:codex-reasoning".to_string()),
            ..GenerateRequest::text("Hello?")
        })
        .await
        .expect("response");

    let captured = request_rx.await.expect("captured request");
    let body: Value = serde_json::from_str(&captured.body).expect("json body");
    assert!(body.get("include").is_none());
}
```

- [ ] **Step 2: Run Codex gate test**

Run:

```bash
cargo test -p noema-core codex_omits_encrypted_reasoning_include_until_verified
```

Expected: PASS if Task 4 kept Codex include disabled; FAIL if it was sent optimistically.

- [ ] **Step 3: Add Codex parsing coverage independent of request include**

Add a Codex SSE parsing test using a mocked Codex response with `reasoning` output in `response.completed`:

```rust
#[tokio::test]
async fn codex_parses_encrypted_reasoning_items_when_returned() {
    let (base_url, _request_rx) = spawn_server(
        200,
        "event: response.completed\n\
         data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test\",\"model\":\"gpt-test\",\"status\":\"completed\",\"output\":[{\"type\":\"reasoning\",\"id\":\"rs_1\",\"encrypted_content\":\"opaque-codex-reasoning\"},{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"Done\"}]}]}}\n\
         \n",
    )
    .await;
    let (provider, _dir) = provider_with_tokens(base_url);

    let response = provider
        .generate(GenerateRequest::text("Hello?"))
        .await
        .expect("response");

    assert_eq!(response.assistant_text(), "Done");
    assert_eq!(response.reasoning_items.len(), 1);
    assert_eq!(
        response.reasoning_items[0].encrypted_content.as_deref(),
        Some("opaque-codex-reasoning")
    );
}
```

- [ ] **Step 4: Run Codex parsing test**

Run:

```bash
cargo test -p noema-core codex_parses_encrypted_reasoning_items_when_returned
```

Expected: PASS if shared SSE parsing handles reasoning items.

- [ ] **Step 5: Document Codex live-verification status**

Update `docs/context/current.md` with one concise bullet:

```markdown
- Stable prompt-cache work treats OpenAI and Codex Responses as separate dialects:
  OpenAI can request `reasoning.encrypted_content` once adapter tests cover it,
  while Codex must keep encrypted-reasoning include gated off until live/provider
  verification confirms the request field and replay shape.
```

- [ ] **Step 6: Commit Task 7**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/src/provider/adapters/codex_responses.rs docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "Verify Codex reasoning gate"
```

Expected: commit contains Codex-specific tests and current-context note.

---

### Task 8: Final Validation

**Files:**
- No planned code changes.

**Interfaces:**
- Consumes: all previous task commits.
- Produces: validated working branch.

- [ ] **Step 1: Run formatting check**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 2: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Run unit tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS.

- [ ] **Step 5: Inspect final diff**

Run:

```bash
git status --short --branch
git diff --check
git diff --stat
```

Expected: only intentional tracked changes remain, or the worktree is clean after task commits. Report any unrelated dirty files.

- [ ] **Step 6: Confirm no final docs commit is needed**

Run:

```bash
git status --short --branch
```

Expected: no unstaged changes from the implementation remain except unrelated pre-existing dirty files reported to the user.
