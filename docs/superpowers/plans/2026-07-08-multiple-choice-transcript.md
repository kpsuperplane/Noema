# Multiple Choice Transcript Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add first-class multiple-choice agent responses and durable user selections to Noema chat.

**Architecture:** Extend the provider response contract with a `multiple_choice` response item, persist assistant prompts and human selections as distinct conversation item kinds, expose them through GraphQL transcript unions, and render/submit them from the web transcript. Keep A2UI separate and treat structured payloads as the authority for semantic choices.

**Tech Stack:** Rust, serde, async-graphql, SQLite-backed Noema store, Apollo GraphQL codegen, React, StyleX, Bun.

## Global Constraints

- Work on `main`.
- Preserve unrelated dirty worktree changes.
- Do not add migrations or compatibility layers for this pre-V1 schema change.
- Do not infer semantic user intent from English phrase matching.
- Do not run smoke tests or fixture tests unless explicitly requested.
- Do not write UI/frontend tests unless explicitly requested.
- Do not inspect frontend work with browser tools unless explicitly requested.
- Never modify `CARGO_BUILD_RUSTC_WRAPPER` or interfere with `sccache`.

---

### Task 1: Provider Contract And Agent Prompt Awareness

**Files:**
- Modify: `crates/noema-core/src/provider/contract.rs`
- Modify: `crates/noema-core/src/provider/adapters/responses.rs`
- Modify: `crates/noema-core/src/daemon/prompts.rs`

**Interfaces:**
- Produces: `GenerateResponseItem::MultipleChoice { phase, prompt, selection_mode, options }`
- Produces: `MultipleChoiceSelectionMode::{PickOne, PickMany}`
- Produces: `MultipleChoiceOption { id: String, label: String }`
- Consumers: transcript persistence in Task 2, prompt context in Task 3, GraphQL replay in Task 4.

- [ ] **Step 1: Write failing provider contract tests**

Add tests in `crates/noema-core/src/provider/contract.rs` that parse this JSON:

```json
{"response_status":"final","responses":[{"kind":"multiple_choice","phase":"final_answer","prompt":"Pick one","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]}],"tool_calls":[]}
```

Assert it returns one `GenerateResponseItem::MultipleChoice` with `PickOne`.
Add rejection tests for duplicate option ids, fewer than two options, and any
`multiple_choice` item in a `needs_tools` response.

- [ ] **Step 2: Verify provider contract tests fail**

Run:

```bash
cargo test -p noema-core provider::contract::tests::required_noema_response_accepts_multiple_choice_response --lib
```

Expected: FAIL because `multiple_choice` is not a known response item.

