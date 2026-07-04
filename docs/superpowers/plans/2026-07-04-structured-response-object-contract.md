# Structured Response Object Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Noema's ordered provider `output` array contract with an explicit object contract using `responses`, `tool_calls`, `response_status`, and top-level `memory_proposals`.

**Architecture:** The provider-facing Noema response object should describe semantic intent, not transcript ordering. Model-visible outputs live in `responses[]`, executable actions live in `tool_calls[]`, memory candidates live in `memory_proposals[]`, and the daemon projects those fields into transcript rows and runtime tool lifecycle rows. Empty `responses` is valid for intermediate tool-only turns so the assistant can run routine tool calls without filler commentary.

**Tech Stack:** Rust provider contract and daemon runtime, serde JSON parsing, existing Noema streaming extractor, SurrealDB-backed transcript persistence, existing React transcript renderer.

---

## Target Contract

The required provider response shape becomes:

```json
{
  "response_status": "final",
  "responses": [
    {
      "kind": "text",
      "phase": "final_answer",
      "text": "Here is the answer."
    }
  ],
  "tool_calls": [],
  "memory_proposals": []
}
```

Silent intermediate tool use is valid:

```json
{
  "response_status": "needs_tools",
  "responses": [],
  "tool_calls": [
    {
      "id": "call_search_1",
      "name": "mcp.web.search",
      "payload": { "query": "OpenAI Codex tool lifecycle" }
    },
    {
      "id": "call_search_2",
      "name": "mcp.web.search",
      "payload": { "query": "OpenAI Responses API commentary final answer phase" }
    }
  ],
  "memory_proposals": []
}
```

Commentary before tools is also valid:

```json
{
  "response_status": "needs_tools",
  "responses": [
    {
      "kind": "text",
      "phase": "commentary",
      "text": "I will check memory and the connected docs."
    }
  ],
  "tool_calls": [
    {
      "id": "call_memory_1",
      "name": "search_memory",
      "payload": {
        "scope_ids": ["human:local"],
        "query": "trains",
        "purpose": "answer_human_question",
        "limit": 8
      }
    }
  ],
  "memory_proposals": []
}
```

Validation invariants:

- `response_status` must be `"needs_tools"` or `"final"`.
- `responses` must be an array. It may be empty only when `response_status` is `"needs_tools"` and `tool_calls` is non-empty.
- `tool_calls` must be an array. It may contain multiple independent calls.
- When `tool_calls` is non-empty, `response_status` must be `"needs_tools"`.
- When `response_status` is `"final"`, `tool_calls` must be empty and `responses` must contain at least one non-empty response item.
- `memory_proposals` must always be present as an array.
- Text response phases are `"commentary"` for pre-tool/progress text and `"final_answer"` for terminal answer text.
- A `final_answer` text response is valid only when `response_status` is `"final"`.

## Scope

This plan intentionally removes the strict provider-facing `output: [...]` array contract. No backwards compatibility layer is required for required Noema responses because the project is pre-V1 and the prompt plus tests are updated in the same slice. Plain text fallback for non-required provider calls remains available for metadata-only helpers that still parse arbitrary provider text.

Frontend transcript rendering should not need changes. The daemon still persists ordinary assistant text rows, structured card rows, tool activity rows, and memory activity rows in the same GraphQL transcript shape.

## File Structure

- Modify: `crates/noema-core/src/provider/contract.rs`
  - Add `GenerateResponseStatus`.
  - Add `GenerateResponseItem` for `responses[]`.
  - Add `GenerateToolCall` for `tool_calls[]`.
  - Change `GenerateResponse` from `output: Vec<GenerateOutputItem>` to explicit fields.
  - Keep a small runtime-only action enum for tool results, approvals, and provider action persistence.
  - Parse the new response object and reject old required `output` objects.
- Modify: `crates/noema-core/src/provider.rs`
  - Re-export the new contract types.
- Modify: `crates/noema-core/src/provider/adapters/noema_response_stream.rs`
  - Stream text deltas from `responses[].text`.
  - Emit tool-call-start events from `tool_calls[].name`.
  - Emit memory-proposals-started from top-level `memory_proposals[]`.
- Modify: `crates/noema-core/src/provider/adapters/{openai.rs,codex_responses.rs,foundation_local.rs}`
  - Construct `GenerateResponse` with the new fields.
  - Keep provider-specific HTTP/SSE plumbing unchanged.
- Modify: `crates/noema-core/src/daemon/prompts.rs`
  - Replace the model-facing JSON shape and rules.
  - Explicitly allow `responses: []` for silent intermediate tool calls.
  - Allow multiple independent tool calls in one response.
- Modify: `crates/noema-core/src/daemon/runtime/tool_lifecycle.rs`
  - Extract local tool calls from `GenerateResponse.tool_calls`.
  - Preserve provider tool-call indexes for stable activity ids.
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
  - Accept `GenerateToolCall`-backed local tool calls.
  - Keep tool result payload normalization unchanged.
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
  - Persist `GenerateResponseItem::Text` and `GenerateResponseItem::Structured`.
  - Persist runtime action items for tool results and approvals.
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
  - Persist `responses[]` first.
  - Execute `tool_calls[]` with runtime-started and runtime-result rows.
  - Use `response_status` to detect protocol mistakes.
- Modify: `crates/noema-core/src/daemon/runtime/context_compaction.rs`
  - Read assistant text from `GenerateResponse.responses`.
- Modify: `crates/noema-core/src/daemon/tests.rs`
  - Update fake providers and regression tests to the object contract.
  - Add silent multi-tool-call coverage.
- Modify: `crates/noema-core/src/graphql/schema.rs`
  - Update any test-only or resolver-only `GenerateResponse` constructors.
- Modify: `docs/context/current.md`
  - Record the new provider contract as durable harness context.

---

### Task 1: Add Structured Response Contract Types

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/provider.rs`

- [ ] **Step 1: Write failing contract tests for the new response object**

Add these tests to `crates/noema-core/src/provider/contract.rs` inside the existing `#[cfg(test)] mod tests`.

