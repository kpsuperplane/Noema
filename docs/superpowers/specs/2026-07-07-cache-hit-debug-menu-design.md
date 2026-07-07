# Cache Hit Debug Menu Design

## Goal

Expose provider prompt-cache effectiveness for individual assistant responses
without adding visible transcript noise.

Users should be able to right-click an assistant message, choose `Debug`, and
inspect provider usage details including cached input tokens and cache hit
ratio. The feature is an owner/debug affordance, not a normal chat event.

## Current Context

Noema already parses Responses-compatible usage metadata into
`TokenUsage.cached_input_tokens`. Recent prompt-cache work made normal turns
more prefix-stable and sends `prompt_cache_key` for OpenAI and Codex
conversation requests.

The missing product path is durable per-message exposure:

- Provider usage is available on `GenerateResponse`.
- Timing logs already include token usage when enabled.
- Assistant transcript items do not carry provider usage metadata.
- GraphQL live `ConversationItemEvent` exposes item metadata, but durable
  `ConversationTranscriptPage` items currently do not.
- The React transcript only uses assistant item metadata for stream identity.

## User Experience

Assistant message bubbles get a right-click context menu using Astryx
`ContextMenu`.

When provider usage metadata is available, the menu contains one item:

```text
Debug
```

Selecting `Debug` opens a compact details surface for that assistant message.
The details surface shows:

- Provider.
- Model.
- Response phase: initial or continuation.
- Response index within that phase, when available.
- Input tokens.
- Cached input tokens.
- Cache hit ratio.
- Output tokens.
- Total tokens.

If provider usage is unavailable, `Debug` is omitted or disabled. The UI must
not show a synthetic `0%` cache hit unless the provider explicitly reported
`cached_input_tokens: 0`.

## Data Contract

Persist provider usage as structured metadata on assistant transcript items.
Use a small provider-neutral object:

```json
{
  "provider_usage": {
    "provider": "codex",
    "model": "gpt-5.4",
    "phase": "initial",
    "response_index": 0,
    "input_tokens": 12000,
    "cached_input_tokens": 9600,
    "cache_hit_ratio": 0.8,
    "output_tokens": 900,
    "total_tokens": 12900
  }
}
```

Rules:

- `cache_hit_ratio` is present only when `input_tokens > 0` and
  `cached_input_tokens` is reported.
- `cached_input_tokens: 0` is preserved as an explicit provider report.
- The ratio is stored as a numeric fraction from `0.0` to `1.0`.
- The object must not include raw provider payloads, encrypted reasoning
  content, prompts, transcript text, credentials, account identifiers, or
  request headers.
- If multiple assistant bubbles are produced from one provider response, each
  bubble may carry the same response-level usage metadata so the right-click
  target always works.

## Backend Design

Extend the assistant response persistence path so
`persist_provider_response_item` receives the response usage and annotates the
persisted assistant item metadata with `provider_usage`.

For initial provider responses, the metadata uses:

- `phase: "initial"`.
- The initial provider response's `provider`, `model`, and `usage`.
- The assistant response item index.

For continuation responses, the metadata uses:

- `phase: "continuation"`.
- The continuation provider response's `provider`, `model`, and `usage`.
- `response_index` as the continuation-local assistant response item index.
- `output_index` may also be included when the existing persistence path
  already computes a globally ordered output index for tool/assistant
  correlation.

Durable GraphQL replay should expose conversation item metadata alongside
`itemId`, `cursor`, `turnId`, and `item`. This aligns replay with live
`ConversationItemEvent` and lets refreshed transcripts still support `Debug`.

## Frontend Design

Carry `metadata` on assistant `TranscriptEntry` values for both live events and
durable replay.

Add a small parsing helper that extracts and validates `provider_usage` from
unknown metadata. It should return `null` unless required numeric fields are
well-formed. Formatting stays in a separate helper so tests can cover:

- Unavailable usage.
- Explicit `0%`.
- Nonzero ratios.
- Large token counts.

Wrap assistant chat bubbles with Astryx `ContextMenu`. The context menu should
not alter normal click, text selection, streaming, or transcript virtualization
behavior.

The `Debug` action opens a compact details surface. A modal/dialog is preferred
over placing all metrics directly in the context menu because token accounting
labels are too dense for a menu. The details surface can be component-local in
the transcript layer; it does not need a new route or Settings page.

## Error Handling

- Missing usage: omit or disable `Debug`.
- Missing cached-token detail: show token usage without a cache ratio, or omit
  cache-specific rows.
- `input_tokens == 0`: omit cache ratio to avoid division by zero.
- Malformed metadata: ignore it client-side and do not break transcript render.
- Provider dialect differences: rely on the existing provider-neutral
  `TokenUsage` fields rather than raw OpenAI/Codex response shapes.

## Testing

Rust tests:

- Provider response persistence includes `provider_usage` metadata when usage
  is returned.
- `cache_hit_ratio` is included for `input_tokens > 0` with reported cached
  tokens.
- `cached_input_tokens: 0` persists as an explicit zero.
- GraphQL `conversationTranscriptPage` returns durable item metadata.

Frontend tests:

- Metadata parser accepts valid provider usage and rejects malformed metadata.
- Formatting distinguishes unavailable ratio from explicit `0%`.
- Assistant transcript entries preserve metadata from both replay and live
  events.

Run the relevant unit validation. Do not run smoke tests or browser inspection
unless explicitly requested.

## Non-Goals

- No aggregate usage dashboard.
- No visible transcript activity row for every provider response.
- No provider raw payload viewer.
- No encrypted reasoning display.
- No prompt text display.
- No changes to cache key or prompt prefix behavior.
