# Cache Hit Debug Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a right-click `Debug` menu item on assistant messages that opens provider usage details, including cached input tokens and cache hit ratio.

**Architecture:** Persist provider-neutral usage metadata on assistant transcript items at response persistence time. Expose durable conversation item metadata through GraphQL replay, carry it through the transcript model, and use Astryx `ContextMenu` plus `Dialog` for an inspect-only debug surface.

**Tech Stack:** Rust 2024, async Rust, SurrealDB-backed `conversation_items`, async-graphql, React 19, TypeScript, Astryx `ContextMenu` and `Dialog`, StyleX, Bun frontend tests.

## Global Constraints

- Work on `main`.
- Preserve unrelated dirty worktree changes.
- Do not modify `CARGO_BUILD_RUSTC_WRAPPER`, unset sccache, or bypass the Rust compiler wrapper.
- Do not run smoke tests or fixture tests.
- Do not run browser inspection unless explicitly requested.
- Do not add a visible transcript activity row for provider usage.
- Do not expose raw provider payloads, encrypted reasoning content, prompts, transcript text, credentials, account identifiers, or request headers.
- Omit or disable `Debug` when provider usage is unavailable.
- Show `0%` only when the provider explicitly reports `cached_input_tokens: 0`.
- Store `cache_hit_ratio` only when `input_tokens > 0` and `cached_input_tokens` is reported.
- Use Astryx `ContextMenu` for the right-click menu.
- Name the menu item `Debug`.

---

## File Structure

- Modify `crates/noema-core/src/daemon/runtime/turn.rs`
  - Add provider response metadata fields to `ProviderActionTurn`.
  - Add `ProviderResponsePosition` so persistence can distinguish local response index from global output index.
  - Pass provider model, usage, phase, and response position at initial, continuation, and progress-finalization persistence call sites.
- Modify `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
  - Build `provider_usage` metadata for assistant text items.
  - Unit-test cache ratio construction and explicit zero behavior.
- Modify `crates/noema-core/src/daemon/web/replay.rs`
  - Preserve conversation item metadata in `ConversationReplayItem`.
- Modify `crates/noema-core/src/daemon/web/mod.rs`
  - Update replay tests for metadata preservation.
- Modify `crates/noema-core/src/graphql/chat.rs`
  - Expose `metadata` on durable `ConversationItem`, aligning replay with live `ConversationItemEvent`.
- Modify `crates/noema-core/src/graphql/schema.rs`
  - Add GraphQL replay coverage for assistant item metadata.
- Modify `crates/noema-core/web/src/graphql/operations.ts`
  - Request `metadata` in `ConversationItemFields`.
- Regenerate `crates/noema-core/web/src/generated/graphql.ts` and `crates/noema-core/web/src/generated/schema.graphql`.
- Modify `crates/noema-core/web/src/shared/types.ts`
  - Add `metadata?: unknown` to durable assistant transcript entries.
- Modify `crates/noema-core/web/src/transcript/events.ts`
  - Carry metadata from replay and live events onto assistant entries.
- Create `crates/noema-core/web/src/components/transcript/debugUsage.ts`
  - Parse and format provider usage metadata.
- Create `crates/noema-core/web/src/components/transcript/debugUsage.test.ts`
  - Unit-test valid, malformed, unavailable, explicit zero, and nonzero ratio cases.
- Create `crates/noema-core/web/src/components/transcript/ProviderUsageDebugDialog.tsx`
  - Render the compact debug details dialog.
- Modify `crates/noema-core/web/src/components/transcript/Message.tsx`
  - Wrap assistant messages with Astryx `ContextMenu` when debug usage exists and open the debug dialog.
- Modify `crates/noema-core/web/src/components/transcript/Transcript.tsx`
  - Parse assistant metadata and pass debug usage into `Message`.

---

### Task 1: Persist Provider Usage Metadata And Replay It Through GraphQL

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/daemon/web/replay.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/graphql/chat.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`

**Interfaces:**
- Consumes: existing `GenerateResponse.usage: Option<TokenUsage>`.
- Produces: assistant conversation item metadata containing optional `provider_usage`.
- Produces: GraphQL `ConversationItem.metadata: JSON!`.

- [ ] **Step 1: Write failing metadata helper tests**