```rust
#[test]
fn required_noema_response_accepts_object_contract_final_text() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[],"memory_proposals":[]}"#
            .to_string(),
    )
    .expect("required structured response");

    assert_eq!(response.response_status, GenerateResponseStatus::Final);
    assert_eq!(
        response.responses,
        vec![GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::FinalAnswer),
            text: "Done.".to_string(),
        }]
    );
    assert!(response.tool_calls.is_empty());
    assert!(response.memory_proposals.is_empty());
}

#[test]
fn required_noema_response_accepts_silent_tool_calls() {
    let response = required_noema_response_from_text(
        r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{"scope_ids":["human:local"],"query":"","purpose":"answer_human_question","limit":8}}],"memory_proposals":[]}"#
            .to_string(),
    )
    .expect("silent tool call response");

    assert_eq!(response.response_status, GenerateResponseStatus::NeedsTools);
    assert!(response.responses.is_empty());
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].id.as_deref(), Some("call_1"));
    assert_eq!(response.tool_calls[0].name, "search_memory");
}

#[test]
fn required_noema_response_rejects_final_without_responses() {
    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[],"tool_calls":[],"memory_proposals":[]}"#
            .to_string(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message == "Noema final response did not include any response items"
    ));
}

#[test]
fn required_noema_response_rejects_tool_calls_in_final_response() {
    let error = required_noema_response_from_text(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Done."}],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}],"memory_proposals":[]}"#
            .to_string(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message == "Noema final response cannot include tool_calls"
    ));
}

#[test]
fn required_noema_response_rejects_old_output_array_contract() {
    let error = required_noema_response_from_text(
        r#"{"output":[{"kind":"assistant_text","text":"old"}]}"#
            .to_string(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        ProviderError::MalformedResponse { message }
            if message.contains("missing field `response_status`")
    ));
}
```

- [ ] **Step 2: Run the failing tests**

Run:

```bash
cargo test -p noema-core provider::contract::tests
```

Expected: compile failure because `required_noema_response_from_text`, `GenerateResponseStatus`, `GenerateResponseItem`, and `GenerateToolCall` do not exist yet.

- [ ] **Step 3: Add the new provider response types**

In `crates/noema-core/src/provider/contract.rs`, replace the current `GenerateResponse` definition with:

```rust
/// Structured response returned by a model provider.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateResponse {
    /// User-visible response items returned by the provider.
    pub responses: Vec<GenerateResponseItem>,
    /// Tool calls requested by the provider.
    pub tool_calls: Vec<GenerateToolCall>,
    /// Memory proposals emitted by the provider.
    pub memory_proposals: Vec<ExtractorMemoryProposal>,
    /// Whether this response needs tool execution or completes the turn.
    pub response_status: GenerateResponseStatus,
    /// Provider identifier that produced the response.
    pub provider: String,
    /// Model identifier used by the provider.
    pub model: String,
    /// Provider response identifier when one is available.
    pub response_id: Option<String>,
    /// Token usage reported by the provider when available.
    pub usage: Option<TokenUsage>,
}

/// Provider-declared status for a Noema response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerateResponseStatus {
    /// The response contains tool calls that Noema should execute.
    NeedsTools,
    /// The response is the terminal assistant response for this turn.
    Final,
}

/// One user-visible response item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenerateResponseItem {
    /// Human-visible assistant text.
    Text {
        /// Whether the text is mid-turn commentary or the final answer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phase: Option<AssistantTextPhase>,
        /// Text to show in the transcript.
        text: String,
    },
    /// Future rich structured output payload.
    Structured {
        /// Stable schema identifier for the payload.
        schema: String,
        /// Provider-produced payload for that schema.
        payload: Value,
    },
}

/// One provider-requested tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateToolCall {
    /// Provider item id or tool-call id, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Tool or operation name.
    pub name: String,
    /// Provider payload for audit and replay.
    #[serde(default)]
    pub payload: Value,
}
```

Add this runtime action enum next to the response item types:

```rust
/// Runtime action item persisted after provider output is interpreted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GenerateActionItem {
    /// Runtime or provider-reported tool invocation.
    ToolCall {
        id: Option<String>,
        name: String,
        payload: Value,
    },
    /// Runtime or provider-reported tool result.
    ToolResult {
        call_id: Option<String>,
        name: Option<String>,
        success: Option<bool>,
        payload: Value,
    },
    /// Provider request for approval or elicitation.
    ApprovalRequest {
        id: Option<String>,
        method: String,
        payload: Value,
    },
    /// Recorded approval or elicitation decision.
    ApprovalResult {
        request_id: Option<String>,
        decision: String,
        payload: Value,
    },
}
```

- [ ] **Step 4: Add helper methods on `GenerateResponse`**

Replace the current `impl GenerateResponse` methods with:

```rust
impl GenerateResponse {
    /// Return all text response items concatenated in order.
    #[must_use]
    pub fn assistant_text(&self) -> String {
        self.responses
            .iter()
            .filter_map(|item| match item {
                GenerateResponseItem::Text { text, .. } => Some(text.as_str()),
                GenerateResponseItem::Structured { .. } => None,
            })
            .collect()
    }

    /// Return whether the response asks Noema to execute any tools.
    #[must_use]
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}
```

Remove the old `memory_proposals()` method because `memory_proposals` is now a direct field.

- [ ] **Step 5: Update `AssistantTextPhase` to use response items**

Replace `effective_for_output` with:

```rust
/// Infer a display phase for provider text that omitted phase.
#[must_use]
pub fn effective_for_response_item(
    item: &GenerateResponseItem,
    provider_phase_has_tools: bool,
) -> Self {
    match item {
        GenerateResponseItem::Text {
            phase: Some(phase), ..
        } => *phase,
        GenerateResponseItem::Text { phase: None, .. } if provider_phase_has_tools => {
            Self::Commentary
        }
        GenerateResponseItem::Text { phase: None, .. } => Self::FinalAnswer,
        GenerateResponseItem::Structured { .. } => Self::FinalAnswer,
    }
}
```

- [ ] **Step 6: Re-export new types**