- [ ] **Step 3: Implement provider response types and validation**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultipleChoiceSelectionMode {
    PickOne,
    PickMany,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipleChoiceOption {
    pub id: String,
    pub label: String,
}
```

Add `GenerateResponseItem::MultipleChoice` and validate non-empty prompt,
`options.len() >= 2`, non-empty unique ids, non-empty labels, and no
multiple-choice items in `needs_tools` responses.

- [ ] **Step 4: Update OpenAI/Codex response JSON schema**

Add a third `oneOf` branch in `noema_response_text_format()` for:

```json
{"kind":"multiple_choice","phase":"final_answer","prompt":"...","selection_mode":"pick_one","options":[{"id":"a","label":"A"},{"id":"b","label":"B"}]}
```

Require `kind`, `phase`, `prompt`, `selection_mode`, and `options`.

- [ ] **Step 5: Make the agent aware in prompt instructions**

Update `build_structured_turn_system_prompt()` with a concise multiple-choice
section:

```text
Multiple-choice responses:
- Use a response item with kind "multiple_choice" when the user should choose from explicit options.
- Use selection_mode "pick_one" when one click should answer; use "pick_many" when the user may choose several and submit Done.
- Give each option a stable language-neutral id and a short label.
- Only include multiple_choice in response_status "final"; do not include it in needs_tools responses.
```

Add a prompt unit test asserting the prompt contains `kind "multiple_choice"`,
`selection_mode "pick_one"`, `selection_mode "pick_many"`, and the final-only
rule.

- [ ] **Step 6: Verify Task 1 passes**

Run:

```bash
cargo test -p noema-core provider::contract::tests::required_noema_response_accepts_multiple_choice_response --lib
cargo test -p noema-core provider::adapters::responses::tests::noema_response_text_format_includes_multiple_choice_response --lib
cargo test -p noema-core daemon::prompts::tests::structured_turn_prompt_explains_multiple_choice_response_items --lib
```

Expected: PASS.

- [ ] **Step 7: Commit Task 1**

```bash
git add crates/noema-core/src/provider/contract.rs crates/noema-core/src/provider/adapters/responses.rs crates/noema-core/src/daemon/prompts.rs
git commit -m "Add multiple-choice provider response contract"
```

### Task 2: Durable Prompt And Replay Items

**Files:**
- Modify: `crates/noema-core/src/conversation/status.rs`
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Modify: `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
- Modify: `crates/noema-core/src/daemon/web/replay.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Consumes: `GenerateResponseItem::MultipleChoice`
- Produces: `ConversationItemKind::MultipleChoicePrompt`
- Produces: `TurnTranscriptItem::MultipleChoicePrompt`

- [ ] **Step 1: Write failing persistence/replay tests**

Add a daemon test that returns a provider response with one
`GenerateResponseItem::MultipleChoice`, runs a turn, and asserts one durable
`ConversationItemKind::MultipleChoicePrompt` with payload `prompt`,
`selection_mode`, and `options`.

Add a replay test that maps a persisted prompt item into
`TurnTranscriptItem::MultipleChoicePrompt`.

- [ ] **Step 2: Verify Task 2 tests fail**

Run:

```bash
cargo test -p noema-core daemon::tests::turn_persists_multiple_choice_prompt --lib
```

Expected: FAIL because the new item kind and persistence branch do not exist.

- [ ] **Step 3: Add durable and live transcript prompt variants**

Add `MultipleChoicePrompt` to `ConversationItemKind`, parse it from
`multiple_choice_prompt`, and add `TurnTranscriptItem::MultipleChoicePrompt`
with `prompt`, `selection_mode`, and `options`.

- [ ] **Step 4: Persist provider multiple-choice prompts**

In `persist_provider_response_item`, map `GenerateResponseItem::MultipleChoice`
to `ConversationItemKind::MultipleChoicePrompt`, `content_text: Some(prompt)`,
and payload:

```json
{"prompt":"...","selection_mode":"pick_one","options":[{"id":"a","label":"A"}]}
```

Send the matching `TurnTranscriptItem::MultipleChoicePrompt` live event.

- [ ] **Step 5: Replay multiple-choice prompts**

In `daemon/web/replay.rs`, deserialize the prompt payload and return
`TurnTranscriptItem::MultipleChoicePrompt`.

- [ ] **Step 6: Verify Task 2 passes**

Run:

```bash
cargo test -p noema-core daemon::tests::turn_persists_multiple_choice_prompt --lib
cargo test -p noema-core daemon::web::tests::conversation_replay_maps_multiple_choice_prompt --lib
```

Expected: PASS.

- [ ] **Step 7: Commit Task 2**

```bash
git add crates/noema-core/src/conversation/status.rs crates/noema-core/src/daemon/protocol.rs crates/noema-core/src/daemon/runtime/transcript_persistence.rs crates/noema-core/src/daemon/web/replay.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Persist multiple-choice prompt transcript items"
```

### Task 3: User Selection Mutation And Model Context

**Files:**
- Modify: `crates/noema-core/src/conversation/status.rs`
- Modify: `crates/noema-core/src/daemon/protocol.rs`
- Modify: `crates/noema-core/src/daemon/runtime/turn.rs`
- Modify: `crates/noema-core/src/daemon/runtime/prompt_context.rs`
- Modify: `crates/noema-core/src/daemon/web/replay.rs`
- Modify: `crates/noema-core/src/graphql/chat.rs`
- Modify: `crates/noema-core/src/graphql/resolvers.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

**Interfaces:**
- Produces: `ConversationItemKind::MultipleChoiceSelection`
- Produces: `TurnTranscriptItem::MultipleChoiceSelection`
- Produces GraphQL mutation: `sendMultipleChoiceSelection(input: SendMultipleChoiceSelectionInput!): TurnAccepted!`

- [ ] **Step 1: Write failing selection validation tests**

Add Rust tests that call the GraphQL mutation against a seeded prompt and assert:
valid `pick_one` appends a selection item, unknown ids fail, duplicate selection
fails, `pick_one` with two ids fails, and `pick_many` with zero ids fails.

- [ ] **Step 2: Verify selection tests fail**

Run:

```bash
cargo test -p noema-core daemon::tests::multiple_choice_selection_pick_one_appends_user_item --lib
```

Expected: FAIL because no selection mutation exists.

- [ ] **Step 3: Add selection item kind and transcript variant**

Add `MultipleChoiceSelection` to `ConversationItemKind` as
`multiple_choice_selection`, and add `TurnTranscriptItem::MultipleChoiceSelection`
with `prompt_item_id`, `selection_mode`, and `selected_options`.

- [ ] **Step 4: Implement backend selection append and turn start**

Add a runtime method that validates the prompt item in the same conversation,
checks no visible selection already references the prompt item, validates ids
against prompt options and selection mode, appends the selection as a human item,
and starts the next agent turn using the structured selection as the trigger.

- [ ] **Step 5: Expose GraphQL types and mutation**

Add `MultipleChoiceOption`, `MultipleChoiceSelectionMode`,
`MultipleChoicePrompt`, `MultipleChoiceSelection`, and
`SendMultipleChoiceSelectionInput` to GraphQL. Extend `TranscriptItem` and
resolvers to map both prompt and selection variants.

- [ ] **Step 6: Include prompt and selection in model context**

Update `input_item_from_transcript_item` to include stable model-visible text
for prompt and selection items without parsing English labels as intent. Use
option ids as the semantic values.

- [ ] **Step 7: Verify Task 3 passes**

Run:

```bash
cargo test -p noema-core daemon::tests::multiple_choice_selection_pick_one_appends_user_item --lib
cargo test -p noema-core daemon::tests::multiple_choice_selection_rejects_invalid_option_id --lib
cargo test -p noema-core daemon::tests::prompt_context_includes_multiple_choice_selection_ids --lib
```

Expected: PASS.

- [ ] **Step 8: Commit Task 3**

```bash
git add crates/noema-core/src/conversation/status.rs crates/noema-core/src/daemon/protocol.rs crates/noema-core/src/daemon/runtime/turn.rs crates/noema-core/src/daemon/runtime/prompt_context.rs crates/noema-core/src/daemon/web/replay.rs crates/noema-core/src/graphql/chat.rs crates/noema-core/src/graphql/resolvers.rs crates/noema-core/src/graphql/schema.rs crates/noema-core/src/daemon/tests.rs
git commit -m "Add multiple-choice selection transcript flow"
```

### Task 4: Web Transcript Rendering And Submission

**Files:**
- Modify: `crates/noema-core/web/src/graphql/operations.ts`
- Modify: `crates/noema-core/web/src/shared/types.ts`
- Modify: `crates/noema-core/web/src/transcript/events.ts`
- Modify: `crates/noema-core/web/src/components/transcript/Transcript.tsx`
- Create: `crates/noema-core/web/src/components/transcript/MultipleChoicePrompt.tsx`
- Modify: `crates/noema-core/web/src/components/ChatSurface.tsx`

**Interfaces:**
- Consumes GraphQL union members `MultipleChoicePrompt` and `MultipleChoiceSelection`
- Consumes mutation `SendMultipleChoiceSelectionDocument`
- Produces pick-one immediate submit and pick-many toggle plus Done submit.

- [ ] **Step 1: Update GraphQL operations**

Add `MultipleChoicePrompt` and `MultipleChoiceSelection` fragments to
`ConversationItemFields` and `ConversationEventsDocument`. Add
`SendMultipleChoiceSelectionDocument`.

- [ ] **Step 2: Generate frontend GraphQL types**

Run:

```bash
cd crates/noema-core/web && bun run gen:types
```

Expected: generated types include `MultipleChoicePrompt`, `MultipleChoiceSelection`,
and `SendMultipleChoiceSelectionMutation`.

- [ ] **Step 3: Map transcript events**

Update `TurnTranscriptItem` and `TranscriptEntry` types, and map GraphQL prompt
and selection union members in `transcript/events.ts`.

- [ ] **Step 4: Render multiple-choice prompts and selections**

Create `MultipleChoicePrompt.tsx`. For `pick_one`, option buttons submit
immediately. For `pick_many`, options toggle selected state and a `Done` button
submits selected ids. Render user selections as a human-lane compact message
using the selected labels.

- [ ] **Step 5: Wire submission through chat surface**

Pass an `onSubmitMultipleChoiceSelection` callback from `ChatSurface` into
`Transcript`, call the mutation, and preserve existing optimistic/user turn
behavior where practical.

- [ ] **Step 6: Verify frontend build**

Run:

```bash
cd crates/noema-core/web && bun run build
```

Expected: PASS.

- [ ] **Step 7: Commit Task 4**

```bash
git add crates/noema-core/web/src/graphql/operations.ts crates/noema-core/web/src/generated crates/noema-core/web/src/shared/types.ts crates/noema-core/web/src/transcript/events.ts crates/noema-core/web/src/components/transcript/Transcript.tsx crates/noema-core/web/src/components/transcript/MultipleChoicePrompt.tsx crates/noema-core/web/src/components/ChatSurface.tsx
git commit -m "Render multiple-choice transcript controls"
```

### Task 5: Final Validation And Context Update

**Files:**
- Modify: `docs/context/current.md`

**Interfaces:**
- Consumes all prior tasks.
- Produces durable project context note.

- [ ] **Step 1: Run ship checklist**

Run:

```bash
git status --short --branch
git diff --check
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cd crates/noema-core/web && bun run build
```

Expected: all commands PASS.

- [ ] **Step 2: Update durable context**

Add one concise bullet to `docs/context/current.md` noting that multiple-choice
responses are first-class transcript items with durable user selections.

- [ ] **Step 3: Commit context update if validation passes**

```bash
git add docs/context/current.md
git commit -m "Update context for multiple-choice transcript support"
```