In `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`, add this test module at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::TokenUsage;

    #[test]
    fn provider_usage_metadata_includes_cache_hit_ratio() {
        let metadata = provider_usage_metadata(
            "codex",
            "gpt-test",
            "initial",
            ProviderResponsePosition {
                response_index: 0,
                output_index: Some(0),
            },
            Some(&TokenUsage {
                input_tokens: 12000,
                output_tokens: 900,
                total_tokens: 12900,
                cached_input_tokens: Some(9600),
            }),
        );

        assert_eq!(metadata["provider_usage"]["provider"], "codex");
        assert_eq!(metadata["provider_usage"]["model"], "gpt-test");
        assert_eq!(metadata["provider_usage"]["phase"], "initial");
        assert_eq!(metadata["provider_usage"]["response_index"], 0);
        assert_eq!(metadata["provider_usage"]["output_index"], 0);
        assert_eq!(metadata["provider_usage"]["input_tokens"], 12000);
        assert_eq!(metadata["provider_usage"]["cached_input_tokens"], 9600);
        assert_eq!(metadata["provider_usage"]["cache_hit_ratio"], 0.8);
    }

    #[test]
    fn provider_usage_metadata_preserves_explicit_zero_cached_tokens() {
        let metadata = provider_usage_metadata(
            "codex",
            "gpt-test",
            "continuation",
            ProviderResponsePosition {
                response_index: 1,
                output_index: Some(12),
            },
            Some(&TokenUsage {
                input_tokens: 2048,
                output_tokens: 12,
                total_tokens: 2060,
                cached_input_tokens: Some(0),
            }),
        );

        assert_eq!(metadata["provider_usage"]["phase"], "continuation");
        assert_eq!(metadata["provider_usage"]["response_index"], 1);
        assert_eq!(metadata["provider_usage"]["output_index"], 12);
        assert_eq!(metadata["provider_usage"]["cached_input_tokens"], 0);
        assert_eq!(metadata["provider_usage"]["cache_hit_ratio"], 0.0);
    }

    #[test]
    fn provider_usage_metadata_omits_ratio_when_cached_tokens_are_unreported() {
        let metadata = provider_usage_metadata(
            "foundation_local",
            "foundation-local-default",
            "initial",
            ProviderResponsePosition {
                response_index: 0,
                output_index: None,
            },
            Some(&TokenUsage {
                input_tokens: 100,
                output_tokens: 5,
                total_tokens: 105,
                cached_input_tokens: None,
            }),
        );

        assert!(metadata["provider_usage"].get("cached_input_tokens").is_none());
        assert!(metadata["provider_usage"].get("cache_hit_ratio").is_none());
    }
}
```

- [ ] **Step 2: Run the failing helper tests**

Run:

```bash
cargo test -p noema-core provider_usage_metadata_ --no-fail-fast
```

Expected: FAIL because `provider_usage_metadata` and `ProviderResponsePosition` do not exist.

- [ ] **Step 3: Add response position and usage metadata helper**

In `crates/noema-core/src/daemon/runtime/turn.rs`, update the imports and structs:

```rust
use crate::provider::TokenUsage;

