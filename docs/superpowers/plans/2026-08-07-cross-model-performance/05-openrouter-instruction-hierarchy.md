# OpenRouter Instruction Hierarchy Plan

**Status:** Independent adapter-local implementation slice

**Goal:** Stop representing Noema-authored developer context as human-authored
OpenRouter messages.

**Observable outcome:** OpenRouter wire requests preserve developer content in
the leading system instruction while actual human messages remain unchanged.

## Scope

- Change only OpenRouter request adaptation.
- Preserve trusted developer-message contents in original order by appending
  them to the leading system message under a fixed Noema-authored delimiter.
- Remove those developer items from the subsequent chat-message list so they
  are not duplicated or represented as user messages.
- Keep canonical transcripts and all direct-provider mappings unchanged.

## Non-Goals

- Do not add `application_context` or another provider-neutral field to
  `GenerateRequest`.
- Do not change OpenAI, Codex, Foundation Models, or local-provider lowering.
- Do not coalesce historical keyed model-context updates in the runtime during
  this slice.
- Do not change context persistence, compaction, export, prompt-cache ordering,
  or token-debug accounting.
- Do not infer trust from XML tags or arbitrary content.

## Mapping Contract

`GenerateRequest` already distinguishes developer messages from human, tool,
and assistant input. OpenRouter adaptation must preserve that typed provenance:

1. Start with the request's existing stable system instructions.
2. Collect typed developer-message contents in their canonical input order.
3. Append one clearly delimited `Noema developer context` section to the system
   content when at least one developer item exists.
4. Lower remaining human, assistant, tool-call, and tool-result items normally.
5. Never promote tool output, web content, email content, memory results, or
   other external evidence into the system section.

The delimiter is transport framing, not the source of authority. Authority
comes from the typed developer role before lowering.

Remove the current OpenRouter `<noema_application_context>` user wrapper and
the system sentence that attempts to make that user message authoritative.

## Continuations and Streaming

- Apply the same deterministic lowering to fresh and continuation requests.
- Preserve the current exact-final and streaming response paths; this is an
  input-only wire change.
- Provider fallback continues rebuilding from canonical typed input, so it
  receives the same ordered system content without a new persisted authority.
- Repeated historical developer context may still consume tokens. Measure that
  separately; runtime coalescing is justified only after this hierarchy fix is
  proven and token data shows it is material.

## Expected Touchpoints

- `crates/noema-providers/src/adapters/openrouter.rs`
- Existing OpenRouter request-lowering tests
- Affected OpenRouter protocol fixtures in `crates/noema-model-evals`

## Tests and Acceptance

1. An OpenRouter wire request places typed developer content after stable
   instructions in the system message and emits no developer-as-user wrapper.
2. Human messages and external tool results remain in their original untrusted
   roles and cannot enter the system section.
3. Multiple developer items preserve order without duplication across fresh
   and continuation request construction.

Run only instruction-priority/context-preservation protocol cases and affected
stateful Primary scenarios. Unrelated model-quality cases are not rerun.

**Budget:** 40–120 production lines, 40–100 test lines, 2–3 focused tests.

**Stop condition:** If typed developer messages are unavailable at the
OpenRouter adapter boundary, stop and identify the smallest existing request
boundary that retains them. Do not introduce a cross-provider context model
without evidence that more than OpenRouter needs it.