In `crates/noema-core/src/provider.rs`, update the public re-exports:

```rust
pub use contract::{
    AssistantTextPhase, DEFAULT_TOOL_CLASSIFICATION_MODEL, GenerateActionItem, GenerateInput,
    GenerateMessage, GenerateMessageRole, GenerateOptions, GenerateRequest, GenerateResponse,
    GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent, GenerateToolCall,
    ModelProvider, PromptCacheRetention, ProviderContextMetadata, ProviderError, TokenUsage,
    noema_response_from_text, output_items_from_text, required_noema_response_from_text,
};
```

- [ ] **Step 7: Run the focused type tests**

Run:

```bash
cargo test -p noema-core provider::contract::tests
```

Expected: compile failures remain in parser functions and old call sites. Those failures are fixed in Tasks 2 and 3 before committing.

---

### Task 2: Parse And Validate The Object Envelope

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`

- [ ] **Step 1: Replace response parsing functions**

Keep `output_items_from_text` only for non-required plain text compatibility. Add these functions below `output_items_from_text`:

```rust
/// Parse provider text that must be a Noema structured response.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when the payload is not a
/// strict Noema response object contract.
pub fn required_noema_response_from_text(
    text: String,
) -> Result<ParsedNoemaResponse, ProviderError> {
    noema_response_from_text_with_mode(text, true)
}

/// Parse provider text into a Noema structured response when it contains the
/// response object contract.
///
/// # Errors
///
/// Returns [`ProviderError::MalformedResponse`] when a response-shaped object
/// is present but invalid.
pub fn noema_response_from_text(text: String) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return noema_response_from_structured_value(value, false);
    }
    embedded_required_noema_response(trimmed)
}
```

Add this parsed response type:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNoemaResponse {
    pub responses: Vec<GenerateResponseItem>,
    pub tool_calls: Vec<GenerateToolCall>,
    pub memory_proposals: Vec<ExtractorMemoryProposal>,
    pub response_status: GenerateResponseStatus,
}
```

- [ ] **Step 2: Add serde response object structs**

Replace `GenerateOutputEnvelope` with:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoemaResponseObject {
    response_status: GenerateResponseStatus,
    responses: Vec<GenerateResponseItem>,
    tool_calls: Vec<GenerateToolCall>,
    memory_proposals: Vec<ExtractorMemoryProposal>,
}
```

- [ ] **Step 3: Implement response object parsing**

Add:

```rust
fn noema_response_from_text_with_mode(
    text: String,
    require_noema_response: bool,
) -> Result<ParsedNoemaResponse, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(response) = noema_response_from_structured_value(value, require_noema_response)? {
            return Ok(response);
        }
        if require_noema_response {
            return Err(ProviderError::MalformedResponse {
                message: "provider did not return a Noema structured response object".to_string(),
            });
        }
    }

    if require_noema_response {
        if let Some(response) = embedded_required_noema_response(trimmed)? {
            return Ok(response);
        }
    }

    Err(ProviderError::MalformedResponse {
        message: "provider did not return a Noema structured response object".to_string(),
    })
}

fn noema_response_from_structured_value(
    value: Value,
    require_noema_response: bool,
) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    if !looks_like_noema_response_object(&value) && !require_noema_response {
        return Ok(None);
    }

    let response_object: NoemaResponseObject =
        serde_json::from_value(value).map_err(|source| ProviderError::MalformedResponse {
            message: format!("invalid Noema structured response: {source}"),
        })?;
    let parsed = ParsedNoemaResponse {
        responses: response_object.responses,
        tool_calls: response_object.tool_calls,
        memory_proposals: response_object.memory_proposals,
        response_status: response_object.response_status,
    };
    if require_noema_response {
        validate_required_noema_response(&parsed)?;
    }
    Ok(Some(parsed))
}

fn looks_like_noema_response_object(value: &Value) -> bool {
    value.get("response_status").is_some()
        || value.get("responses").is_some()
        || value.get("tool_calls").is_some()
        || value.get("memory_proposals").is_some()
}
```

- [ ] **Step 4: Implement response object validation**

Replace `validate_required_noema_response_output` with:

```rust
fn validate_required_noema_response(response: &ParsedNoemaResponse) -> Result<(), ProviderError> {
    match response.response_status {
        GenerateResponseStatus::Final => {
            if !response.tool_calls.is_empty() {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema final response cannot include tool_calls".to_string(),
                });
            }
            if !has_non_empty_response_item(&response.responses) {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema final response did not include any response items".to_string(),
                });
            }
            if response.responses.iter().any(is_commentary_text_response) {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema final response cannot include commentary text".to_string(),
                });
            }
        }
        GenerateResponseStatus::NeedsTools => {
            if response.tool_calls.is_empty() {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema needs_tools response did not include tool_calls".to_string(),
                });
            }
            if response.responses.iter().any(is_final_answer_text_response) {
                return Err(ProviderError::MalformedResponse {
                    message: "Noema needs_tools response cannot include final_answer text".to_string(),
                });
            }
        }
    }
    Ok(())
}

fn has_non_empty_response_item(responses: &[GenerateResponseItem]) -> bool {
    responses.iter().any(|item| match item {
        GenerateResponseItem::Text { text, .. } => !text.trim().is_empty(),
        GenerateResponseItem::Structured { .. } => true,
    })
}

fn is_commentary_text_response(item: &GenerateResponseItem) -> bool {
    matches!(
        item,
        GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::Commentary),
            ..
        }
    )
}