pub(in crate::daemon) struct ProviderActionTurn {
    pub(in crate::daemon) conversation_id: String,
    pub(in crate::daemon) turn_id: String,
    pub(in crate::daemon) turn_index: u64,
    pub(in crate::daemon) user_item_id: String,
    pub(in crate::daemon) provider: String,
    pub(in crate::daemon) model: String,
    pub(in crate::daemon) response_phase: &'static str,
    pub(in crate::daemon) usage: Option<TokenUsage>,
    pub(in crate::daemon) stream_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::daemon) struct ProviderResponsePosition {
    pub(in crate::daemon) response_index: usize,
    pub(in crate::daemon) output_index: Option<usize>,
}
```

In `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`, update the imports:

```rust
use super::{
    actor::CodexRuntimeActor,
    tool_lifecycle::{LocalToolCall, tool_call_action_item},
    turn::{
        ProviderActionOutput, ProviderActionTurn, ProviderAssistantResponse,
        ProviderResponsePosition,
    },
};
```

Add this helper above `impl CodexRuntimeActor`:

```rust
pub(in crate::daemon::runtime) fn provider_usage_metadata(
    provider: &str,
    model: &str,
    phase: &'static str,
    position: ProviderResponsePosition,
    usage: Option<&crate::provider::TokenUsage>,
) -> Value {
    let Some(usage) = usage else {
        return json!({});
    };

    let mut provider_usage = serde_json::Map::new();
    provider_usage.insert("provider".to_string(), json!(provider));
    provider_usage.insert("model".to_string(), json!(model));
    provider_usage.insert("phase".to_string(), json!(phase));
    provider_usage.insert("response_index".to_string(), json!(position.response_index));
    if let Some(output_index) = position.output_index {
        provider_usage.insert("output_index".to_string(), json!(output_index));
    }
    provider_usage.insert("input_tokens".to_string(), json!(usage.input_tokens));
    provider_usage.insert("output_tokens".to_string(), json!(usage.output_tokens));
    provider_usage.insert("total_tokens".to_string(), json!(usage.total_tokens));
    if let Some(cached_input_tokens) = usage.cached_input_tokens {
        provider_usage.insert(
            "cached_input_tokens".to_string(),
            json!(cached_input_tokens),
        );
        if usage.input_tokens > 0 {
            provider_usage.insert(
                "cache_hit_ratio".to_string(),
                json!(cached_input_tokens as f64 / usage.input_tokens as f64),
            );
        }
    }

    json!({ "provider_usage": Value::Object(provider_usage) })
}
```

- [ ] **Step 4: Wire provider usage metadata into assistant text persistence**

Change `persist_provider_response_item` in `transcript_persistence.rs` to accept a position:

```rust
pub(super) async fn persist_provider_response_item(
    &mut self,
    turn: &ProviderActionTurn,
    position: ProviderResponsePosition,
    item: GenerateResponseItem,
    provider_phase_has_tools: bool,
    assistant_response: &mut ProviderAssistantResponse,
    item_tx: &mpsc::UnboundedSender<TurnStreamEvent>,
) -> Result<(), DaemonError> {
```

Inside the text branch, replace `index` usages with `position.output_index.unwrap_or(position.response_index)` only for stream id and existing global metadata, and add `provider_usage`:

```rust
let output_index = position.output_index.unwrap_or(position.response_index);
let stream_id = turn
    .stream_id
    .as_deref()
    .map(|stream_id| assistant_response_stream_id(stream_id, output_index));
let mut metadata = json!({
    "turn_index": turn.turn_index,
    "response_index": position.response_index,
    "output_index": position.output_index,
    "stream_id": stream_id,
    "phase": effective_phase.as_str(),
});
merge_metadata(
    &mut metadata,
    provider_usage_metadata(
        &turn.provider,
        &turn.model,
        turn.response_phase,
        position,
        turn.usage.as_ref(),
    ),
);
```

Add this helper near `provider_usage_metadata`:

```rust
fn merge_metadata(base: &mut Value, extra: Value) {
    let Some(base) = base.as_object_mut() else {
        return;
    };
    let Value::Object(extra) = extra else {
        return;
    };
    for (key, value) in extra {
        base.insert(key, value);
    }
}
```

Update structured card persistence to use `output_index` for existing ids:

```rust
let output_index = position.output_index.unwrap_or(position.response_index);
let card_id = format!(
    "provider_structured:{}:{}:{output_index}",
    turn.conversation_id, turn.turn_index
);
let metadata = json!({
    "turn_index": turn.turn_index,
    "response_index": position.response_index,
    "output_index": position.output_index,
    "source": "provider_structured_output",
});
```

- [ ] **Step 5: Update all backend persistence call sites**

In `turn.rs`, update every `ProviderActionTurn` initializer.

Initial provider response:

```rust
let action_turn = ProviderActionTurn {
    conversation_id: turn.conversation_id.clone(),
    turn_id: turn.turn_id.clone(),
    turn_index: turn.turn_index,
    user_item_id: turn.user_item_id.clone(),
    provider: turn.response.provider.clone(),
    model: turn.response.model.clone(),
    response_phase: "initial",
    usage: turn.response.usage.clone(),
    stream_id: Some(turn.initial_stream_id.clone()),
};
```

Initial response loop:

```rust
self.persist_provider_response_item(
    &action_turn,
    ProviderResponsePosition {
        response_index: index,
        output_index: Some(index),
    },
    response_item,
    initial_phase_has_tools,
    &mut initial_assistant_response,
    item_tx,
)
.await?;
```

Continuation action turn:

```rust
let continuation_action_turn = ProviderActionTurn {
    conversation_id: turn.conversation_id.clone(),
    turn_id: turn.turn_id.clone(),
    turn_index: turn.turn_index,
    user_item_id: turn.user_item_id.clone(),
    provider: continuation_response.provider.clone(),
    model: continuation_response.model.clone(),
    response_phase: "continuation",
    usage: continuation_response.usage.clone(),
    stream_id: Some(continuation_stream_id.clone()),
};
```

Continuation response loop:

```rust
self.persist_provider_response_item(
    &continuation_action_turn,
    ProviderResponsePosition {
        response_index: offset,
        output_index: Some(continuation_output_base + offset),
    },
    response_item,
    continuation_phase_has_tools,
    &mut continuation_assistant_response,
    item_tx,
)
.await?;
```

Progress-finalization action turn:

```rust
let action_turn = ProviderActionTurn {
    conversation_id: turn.conversation_id.clone(),
    turn_id: turn.turn_id.clone(),
    turn_index: turn.turn_index,
    user_item_id: turn.user_item_id.clone(),
    provider: response.provider.clone(),
    model: response.model.clone(),
    response_phase: "continuation",
    usage: response.usage.clone(),
    stream_id: None,
};
```

Progress-finalization loop:

```rust
self.persist_provider_response_item(
    &action_turn,
    ProviderResponsePosition {
        response_index: offset,
        output_index: Some(index + offset),
    },
    response_item,
    false,
    &mut assistant_response,
    item_tx,
)
.await?;
```

For local-only `ProviderActionTurn` values used for tool/audit activity, set:

```rust
model: "noema_local".to_string(),
response_phase: "continuation",
usage: None,
```

- [ ] **Step 6: Preserve replay metadata and expose it in GraphQL**

In `daemon/web/replay.rs`, add metadata to `ConversationReplayItem`:

```rust
pub(crate) struct ConversationReplayItem {
    pub(crate) item_id: String,
    pub(crate) cursor: String,
    pub(crate) turn_id: Option<String>,
    pub(crate) metadata: Value,
    pub(crate) item: TurnTranscriptItem,
}
```

Update `ConversationReplayItem::new`:

```rust
pub(crate) fn new(
    item_id: String,
    cursor: String,
    turn_id: Option<String>,
    metadata: Value,
    item: TurnTranscriptItem,
) -> Self {
    Self {
        item_id,
        cursor,
        turn_id,
        metadata,
        item,
    }
}
```

Update `web_conversation_item_from_record`:

```rust
Ok(Some(ConversationReplayItem::new(
    record.item_id,
    record.cursor,
    record.turn_id,
    record.metadata,
    item,
)))
```

In `graphql/chat.rs`, add metadata to durable items:

```rust
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ConversationItem")]
pub struct GraphqlConversationItem {
    pub item_id: String,
    pub cursor: String,
    pub turn_id: Option<String>,
    pub metadata: Json<Value>,
    pub item: GraphqlTranscriptItem,
}
```

Update the conversion:

```rust
Self {
    item_id: item.item_id,
    cursor: item.cursor,
    turn_id: item.turn_id,
    metadata: Json(item.metadata),
    item: item.item.into(),
}
```

- [ ] **Step 7: Add replay and GraphQL tests**

In `daemon/web/mod.rs`, update `conversation_replay_item_converts_assistant_text_record` so the test record includes metadata:

```rust
metadata: json!({
    "provider_usage": {
        "provider": "codex",
        "model": "gpt-test",
        "phase": "initial",
        "response_index": 0,
        "input_tokens": 100,
        "cached_input_tokens": 50,
        "cache_hit_ratio": 0.5,
        "output_tokens": 10,
        "total_tokens": 110
    }
}),
```

Add this assertion:

```rust
assert_eq!(item.metadata["provider_usage"]["cache_hit_ratio"], 0.5);
```

In `graphql/schema.rs`, add a test near the existing conversation transcript tests:

```rust
#[tokio::test]
async fn conversation_transcript_page_returns_item_metadata() {
    use crate::store::tests::test_store;

    let store = test_store().await;
    let conversation = store
        .create_conversation(crate::NewConversation {
            title: Some("Metadata test".to_string()),
            ..crate::NewConversation::chat_defaults()
        })
        .await
        .expect("conversation");
    store
        .append_conversation_item(crate::NewConversationItem {
            conversation_id: conversation.conversation_id.clone(),
            turn_id: Some("turn:metadata".to_string()),
            parent_item_id: None,
            kind: crate::ConversationItemKind::AssistantText,
            status: crate::ConversationItemStatus::Completed,
            author: crate::ActorRef::agent("agent:primary"),
            content_text: Some("hello".to_string()),
            payload_json: serde_json::json!({}),
            metadata: serde_json::json!({
                "provider_usage": {
                    "provider": "codex",
                    "model": "gpt-test",
                    "phase": "initial",
                    "response_index": 0,
                    "input_tokens": 100,
                    "cached_input_tokens": 25,
                    "cache_hit_ratio": 0.25,
                    "output_tokens": 5,
                    "total_tokens": 105
                }
            }),
        })
        .await
        .expect("item");

    let schema = build_schema(GraphqlState::for_tests_with_store(store));
    let response = schema
        .execute(format!(
            r#"
            query {{
              conversationTranscriptPage(input: {{ conversationId: "{}", limit: 10 }}) {{
                items {{
                  metadata
                  item {{ __typename ... on AssistantText {{ text }} }}
                }}
              }}
            }}
            "#,
            conversation.conversation_id
        ))
        .await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("json");
    let item = &data["conversationTranscriptPage"]["items"][0];
    assert_eq!(item["metadata"]["provider_usage"]["cached_input_tokens"], 25);
    assert_eq!(item["metadata"]["provider_usage"]["cache_hit_ratio"], 0.25);
}
```

- [ ] **Step 8: Run focused backend tests**

Run:

```bash
cargo test -p noema-core provider_usage_metadata_ --no-fail-fast
cargo test -p noema-core conversation_replay_item_converts_assistant_text_record --no-fail-fast
cargo test -p noema-core conversation_transcript_page_returns_item_metadata --no-fail-fast
```

Expected: all focused backend tests PASS.

- [ ] **Step 9: Commit backend metadata contract**

Run:

```bash
git add crates/noema-core/src/daemon/runtime/turn.rs \
  crates/noema-core/src/daemon/runtime/transcript_persistence.rs \
  crates/noema-core/src/daemon/web/replay.rs \
  crates/noema-core/src/daemon/web/mod.rs \
  crates/noema-core/src/graphql/chat.rs \
  crates/noema-core/src/graphql/schema.rs
git commit -m "feat: persist provider usage debug metadata"
```

Expected: commit contains only backend metadata and GraphQL replay changes.

---

### Task 2: Carry Usage Metadata Through Frontend Data Models

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/web/src/generated/graphql.ts`
- Modify: `crates/noema-core/web/src/generated/schema.graphql`
- Modify: `crates/noema-core/web/src/shared/types.ts`
- Modify: `crates/noema-core/web/src/transcript/events.ts`
- Create: `crates/noema-core/web/src/components/transcript/debugUsage.ts`
- Create: `crates/noema-core/web/src/components/transcript/debugUsage.test.ts`

**Interfaces:**
- Consumes: GraphQL `ConversationItem.metadata`.
- Produces: assistant `TranscriptEntry.metadata?: unknown`.
- Produces: `parseProviderUsageDebug(metadata: unknown): ProviderUsageDebug | null`.
- Produces: `providerUsageDebugRows(debug: ProviderUsageDebug): ProviderUsageDebugRow[]`.

- [ ] **Step 1: Request replay metadata and regenerate GraphQL types**

In `crates/noema-core/web/src/graphql/operations.ts`, add `metadata` to `ConversationItemFields`:

```ts
fragment ConversationItemFields on ConversationItem {
  itemId
  cursor
  turnId
  metadata
  item {
    __typename
```

Run:

```bash
cd crates/noema-core/web
bun run gen:types
```

Expected: generated GraphQL files include `metadata` on `ConversationTranscriptPageQuery["conversationTranscriptPage"]["items"][number]`.

- [ ] **Step 2: Write failing frontend metadata parser tests**

Create `crates/noema-core/web/src/components/transcript/debugUsage.test.ts`:

```ts
import { describe, test } from "node:test";
import assert from "node:assert/strict";
import {
  formatCacheHitRatio,
  parseProviderUsageDebug,
  providerUsageDebugRows
} from "./debugUsage";

describe("parseProviderUsageDebug", () => {
  test("accepts valid provider usage metadata", () => {
    const parsed = parseProviderUsageDebug({
      provider_usage: {
        provider: "codex",
        model: "gpt-test",
        phase: "initial",
        response_index: 0,
        output_index: 0,
        input_tokens: 12000,
        cached_input_tokens: 9600,
        cache_hit_ratio: 0.8,
        output_tokens: 900,
        total_tokens: 12900
      }
    });

    assert.deepEqual(parsed, {
      provider: "codex",
      model: "gpt-test",
      phase: "initial",
      responseIndex: 0,
      outputIndex: 0,
      inputTokens: 12000,
      cachedInputTokens: 9600,
      cacheHitRatio: 0.8,
      outputTokens: 900,
      totalTokens: 12900
    });
  });

  test("preserves explicit zero cached tokens", () => {
    const parsed = parseProviderUsageDebug({
      provider_usage: {
        provider: "codex",
        model: "gpt-test",
        phase: "continuation",
        response_index: 1,
        input_tokens: 2048,
        cached_input_tokens: 0,
        cache_hit_ratio: 0,
        output_tokens: 12,
        total_tokens: 2060
      }
    });

    assert.equal(parsed?.cachedInputTokens, 0);
    assert.equal(parsed?.cacheHitRatio, 0);
    assert.equal(formatCacheHitRatio(parsed?.cacheHitRatio ?? null), "0%");
  });

  test("rejects malformed provider usage metadata", () => {
    assert.equal(parseProviderUsageDebug(null), null);
    assert.equal(parseProviderUsageDebug({ provider_usage: "bad" }), null);
    assert.equal(
      parseProviderUsageDebug({
        provider_usage: {
          provider: "codex",
          model: "gpt-test",
          phase: "initial",
          response_index: 0,
          input_tokens: "12000",
          output_tokens: 900,
          total_tokens: 12900
        }
      }),
      null
    );
  });

  test("formats rows without inventing cache hits", () => {
    const parsed = parseProviderUsageDebug({
      provider_usage: {
        provider: "codex",
        model: "gpt-test",
        phase: "initial",
        response_index: 0,
        input_tokens: 12000,
        output_tokens: 900,
        total_tokens: 12900
      }
    });

    assert.ok(parsed);
    const rows = providerUsageDebugRows(parsed);
    assert.equal(rows.some((row) => row.label === "Cache hit ratio"), false);
    assert.equal(formatCacheHitRatio(null), "Unavailable");
  });
});
```

- [ ] **Step 3: Run the failing frontend parser tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/debugUsage.test.ts
```

Expected: FAIL because `debugUsage.ts` does not exist.

- [ ] **Step 4: Implement debug usage parser and formatter**

Create `crates/noema-core/web/src/components/transcript/debugUsage.ts`:

```ts
export type ProviderUsageDebug = {
  provider: string;
  model: string;
  phase: "initial" | "continuation" | string;
  responseIndex: number;
  outputIndex?: number;
  inputTokens: number;
  cachedInputTokens?: number;
  cacheHitRatio?: number;
  outputTokens: number;
  totalTokens: number;
};

export type ProviderUsageDebugRow = {
  label: string;
  value: string;
};

export function parseProviderUsageDebug(metadata: unknown): ProviderUsageDebug | null {
  if (!isRecord(metadata)) return null;
  const usage = metadata.provider_usage;
  if (!isRecord(usage)) return null;

  const provider = stringValue(usage.provider);
  const model = stringValue(usage.model);
  const phase = stringValue(usage.phase);
  const responseIndex = numberValue(usage.response_index);
  const outputIndex = optionalNumberValue(usage.output_index);
  const inputTokens = numberValue(usage.input_tokens);
  const outputTokens = numberValue(usage.output_tokens);
  const totalTokens = numberValue(usage.total_tokens);
  const cachedInputTokens = optionalNumberValue(usage.cached_input_tokens);
  const cacheHitRatio = optionalNumberValue(usage.cache_hit_ratio);

  if (
    provider === null ||
    model === null ||
    phase === null ||
    responseIndex === null ||
    outputIndex === false ||
    inputTokens === null ||
    outputTokens === null ||
    totalTokens === null ||
    cachedInputTokens === false ||
    cacheHitRatio === false
  ) {
    return null;
  }

  return {
    provider,
    model,
    phase,
    responseIndex,
    ...(outputIndex === undefined ? {} : { outputIndex }),
    inputTokens,
    ...(cachedInputTokens === undefined ? {} : { cachedInputTokens }),
    ...(cacheHitRatio === undefined ? {} : { cacheHitRatio }),
    outputTokens,
    totalTokens
  };
}

export function providerUsageDebugRows(debug: ProviderUsageDebug): ProviderUsageDebugRow[] {
  const rows: ProviderUsageDebugRow[] = [
    { label: "Provider", value: debug.provider },
    { label: "Model", value: debug.model },
    { label: "Phase", value: debug.phase },
    { label: "Response index", value: String(debug.responseIndex) }
  ];
  if (debug.outputIndex !== undefined) {
    rows.push({ label: "Output index", value: String(debug.outputIndex) });
  }
  rows.push(
    { label: "Input tokens", value: formatInteger(debug.inputTokens) },
    { label: "Cached input tokens", value: debug.cachedInputTokens === undefined ? "Unavailable" : formatInteger(debug.cachedInputTokens) }
  );
  if (debug.cacheHitRatio !== undefined) {
    rows.push({ label: "Cache hit ratio", value: formatCacheHitRatio(debug.cacheHitRatio) });
  }
  rows.push(
    { label: "Output tokens", value: formatInteger(debug.outputTokens) },
    { label: "Total tokens", value: formatInteger(debug.totalTokens) }
  );
  return rows;
}

export function formatCacheHitRatio(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "Unavailable";
  }
  return `${Math.round(value * 100)}%`;
}

