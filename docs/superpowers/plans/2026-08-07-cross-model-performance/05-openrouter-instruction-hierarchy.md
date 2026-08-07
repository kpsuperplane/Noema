# OpenRouter Instruction Hierarchy Plan

**Status:** Approved for implementation after contract compatibility

**Goal:** Preserve Noema-authored context at the strongest supported
instruction level instead of disguising developer context as human messages.

**Outcome:** OpenRouter models receive one unambiguous instruction kernel and
one current trusted application-context block; durable human messages remain
purely human-authored.

## Boundaries

- Do not merge tool outputs, web content, email content, memory search results,
  or other external evidence into trusted instructions.
- Do not change the canonical transcript or historical human/assistant text.
- Do not infer trust from XML tags or message content.
- Keep direct OpenAI/Codex provider-native developer roles where supported.

## Provider-Neutral Request Shape

Add an explicit optional `application_context` field to `GenerateRequest`.
It contains only runtime-authored current context and is separate from:

- `instructions`: stable execution-role policy;
- `input`: human, assistant, tool-call, and tool-result history;
- native tool definitions: executable capability contracts.

The runtime constructs `application_context` from the latest
`ModelContextSnapshot` in stable section order: agent identity, tool visibility,
then volatile runtime environment. It sends a full snapshot on every fresh
request and continuation; historical `NOEMA_MODEL_CONTEXT_UPDATE` messages are
filtered from provider input once their latest state has been reconstructed.

## Provider Mapping

- OpenAI/Codex Responses: map `application_context` to one developer message
  after stable instructions and before conversation input.
- OpenRouter Chat Completions: append a clearly delimited
  `Noema application context` block to the leading system message. Never emit
  it as a user message.
- Local Chat Completions and Foundation Models: place it in their strongest
  system/instruction surface using the same ordering.
- Providers without an instruction surface are incompatible with contextual
  Noema execution; do not downgrade trusted context into human input.

Remove the OpenRouter-only `<noema_application_context>` wrapping rule and its
system sentence once every call site supplies the explicit field.

## Coalescing and Continuations

- Parse keyed runtime context updates only inside `noema-runtime`, where their
  type is authoritative. Provider adapters receive the already rendered latest
  snapshot and never parse Noema envelopes.
- Ordinary developer messages that are not keyed model-context updates remain
  explicit trusted application messages and are appended after the snapshot in
  original order.
- Context compaction summarizes conversation evidence only. It never summarizes
  or duplicates the current application-context block.
- Provider-chain fallback and catalog-expansion resets resend the same current
  application context with canonical messages.

## Prompt Cache and Token Use

- Keep stable role instructions first in the system/developer surface.
- Render agent identity and tool visibility before the per-turn clock so stable
  prefixes remain cacheable where the provider supports prefix caching.
- Do not repeat operation descriptions in application context after progressive
  disclosure removes them.
- Add debug token estimates for instructions, application context, native tool
  schemas, and conversation input without recording their content.

## Migration

- This is a request-construction change, not a persisted transcript migration.
- Existing durable context-update items remain readable and are coalesced into
  the new field at request time.
- New turns may stop persisting redundant full model-context updates only after
  replay, interaction resume, and export tests prove the latest snapshot is
  reconstructible. That deletion is outside this first slice.

## Evaluation Changes

- Update protocol fixtures to populate `application_context` directly.
- Add conflicting user text versus application-context authority, latest
  section replacement, tool-result injection, and provider-chain fallback
  cases.
- Re-run protocol context preservation/instruction priority and affected
  stateful Primary cases for Luna low/high, Sol medium, Sonnet 5, and Opus 5.
- Compare input tokens and cache reuse; do not run unaffected task-quality
  cases.

## Expected Touchpoints

- `crates/noema-providers/src/generation/request.rs`: add the provider-neutral
  application-context field.
- `crates/noema-providers/src/chat_completions/request.rs`,
  `adapters/openrouter.rs`, and `adapters/responses/input.rs`: map it to the
  strongest supported instruction surface.
- Foundation and local adapter lowering modules implement the same ordering;
  no provider adapter parses Noema context envelopes.
- `crates/noema-runtime/src/daemon/runtime/model_context.rs`,
  `turn/provider_request.rs`, and `context_compaction.rs`: coalesce the latest
  snapshot and exclude stale transport updates.
- `crates/noema-model-evals/src/hosted_provider.rs` and existing provider
  protocol tests own instruction-priority coverage.

## Tests and Acceptance

- OpenRouter wire requests contain no developer-as-user wrapper messages.
- Latest keyed context wins and stale updates are absent from provider input.
- Human messages cannot override runtime date, identity, tool authority, or
  role policy; external tool results remain untrusted evidence.
- Direct provider mappings retain their native developer role.
- Continuation, compaction, interaction resume, and fallback carry exactly one
  current application-context block.
- All representative models pass instruction-priority and context-preservation
  protocol cases without degrading exact final or streaming behavior.

**Budget:** 240–400 production lines, 160–240 test lines, 6–9 focused tests.

**Stop condition:** If the latest `ModelContextSnapshot` cannot be reconstructed
without historical provider messages, retain those items in persistence but
still coalesce them before transport. Do not create a second persisted context
authority.