fn is_final_answer_text_response(item: &GenerateResponseItem) -> bool {
    matches!(
        item,
        GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::FinalAnswer),
            ..
        }
    )
}
```

- [ ] **Step 5: Preserve plain text fallback for optional parsing**

Keep `output_items_from_text` as a narrow compatibility helper for metadata tasks:

```rust
pub fn output_items_from_text(text: String) -> Result<Vec<GenerateResponseItem>, ProviderError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ProviderError::MalformedResponse {
            message: "provider produced empty output".to_string(),
        });
    }
    if let Ok(Some(response)) = noema_response_from_text(trimmed.to_string()) {
        return Ok(response.responses);
    }
    Ok(vec![GenerateResponseItem::Text {
        phase: None,
        text,
    }])
}
```

- [ ] **Step 6: Parse embedded required response objects**

Replace `embedded_required_noema_response_output` with:

```rust
fn embedded_required_noema_response(
    text: &str,
) -> Result<Option<ParsedNoemaResponse>, ProviderError> {
    let mut output = None;
    for candidate in balanced_json_object_candidates(text) {
        let Ok(value) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        if !looks_like_noema_response_object(&value) {
            continue;
        }
        let Some(candidate_output) = noema_response_from_structured_value(value, true)? else {
            continue;
        };
        if output.is_some() {
            return Err(ProviderError::MalformedResponse {
                message: "provider returned multiple Noema structured response objects"
                    .to_string(),
            });
        }
        output = Some(candidate_output);
    }
    Ok(output)
}
```

- [ ] **Step 7: Run contract parser tests**

Run:

```bash
cargo test -p noema-core provider::contract::tests
```

Expected: contract tests compile and pass after updating old tests to the object contract. Existing tests that intentionally cover old `output` parsing should be changed to assert rejection for required Noema responses or plain text fallback for optional parsing.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/provider/contract.rs crates/noema-core/src/provider.rs
git commit -m "feat: add structured response object contract"
```

---

### Task 3: Update Provider Adapters And Constructors

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/openai.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`
- Modify: `crates/noema-core/src/provider/adapters/foundation_local.rs`
- Modify: `crates/noema-core/src/provider/adapters/sse.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add a constructor helper for text-only responses**

In `crates/noema-core/src/provider/contract.rs`, add:

```rust
impl GenerateResponse {
    /// Build a final text response for provider adapters and tests.
    #[must_use]
    pub fn final_text(
        text: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            responses: vec![GenerateResponseItem::Text {
                phase: None,
                text: text.into(),
            }],
            tool_calls: Vec::new(),
            memory_proposals: Vec::new(),
            response_status: GenerateResponseStatus::Final,
            provider: provider.into(),
            model: model.into(),
            response_id: None,
            usage: None,
        }
    }
}
```

- [ ] **Step 2: Update OpenAI and Codex response adapters**

In `openai.rs` and `codex_responses.rs`, replace parsed response construction:

```rust
let parsed = required_noema_response_from_text(text)?;
Ok(GenerateResponse {
    responses: parsed.responses,
    tool_calls: parsed.tool_calls,
    memory_proposals: parsed.memory_proposals,
    response_status: parsed.response_status,
    provider: "codex".to_string(),
    model,
    response_id,
    usage,
})
```

Use the provider string already used by each adapter. Keep current usage and response id logic unchanged.

- [ ] **Step 3: Update Foundation Local response adapter**

In `foundation_local.rs`, replace each `GenerateResponse` construction that currently sets the old `output` field with the same parsed response field mapping:

```rust
let parsed = required_noema_response_from_text(output_text)?;
Ok(GenerateResponse {
    responses: parsed.responses,
    tool_calls: parsed.tool_calls,
    memory_proposals: parsed.memory_proposals,
    response_status: parsed.response_status,
    provider: "foundation_local".to_string(),
    model,
    response_id,
    usage,
})
```

- [ ] **Step 4: Update fallback/mock constructors**

Replace simple test/mock constructors shaped like:

```rust
GenerateResponse {
    output: vec![GenerateOutputItem::AssistantText { phase: None, text }],
    provider: "mock".to_string(),
    model,
    response_id: Some("fake-response".to_string()),
    usage: None,
}
```

with:

```rust
GenerateResponse {
    responses: vec![GenerateResponseItem::Text { phase: None, text }],
    tool_calls: Vec::new(),
    memory_proposals: Vec::new(),
    response_status: GenerateResponseStatus::Final,
    provider: "mock".to_string(),
    model,
    response_id: Some("fake-response".to_string()),
    usage: None,
}
```

- [ ] **Step 5: Run compile to find remaining old constructors**

Run:

```bash
cargo check -p noema-core
```

Expected: compile errors point to remaining `.output`, `GenerateOutputItem::AssistantText`, or missing `GenerateResponse` fields. Update every constructor using the same new field pattern from Step 4.

- [ ] **Step 6: Run provider adapter tests**

Run:

```bash
cargo test -p noema-core provider::adapters
```

Expected: adapter tests pass after test JSON fixtures use the object contract.

- [ ] **Step 7: Commit**

```bash
git add crates/noema-core/src/provider/adapters crates/noema-core/src/graphql/schema.rs crates/noema-core/src/daemon/tests.rs crates/noema-core/src/provider/contract.rs
git commit -m "refactor: update providers for response object contract"
```

---

### Task 4: Stream Responses From `responses[]` And `tool_calls[]`

**Files:**
- Modify: `crates/noema-core/src/provider/adapters/noema_response_stream.rs`
- Modify: `crates/noema-core/src/provider/adapters/sse.rs`
- Modify: `crates/noema-core/src/provider/adapters/codex_responses.rs`

- [ ] **Step 1: Write streaming parser tests for the new shape**

Add these tests to `noema_response_stream.rs`:

```rust
#[test]
fn noema_stream_extractor_streams_text_from_responses_array() {
    let streamed_text = extract_streamed_text(&[
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Hel"#,
        r#"lo"}],"tool_calls":[],"memory_proposals":[]}"#,
    ]);

    assert_eq!(streamed_text, "Hello");
}

#[test]
fn noema_stream_extractor_emits_tool_call_started_from_tool_calls_array() {
    let mut extractor = NoemaAssistantTextDeltaExtractor::default();
    let mut events = Vec::new();
    extractor.push_delta(
        r#"{"response_status":"needs_tools","responses":[],"tool_calls":[{"id":"call_1","name":"search_memory","payload":{}}],"memory_proposals":[]}"#,
        &mut |event| events.push(event),
    );

    assert!(events.iter().any(|event| {
        matches!(
            event,
            GenerateStreamEvent::ToolCallStarted {
                output_index: 0,
                name
            } if name == "search_memory"
        )
    }));
}

#[test]
fn noema_stream_extractor_emits_memory_started_from_top_level_memory_proposals() {
    let mut extractor = NoemaAssistantTextDeltaExtractor::default();
    let mut events = Vec::new();
    extractor.push_delta(
        r#"{"response_status":"final","responses":[{"kind":"text","phase":"final_answer","text":"Saved."}],"tool_calls":[],"memory_proposals":[{"#,
        &mut |event| events.push(event),
    );

    assert!(events
        .iter()
        .any(|event| matches!(event, GenerateStreamEvent::MemoryProposalsStarted)));
}
```