function formatInteger(value: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() !== "" ? value : null;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function optionalNumberValue(value: unknown): number | undefined | false {
  if (value === undefined || value === null) return undefined;
  return typeof value === "number" && Number.isFinite(value) ? value : false;
}
```

- [ ] **Step 5: Carry metadata through transcript entries**

In `crates/noema-core/web/src/shared/types.ts`, add metadata to assistant entries:

```ts
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "assistant";
      streamId?: string;
      responseIndex?: number;
      metadata?: unknown;
      text: string;
    }
```

In `crates/noema-core/web/src/transcript/events.ts`, update assistant replay mapping:

```ts
if (transcriptItem.kind === "assistant_text") {
  return {
    id: itemId,
    itemId,
    cursor,
    turnId,
    type: "assistant",
    streamId: streamIdFromMetadata(metadata),
    responseIndex: responseIndexFromMetadata(metadata),
    metadata,
    text: transcriptItem.text
  };
}
```

Confirm live `ConversationItemEvent` already passes `event.metadata` into `entryFromConversationItem`; if not, update it to do so:

```ts
return entryFromConversationItem(
  event.itemId,
  event.cursor,
  event.turnId ?? undefined,
  event.item,
  event.metadata
);
```

- [ ] **Step 6: Run frontend parser tests and type generation checks**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/debugUsage.test.ts
bun test src/components/transcript/renderModel.test.ts
```

