# Stable Prompt Cache And Encrypted Reasoning Design

## Goal

Noema should maximize provider prompt-cache hits while preserving reasoning
continuity in stateless Responses API mode.

Outside of compaction, model/provider changes, and tool contract changes, a new
user turn in the same conversation should reuse the exact prompt prefix from the
previous turn. The only uncached portion should be newly appended transcript
content, especially the latest user message.

## Current Problem

Noema currently sends most long-lived prompt policy through `instructions`, but
that prompt also includes volatile fields:

- conversation id
- turn index
- project hint
- active retrieval ids
- compacted summary text
- rendered available tool rows

Because those fields appear before replayed transcript input, normal turns can
change the request prefix before the transcript reaches the model. That reduces
prompt-cache effectiveness even though `store: false` itself does not disable
prompt caching.

Noema also uses stateless Responses calls (`store: false`). OpenAI reasoning
models such as `gpt-5.5` default to medium reasoning effort, so reasoning items
can matter even when Noema does not explicitly set `reasoning.effort`. In
stateless mode, encrypted reasoning items must be requested and replayed if
Noema wants the provider to continue prior reasoning efficiently.

## Design

### Stable Instructions

`instructions` must be stable for a fixed provider, model profile, Noema prompt
contract version, and model-visible tool contract.

Stable instructions may include:

- Noema assistant identity and voice contract.
- Structured response JSON contract.
- Memory proposal schema and policy.
- Tool-use rules for the active tool exposure mode.
- Security and trust rules.

Stable instructions must not include:

- conversation id
- turn index
- current working directory or project hint
- active retrieval ids
- compacted summary text
- current user text
- prior transcript text
- timestamps or request ids

Tool contract changes may change instructions or native tool specs. That is an
acceptable cache reset because the model-visible capability surface changed.

### Transcript Replay

For an uncompacted conversation, the provider input should be append-only:

1. previous user messages
2. previous assistant messages
3. previous provider-native reasoning items, when available
4. previous provider-native function calls
5. previous function outputs
6. latest user message

The previous turn's full input should be a byte-for-byte prefix of the next
turn's input, except for provider serialization differences outside Noema's
control.

Noema should not add a general volatile context packet before transcript items.
If per-turn metadata is still needed, it must be minimized and placed as late as
possible. The preferred design is to remove it from model-visible context unless
it is required for the current answer.

### Compaction Boundary

Compaction is an intentional cache reset point.

When Noema compacts older transcript, it may replace those older items with a
summary item placed before the post-compaction transcript. After that summary is
created, subsequent turns should again become append-only from the new
checkpoint.

Compaction may also drop encrypted reasoning items covered by the compacted
range, because their corresponding detailed transcript history is no longer
being replayed.

### Encrypted Reasoning

For OpenAI/Codex Responses providers that support reasoning items, Noema should
request encrypted reasoning content by adding:

```json
"include": ["reasoning.encrypted_content"]
```

Noema should parse and persist provider `reasoning` output items with their
encrypted content and replay them unchanged in later provider input, in the same
relative order as returned by the provider.

Noema must not expose raw reasoning to users. It should store only opaque
encrypted content and minimal metadata required for provider replay.

If a provider does not support encrypted reasoning, Noema should continue with
the existing stateless transcript replay behavior rather than failing normal
chat.

### Provider Verification

OpenAI Responses and Codex Responses are similar but not identical APIs. Noema
must verify request and response behavior independently for each adapter instead
of assuming a field accepted by one provider is accepted by the other.

Implementation must separately validate:

- whether the provider accepts `include: ["reasoning.encrypted_content"]`
- the exact shape of returned reasoning output items
- the exact shape required to replay encrypted reasoning items as input
- whether `prompt_cache_key` and `prompt_cache_retention` are accepted
- how provider errors report unsupported request fields

Unsupported optional fields should be gated by provider capabilities rather than
sent optimistically in normal chat requests.

### Prompt Cache Controls

OpenAI Responses requests should send:

- `prompt_cache_retention: "24h"` when the provider capability advertises
  support.
- `prompt_cache_key` derived from the Noema conversation id, matching the Codex
  adapter behavior.

Codex Responses requests already send `prompt_cache_key` for conversation
requests. If Codex later advertises or accepts prompt cache retention, Noema can
enable it through the provider capability path.

## Testing

Add regression tests that verify:

- Two normal turns in one conversation produce identical `instructions`.
- The second turn replays the first turn's input as an exact prefix before
  appending new items.
- `turn_index`, conversation id, cwd/project hint, and compacted summary do not
  appear in stable instructions.
- Compaction changes the replay prefix only at the summary checkpoint.
- OpenAI requests include `prompt_cache_key` for conversation requests.
- OpenAI and Codex adapter tests independently cover accepted
  `reasoning.encrypted_content` request fields or provider-specific fallbacks.
- Provider `reasoning` items with encrypted content are parsed, persisted, and
  replayed in order.

## Rollout

1. Introduce provider-neutral history items for encrypted reasoning replay.
2. Move volatile prompt metadata out of `instructions`.
3. Add OpenAI `prompt_cache_key`.
4. Request encrypted reasoning content for reasoning-capable Responses
   providers.
5. Persist and replay encrypted reasoning items.
6. Keep compaction as the only normal cache reset for long conversations.