- [ ] **Step 2: Run the failing streaming tests**

Run:

```bash
cargo test -p noema-core provider::adapters::noema_response_stream::tests
```

Expected: fail because the streaming parser still looks for root `output`.

- [ ] **Step 3: Rename parser roles from output items to response and tool arrays**

In `JsonArrayRole`, replace `Output { next_index: usize }` with:

```rust
Responses { next_index: usize },
ToolCalls { next_index: usize },
MemoryProposals,
Nested,
```

In `JsonObjectRole`, replace `OutputItem(OutputItemState)` with:

```rust
ResponseItem(ResponseItemState),
ToolCall(ToolCallState),
Nested,
Root,
```

- [ ] **Step 4: Route root arrays by key**

Replace `is_root_output_value` with:

```rust
fn root_array_key(&self) -> Option<&str> {
    match self {
        Self::Object(JsonObjectContext {
            role: JsonObjectRole::Root,
            pending_key: Some(key),
            ..
        }) => Some(key.as_str()),
        Self::Object(_) | Self::Array(_) => None,
    }
}
```

In `push_array`, use:

```rust
let role = match self.stack.last().and_then(JsonContext::root_array_key) {
    Some("responses") => JsonArrayRole::Responses { next_index: 0 },
    Some("tool_calls") => JsonArrayRole::ToolCalls { next_index: 0 },
    Some("memory_proposals") => JsonArrayRole::MemoryProposals,
    _ => JsonArrayRole::Nested,
};
```

- [ ] **Step 5: Create response item and tool call states**

Replace `OutputItemState` with:

```rust
#[derive(Debug, Default)]
pub(crate) struct ResponseItemState {
    kind: Option<String>,
    buffered_text: String,
}

impl ResponseItemState {
    fn is_text(&self) -> bool {
        self.kind.as_deref() == Some("text")
    }
}

#[derive(Debug, Default)]
pub(crate) struct ToolCallState {
    output_index: usize,
    name: Option<String>,
    tool_call_started_emitted: bool,
}

impl ToolCallState {
    fn tool_call_started_event(&mut self) -> Option<GenerateStreamEvent> {
        if self.tool_call_started_emitted {
            return None;
        }
        let name = self.name.as_deref()?.trim();
        if name.is_empty() {
            return None;
        }
        self.tool_call_started_emitted = true;
        Some(GenerateStreamEvent::ToolCallStarted {
            output_index: self.output_index,
            name: name.to_string(),
        })
    }
}
```

- [ ] **Step 6: Update string target detection**

In `next_string_target`, detect:

```rust
match (&context.role, key) {
    (JsonObjectRole::ResponseItem(_), "text") => JsonStringTarget::ItemText,
    (JsonObjectRole::ResponseItem(_), "kind") => JsonStringTarget::ItemKind,
    (JsonObjectRole::ToolCall(_), "name") => JsonStringTarget::ItemName,
    _ => JsonStringTarget::Value,
}
```

Update `finish_string` so `ItemKind` updates `ResponseItemState`, `ItemName` updates `ToolCallState`, and `ItemText` buffers text until kind is known.

- [ ] **Step 7: Run stream parser tests**

Run:

```bash
cargo test -p noema-core provider::adapters::noema_response_stream::tests
```

Expected: all stream parser tests pass after updating old fixtures from `output` to `responses`/`tool_calls`.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/provider/adapters/noema_response_stream.rs crates/noema-core/src/provider/adapters/sse.rs crates/noema-core/src/provider/adapters/codex_responses.rs
git commit -m "fix: stream structured response object fields"
```

---

### Task 5: Prompt The Object Contract

**Files:**
- Modify: `crates/noema-core/src/daemon/prompts.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write prompt contract coverage**

Update `runtime_provider_prompt_includes_assistant_text_phase_contract` or add a sibling test:

```rust
#[tokio::test]
async fn runtime_provider_prompt_includes_response_object_contract() {
    let handle = test_runtime_handle(fake_provider(FakeCodexScenario::PromptObjectContract)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(&handle, conversation_id, "Search a few things.".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw object contract"
    )));
}
```

In the fake provider, add:

```rust
FakeCodexScenario::PromptObjectContract => {
    let saw_object_contract = instructions.contains(r#""responses":[]"#)
        && instructions.contains(r#""tool_calls":[]"#)
        && instructions.contains(r#""response_status":"needs_tools""#)
        && instructions.contains("Use responses: [] when there is nothing useful to say before tool execution.")
        && instructions.contains("You may emit multiple independent tool calls in tool_calls.");
    assistant_with_no_memories(if saw_object_contract {
        "saw object contract"
    } else {
        "missing object contract"
    })
}
```