Expected: tests PASS.

- [ ] **Step 7: Commit frontend data flow**

Run:

```bash
git add crates/noema-core/web/src/graphql/operations.ts \
  crates/noema-core/web/src/generated/graphql.ts \
  crates/noema-core/web/src/generated/schema.graphql \
  crates/noema-core/web/src/shared/types.ts \
  crates/noema-core/web/src/transcript/events.ts \
  crates/noema-core/web/src/components/transcript/debugUsage.ts \
  crates/noema-core/web/src/components/transcript/debugUsage.test.ts
git commit -m "feat: carry provider usage metadata to transcript"
```

Expected: commit contains GraphQL generated changes, transcript metadata plumbing, and parser tests.

---

### Task 3: Add Assistant Message Debug Context Menu

**Files:**
- Create: `crates/noema-core/web/src/components/transcript/ProviderUsageDebugDialog.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/Message.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/Transcript.tsx`

**Interfaces:**
- Consumes: `ProviderUsageDebug` and `providerUsageDebugRows` from `debugUsage.ts`.
- Produces: right-click assistant menu item named `Debug`.

- [ ] **Step 1: Create the provider usage debug dialog component**

Create `crates/noema-core/web/src/components/transcript/ProviderUsageDebugDialog.tsx`:

```tsx
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";
import { providerUsageDebugRows, type ProviderUsageDebug } from "./debugUsage";

const styles = stylex.create({
  body: {
    display: "grid",
    gap: 14,
    padding: 18
  },
  details: {
    display: "grid",
    gridTemplateColumns: "minmax(120px, max-content) minmax(0, 1fr)",
    columnGap: 16,
    rowGap: 9,
    margin: 0
  },
  label: {
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10,
    letterSpacing: "0.08em",
    textTransform: "uppercase",
    color: "var(--noema-text-faint)"
  },
  value: {
    margin: 0,
    fontSize: 13,
    color: "var(--noema-text-primary)",
    overflowWrap: "anywhere"
  }
});

export function ProviderUsageDebugDialog({
  debug,
  open,
  onOpenChange
}: {
  debug: ProviderUsageDebug | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="info"
      width={460}
      aria-label="Provider debug details"
    >
      <DialogHeader title="Debug" subtitle="Provider usage" onOpenChange={onOpenChange} />
      <div {...stylex.props(styles.body)}>
        {debug ? (
          <dl {...stylex.props(styles.details)}>
            {providerUsageDebugRows(debug).map((row) => (
              <div key={row.label}>
                <dt {...stylex.props(styles.label)}>{row.label}</dt>
                <dd {...stylex.props(styles.value)}>{row.value}</dd>
              </div>
            ))}
          </dl>
        ) : null}
      </div>
    </Dialog>
  );
}
```