- [ ] **Step 2: Run the failing prompt test**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_provider_prompt_includes_response_object_contract
```

Expected: fail because the prompt still describes `output`.

- [ ] **Step 3: Replace the structured response prompt shape**

In `build_structured_turn_system_prompt`, replace the current JSON shape block with:

```rust
Return exactly this top-level shape:
{{
  "response_status": "final",
  "responses": [
    {{"kind":"text","phase":"final_answer","text":"assistant reply to show the user"}}
  ],
  "tool_calls": [],
  "memory_proposals": []
}}
```

Replace tool call item examples with:

```rust
Use this tool_calls item shape:
{{"id":"call_memory_1","name":"search_memory","payload":{{"scope_ids":["human:local"],"query":"","purpose":"answer_human_question","limit":8}}}}
```

- [ ] **Step 4: Add explicit silent tool-call rules**

Replace the assistant text phase rules with:

```rust
Response and tool-call rules:
- Put user-visible output items in responses. Text items use {{"kind":"text","phase":"commentary","text":"I will check that."}} or {{"kind":"text","phase":"final_answer","text":"Here is the answer."}}.
- Use responses: [] when there is nothing useful to say before tool execution.
- Use response_status "needs_tools" whenever tool_calls is non-empty.
- Use response_status "final" only for the terminal answer, and then tool_calls must be empty.
- Use phase "commentary" only for text that helps the user understand why tools are about to run.
- Use phase "final_answer" only for terminal answer text in a final response.
- You may emit multiple independent tool calls in tool_calls when they do not depend on each other's results.
- If a later tool call depends on an earlier tool result, emit only the ready tool call now; after Noema sends a NOEMA_LOCAL_TOOL_RESULT message, continue with the next tool call or final answer.
Example silent tool response: {{"response_status":"needs_tools","responses":[],"tool_calls":[{{"id":"call_1","name":"mcp.web.search","payload":{{"query":"example"}}}}],"memory_proposals":[]}}
```

- [ ] **Step 5: Replace the memory proposal rules**

Replace:

```text
- Always include exactly one assistant_text item.
- Include exactly one memory_proposals item. Use an empty proposals array when there are no durable memories.
```

with:

```text
- Always include the top-level memory_proposals array. Use an empty array when there are no durable memories.
- Do not include a text response just to acknowledge routine tool use.
- Final responses must include at least one useful response item.
```

- [ ] **Step 6: Update onboarding prompt**

In `build_initial_name_onboarding_system_prompt`, use:

```rust
Return exactly this top-level shape:
{{
  "response_status": "final",
  "responses": [
    {{"kind":"text","phase":"final_answer","text":"a warm, concise onboarding message ending with a naming question"}}
  ],
  "tool_calls": [],
  "memory_proposals": []
}}
```

- [ ] **Step 7: Run prompt tests**

Run:

```bash
cargo test -p noema-core daemon::prompts::tests
cargo test -p noema-core daemon::tests::runtime_provider_prompt_includes_response_object_contract
cargo test -p noema-core daemon::tests::runtime_prompt_includes_unnamed_agent_onboarding
```

Expected: prompt tests pass after old string assertions are updated from `assistant_text` and `output` to `responses` and `tool_calls`.

- [ ] **Step 8: Commit**

```bash
git add crates/noema-core/src/daemon/prompts.rs crates/noema-core/src/daemon/tests.rs
git commit -m "prompt: use response object contract"
```

---

### Task 6: Refactor Runtime To Consume Structured Fields

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/tool_lifecycle.rs`
- Modify: `crates/noema-core/src/daemon/runtime/local_tools.rs`
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/runtime/context_compaction.rs`

- [ ] **Step 1: Update local tool call extraction tests**

In `tool_lifecycle.rs`, replace the existing output-index test input with:

```rust
let tool_calls = vec![
    GenerateToolCall {
        id: Some("call_1".to_string()),
        name: "search_memory".to_string(),
        payload: json!({
            "scope_ids": ["human:local"],
            "query": "",
            "purpose": "answer_human_question"
        }),
    },
    GenerateToolCall {
        id: Some("call_2".to_string()),
        name: "mcp.web.search".to_string(),
        payload: json!({"query":"second"}),
    },
];

let calls = local_tool_calls(&tool_calls);

assert_eq!(calls.len(), 2);
assert_eq!(calls[0].output_index, 0);
assert_eq!(calls[1].output_index, 1);
assert_eq!(calls[0].call_id.as_deref(), Some("call_1"));
assert_eq!(calls[1].name, "mcp.web.search");
```

- [ ] **Step 2: Refactor `LocalToolCall` source type**

In `tool_lifecycle.rs`, change imports and extraction:

```rust
use crate::provider::{GenerateActionItem, GenerateToolCall};
use serde_json::Value;

pub(super) fn local_tool_calls(tool_calls: &[GenerateToolCall]) -> Vec<LocalToolCall> {
    tool_calls
        .iter()
        .enumerate()
        .map(|(output_index, call)| LocalToolCall {
            output_index,
            call_id: call.id.clone(),
            name: call.name.clone(),
            payload: call.payload.clone(),
        })
        .collect()
}

pub(super) fn tool_call_action_item(call: &LocalToolCall) -> GenerateActionItem {
    GenerateActionItem::ToolCall {
        id: call.call_id.clone(),
        name: call.name.clone(),
        payload: call.payload.clone(),
    }
}
```

- [ ] **Step 3: Update local tool result items**

In `local_tools.rs`, change `local_tool_result_output_item` to:

```rust
pub(super) fn local_tool_result_action_item(result: &LocalToolResult) -> GenerateActionItem {
    GenerateActionItem::ToolResult {
        call_id: result.call_id().cloned(),
        name: Some(result.name().to_string()),
        success: Some(result.success()),
        payload: result.payload().clone(),
    }
}
```

Rename all call sites from `local_tool_result_output_item` to `local_tool_result_action_item`.

- [ ] **Step 4: Persist response items instead of output items**

In `transcript_persistence.rs`, rename `persist_provider_response_output_item` to `persist_provider_response_item` with this signature:

```rust
pub(super) async fn persist_provider_response_item(
    &mut self,
    turn: &ProviderActionTurn,
    index: usize,
    item: GenerateResponseItem,
    provider_phase_has_tools: bool,
    assistant_response: &mut ProviderAssistantResponse,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) -> Result<(), DaemonError>
```

The text branch becomes:

```rust
GenerateResponseItem::Text { phase, text } => {
    assistant_response.push_text(&text);
    let effective_phase = AssistantTextPhase::effective_for_response_item(
        &GenerateResponseItem::Text {
            phase,
            text: text.clone(),
        },
        provider_phase_has_tools,
    );
    let metadata = json!({
        "turn_index": turn.turn_index,
        "response_index": index,
        "stream_id": turn.stream_id,
        "phase": effective_phase.as_str(),
    });
    Persist the same `ConversationItemKind::AssistantText` record and send the
    same `TurnTranscriptItem::AssistantText` event that the current
    `GenerateOutputItem::AssistantText` branch persists.
}
```

The structured branch becomes:

```rust
GenerateResponseItem::Structured { schema, payload } => {
    let card_id = format!(
        "provider_structured:{}:{}:{index}",
        turn.conversation_id, turn.turn_index
    );
    Persist the same `ConversationItemKind::A2uiCard` record and send the same
    `TurnTranscriptItem::A2uiCard` event that the current
    `GenerateOutputItem::Structured` branch persists.
}
```

- [ ] **Step 5: Persist action items instead of output items**

Rename `persist_provider_action_output_item` to `persist_provider_action_item` and change its argument from `GenerateOutputItem` to `GenerateActionItem`. The match should contain exactly these variants and should move the existing branch bodies without behavioral changes: `GenerateActionItem::ToolCall`, `GenerateActionItem::ToolResult`, `GenerateActionItem::ApprovalRequest`, and `GenerateActionItem::ApprovalResult`. Do not leave no-op branches for response items; response items are handled only by `persist_provider_response_item`.

Update `persist_provider_tool_call_started` to call `tool_call_action_item`.

- [ ] **Step 6: Refactor initial turn persistence**

In `turn.rs`, replace the initial response loop:

```rust
let initial_memory_proposals = turn.response.memory_proposals.clone();
let initial_response_count = turn.response.responses.len();
let initial_tool_calls = local_tool_calls(&turn.response.tool_calls);
let initial_phase_has_tools = !initial_tool_calls.is_empty();
for (index, response_item) in turn.response.responses.iter().cloned().enumerate() {
    self.persist_provider_response_item(
        &action_turn,
        index,
        response_item,
        initial_phase_has_tools,
        &mut initial_assistant_response,
        item_tx,
    )
    .await?;
}
let mut next_output_index = initial_response_count + initial_tool_calls.len();
```

Keep tool execution after response persistence:

```rust
for call in &initial_tool_calls {
    self.persist_provider_tool_call_started(&local_action_turn, call.output_index, call, item_tx)
        .await?;
    let result = self
        .execute_local_tool(&turn, &turn.agent_identity, call)
        .await;
    self.persist_provider_action_item(
        &local_action_turn,
        next_output_index,
        local_tool_result_action_item(&result),
        item_tx,
    )
    .await?;
    next_output_index += 1;
    local_tool_results.push(result);
}
```

- [ ] **Step 7: Refactor continuation persistence**

In the continuation loop, replace `continuation_outputs_for_tools` with direct fields:

```rust
let continuation_tool_calls = if continuation_response.response_status == GenerateResponseStatus::NeedsTools {
    local_tool_calls(&continuation_response.tool_calls)
} else {
    Vec::new()
};
let continuation_phase_has_tools = !continuation_tool_calls.is_empty();
for (offset, response_item) in continuation_response.responses.iter().cloned().enumerate() {
    self.persist_provider_response_item(
        &continuation_action_turn,
        continuation_output_base + offset,
        response_item,
        continuation_phase_has_tools,
        &mut continuation_assistant_response,
        item_tx,
    )
    .await?;
}
let continuation_response_count = continuation_response.responses.len();
next_output_index += continuation_response_count + continuation_tool_calls.len();
```

Build `continuation_turn` using `continuation_response` fields, not a filtered old output list.

- [ ] **Step 8: Preserve disallowed continuation behavior for `update_own_name`**

Replace `is_disallowed_continuation_output` with:

```rust
fn is_disallowed_continuation_tool_call(call: &GenerateToolCall) -> bool {
    is_update_own_name_tool(&call.name)
}
```

Before extracting continuation tool calls, filter:

```rust
let continuation_tool_call_items = continuation_response
    .tool_calls
    .iter()
    .filter(|call| !is_disallowed_continuation_tool_call(call))
    .cloned()
    .collect::<Vec<_>>();
let continuation_tool_calls = local_tool_calls(&continuation_tool_call_items);
```

- [ ] **Step 9: Update context compaction**

In `context_compaction.rs`, replace `.output.iter()` assistant text extraction with:

```rust
response
    .responses
    .iter()
    .filter_map(|item| match item {
        GenerateResponseItem::Text { text, .. } => Some(text),
        GenerateResponseItem::Structured { .. } => None,
    })