- [ ] **Step 2: Wrap assistant messages with Astryx ContextMenu**

In `crates/noema-core/web/src/components/transcript/Message.tsx`, add imports:

```tsx
import * as React from "react";
import { ContextMenu } from "@astryxdesign/core/ContextMenu";
import type { ProviderUsageDebug } from "./debugUsage";
import { ProviderUsageDebugDialog } from "./ProviderUsageDebugDialog";
```

Update props:

```tsx
export function Message({
  animate,
  role,
  text,
  showAvatar,
  debugUsage = null
}: {
  animate: boolean;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
  debugUsage?: ProviderUsageDebug | null;
}) {
```

Replace the return body with:

```tsx
  const [debugOpen, setDebugOpen] = React.useState(false);
  const bubble = (
    <TranscriptChatBubble role={role} showAvatar={showAvatar}>
      <Markdown
        autolink="gfm"
        components={role === "user" ? userMarkdownComponents : undefined}
        contentWidth="100%"
        density="compact"
        headingLevelStart={3}
        isStreaming={animate}
        xstyle={markdownXStyle(styles.markdown, role === "user" && styles.userMarkdown)}
      >
        {text}
      </Markdown>
    </TranscriptChatBubble>
  );

  if (role !== "assistant" || !debugUsage) {
    return bubble;
  }

  return (
    <>
      <ContextMenu
        items={[
          {
            label: "Debug",
            onClick: () => setDebugOpen(true)
          }
        ]}
      >
        {bubble}
      </ContextMenu>
      <ProviderUsageDebugDialog
        debug={debugUsage}
        open={debugOpen}
        onOpenChange={setDebugOpen}
      />
    </>
  );
```

- [ ] **Step 3: Parse assistant metadata in Transcript**

In `crates/noema-core/web/src/components/transcript/Transcript.tsx`, add:

```tsx
import { parseProviderUsageDebug } from "./debugUsage";
```

Update assistant rendering:

```tsx
if (entry.type === "assistant") {
  return (
    <Message
      animate={animateText}
      role="assistant"
      text={entry.text}
      showAvatar={showAvatar}
      debugUsage={parseProviderUsageDebug(entry.metadata)}
    />
  );
}
```

Leave `assistant_stream` without debug usage because provider usage is only durable after response completion:

```tsx
if (entry.type === "assistant_stream") {
  return <Message animate={animateText} role="assistant" text={entry.text} showAvatar={showAvatar} />;
}
```

- [ ] **Step 4: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/debugUsage.test.ts
bun test src/components/transcript/renderModel.test.ts
bun run lint
```

Expected: tests and lint PASS.

- [ ] **Step 5: Commit context menu UI**

Run:

```bash
git add crates/noema-core/web/src/components/transcript/ProviderUsageDebugDialog.tsx \
  crates/noema-core/web/src/components/transcript/Message.tsx \
  crates/noema-core/web/src/components/transcript/Transcript.tsx
git commit -m "feat: add assistant debug context menu"
```

Expected: commit contains only context menu and dialog UI changes.

---

### Task 4: Final Validation And Handoff

**Files:**
- Inspect: staged and unstaged git state.
- Modify: no files unless validation reveals a bug in changed files.

**Interfaces:**
- Consumes: commits from Tasks 1 through 3.
- Produces: final validated branch state.

- [ ] **Step 1: Run backend validation**

Run:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

Expected: all commands PASS.

- [ ] **Step 2: Run frontend validation**

Run:

```bash
cd crates/noema-core/web
bun test src/components/transcript/debugUsage.test.ts
bun test src/components/transcript/renderModel.test.ts
bun run lint
```

Expected: all commands PASS.

- [ ] **Step 3: Run ship checks**

Run:

```bash
git status --short --branch
git diff --check
git diff --stat
```

Expected: only unrelated pre-existing dirty files remain unless the implementation task intentionally changed them. The known unrelated files at plan-writing time are:

```text
crates/noema-core/src/web_fetch/direct_http.rs
crates/noema-core/web/src/components/shell/AppShell.tsx
```

- [ ] **Step 4: Commit validation fixes when needed**

If validation required code fixes, commit only those fixes:

```bash
git add <changed-files-from-this-plan>
git commit -m "fix: stabilize cache debug menu validation"
```

Expected: no unrelated dirty files are staged.

---

## Self-Review

- Spec coverage: the plan persists provider usage metadata, exposes durable GraphQL metadata, carries metadata into assistant entries, adds Astryx right-click `Debug`, opens a compact details dialog, handles unavailable and explicit-zero cache hits, avoids raw provider/reasoning/prompt exposure, and includes backend/frontend tests.
- Banned-marker scan: the plan has no incomplete marker text, deferred
  implementation steps, or unspecified test commands.
- Type consistency: backend metadata key is `provider_usage`; frontend parser consumes `provider_usage`; persisted fields use snake_case; frontend model converts to camelCase; menu label is exactly `Debug`.