```

- [ ] **Step 10: Run runtime tests**

Run:

```bash
cargo test -p noema-core daemon::runtime::tool_lifecycle::tests
cargo test -p noema-core daemon::tests::runtime_displays_commentary_before_tool_lifecycle_when_provider_orders_tool_first
cargo test -p noema-core daemon::tests::runtime_keeps_commentary_before_tool_lifecycle_when_provider_orders_text_first
cargo test -p noema-core daemon::tests::update_own_name_tool_does_not_start_repeated_continuation_tool_calls
cargo test -p noema-core daemon::tests::runtime_actor_continues_after_continuation_tool_call
```

Expected: runtime tests pass after fakes are updated in Task 7.

- [ ] **Step 11: Commit**

```bash
git add crates/noema-core/src/daemon/runtime crates/noema-core/src/daemon/tests.rs
git commit -m "refactor: consume structured response fields"
```

---

### Task 7: Add Silent Multi-Tool Regression Coverage

**Files:**
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Add a fake scenario**

Add this enum variant:

```rust
SilentMultipleToolCalls,
```

In the fake provider match:

```rust
FakeCodexScenario::SilentMultipleToolCalls => {
    if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
        assistant_with_no_memories("I checked both memory topics.")
    } else {
        Ok(GenerateResponse {
            responses: Vec::new(),
            tool_calls: vec![
                GenerateToolCall {
                    id: Some("call_1".to_string()),
                    name: "search_memory".to_string(),
                    payload: json!({
                        "arguments": {
                            "scope_ids": ["human:local"],
                            "query": "trains",
                            "purpose": "answer_human_question",
                            "limit": 8
                        }
                    }),
                },
                GenerateToolCall {
                    id: Some("call_2".to_string()),
                    name: "search_memory".to_string(),
                    payload: json!({
                        "arguments": {
                            "scope_ids": ["human:local"],
                            "query": "planes",
                            "purpose": "answer_human_question",
                            "limit": 8
                        }
                    }),
                },
            ],
            memory_proposals: Vec::new(),
            response_status: GenerateResponseStatus::NeedsTools,
            provider: "codex".to_string(),
            model,
            response_id: Some("fake-response".to_string()),
            usage: None,
        })
    }
}
```

- [ ] **Step 2: Add the regression test**

Add:

```rust
#[tokio::test]
async fn runtime_executes_silent_multiple_tool_calls_without_commentary() {
    let handle =
        test_runtime_handle(fake_provider(FakeCodexScenario::SilentMultipleToolCalls)).await;
    let conversation_id = handle
        .start_conversation(None)
        .await
        .expect("conversation")
        .conversation_id;

    let items = collect_turn(
        &handle,
        conversation_id,
        "Check memory for trains and planes.".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let initial_assistant_text_count = items
        .iter()
        .take_while(|item| !matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Started,
                ..
            } if activity_kind == "tool_call"
        ))
        .filter(|item| matches!(item, TurnTranscriptItem::AssistantText { .. }))
        .count();
    assert_eq!(
        initial_assistant_text_count, 0,
        "silent tool-only response should not synthesize commentary: {items:?}"
    );

    let started_tool_count = items
        .iter()
        .filter(|item| matches!(
            item,
            TurnTranscriptItem::Activity {
                activity_kind,
                status: TurnActivityStatus::Started,
                ..
            } if activity_kind == "tool_call"
        ))
        .count();
    assert_eq!(started_tool_count, 2, "{items:?}");

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "I checked both memory topics."
    )));
}
```

- [ ] **Step 3: Run the new test**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_executes_silent_multiple_tool_calls_without_commentary
```

Expected: pass after the runtime consumes `tool_calls[]` and does not require `responses[]` before tool execution.

- [ ] **Step 4: Commit**

```bash
git add crates/noema-core/src/daemon/tests.rs
git commit -m "test: cover silent multiple tool calls"
```

---

### Task 8: Update Memory And Metadata Callers

**Files:**
- Modify: `crates/noema-core/src/daemon/memory/writes.rs`
- Modify: `crates/noema-core/src/graphql/mcp.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`

- [ ] **Step 1: Keep metadata helpers using `assistant_text()`**

For callers that parse metadata-only model text, keep this pattern:

```rust
let parsed = parse_canonicalization_response(&response.assistant_text())?;
```

No structural memory-write parser changes are needed when callers only need final text.

- [ ] **Step 2: Update turn response metadata recording**

In `turn.rs`, replace response metadata that stores `"output": &response.output` with:

```rust
"responses": &response.responses,
"tool_calls": &response.tool_calls,
"memory_proposals": &response.memory_proposals,
"response_status": response.response_status,
```

- [ ] **Step 3: Run targeted non-chat tests**

Run:

```bash
cargo test -p noema-core daemon::memory::writes::tests
cargo test -p noema-core graphql::mcp::tests
cargo test -p noema-core daemon::tests::provider_memory_proposal_uses_matching_assistant_item_within_phase
```

Expected: tests pass with no changes to memory parsing semantics.

- [ ] **Step 4: Commit**

```bash
git add crates/noema-core/src/daemon/memory/writes.rs crates/noema-core/src/graphql/mcp.rs crates/noema-core/src/daemon/runtime/turn.rs
git commit -m "refactor: record structured response metadata"
```

---

### Task 9: Durable Context And Full Validation

**Files:**
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update durable context**

Append this bullet under the active harness/runtime direction in `docs/context/current.md`:

```markdown
- The provider-facing Noema response contract is object-shaped rather than
  ordered-output-shaped: `responses[]` carries user-visible outputs,
  `tool_calls[]` carries executable actions, `memory_proposals[]` carries
  candidate memory writes, and `response_status` declares whether the turn needs
  tools or is final. Empty `responses[]` is valid for routine intermediate
  tool-only responses, so the runtime does not force filler commentary before
  executing tools.
```

- [ ] **Step 2: Run whitespace and format checks**

Run:

```bash
git status --short --branch
git diff --check
cargo fmt --all --check
```

Expected:
- `git diff --check` prints no whitespace errors.
- `cargo fmt --all --check` exits 0.
- Only intended implementation files and pre-existing unrelated web files are unstaged.

- [ ] **Step 3: Run Rust validation**

Run:

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands exit 0.

- [ ] **Step 4: Run frontend transcript validation**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/renderModel.test.ts
bun run lint
bun run build
```

Expected:
- The transcript render-model test passes.
- Lint and build pass.
- A Vite large-chunk warning may appear during build and is not a failure.

- [ ] **Step 5: Ship checklist**

Run:

```bash
git status --short --branch
git diff --check
git diff --cached --stat
git diff --cached --name-status
```

Expected:
- No staged unrelated files.
- Any remaining unstaged web files are reported as unrelated pre-existing work.

- [ ] **Step 6: Commit**

```bash
git add docs/context/current.md
git commit -m "docs: record structured response object contract"
```

---

## Self-Review Notes

Spec coverage:
- The plan replaces `message` with `responses[]`, matching the requested ability to support text, future attachments, and more text.
- The plan allows `responses: []` for intermediate tool-only responses.
- The plan supports multiple independent tool calls in one response.
- The plan keeps runtime tool lifecycle rows owned by Noema rather than provider array order.

Placeholder scan:
- Every task names exact files, expected commands, and concrete code shapes.
- No task depends on an unspecified migration or hidden frontend redesign.

Type consistency:
- Provider-visible output items are `GenerateResponseItem`.
- Runtime action rows are `GenerateActionItem`.
- Provider tool requests are `GenerateToolCall`.
- Turn completion is declared by `GenerateResponseStatus`.
