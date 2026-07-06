# Tool Progress Audits Design

## Summary

Noema should support longer same-turn tool continuation chains without turning
runaway loops into raw provider protocol errors. The current runtime uses a
small fixed continuation limit, which can interrupt legitimate workflows that
need repeated search, fetch, MCP inspection, and write steps. Replace that
single hard stop with a larger bounded policy plus visible model-assisted
progress audits every 20 continuation steps.

The progress audit is not a substitute for deterministic runtime guardrails.
The runtime owns counters, repeated-call detection, failure thresholds, hard
ceilings, and transcript markers. A configurable auxiliary model judges semantic
progress from compact structured evidence and returns one of a small set of
strict decisions.

## Goals

- Allow long but legitimate tool chains to continue beyond the current fixed
  continuation limit.
- Surface visible progress checks in chat so users can tell Noema is monitoring
  long-running tool work.
- Use an auxiliary audit model that is configurable like the web fetch
  summarizer.
- Keep provider defaults accurate: `gpt-5.4-mini` is the default only for Codex
  and OpenAI provider accounts.
- Fail gracefully with a user-meaningful progress summary instead of a raw
  `tool continuation limit exceeded` provider protocol error.
- Preserve deterministic runtime safety even when the audit model is unavailable
  or returns invalid output.

## Non-Goals

- Do not add durable run envelopes or full resumable workflow checkpoints in
  this slice.
- Do not make the audit model the sole authority on whether a loop may continue.
- Do not expose every internal continuation step as verbose chat text.
- Do not introduce migrations or backwards compatibility layers for pre-V1
  schema changes.

## Current Behavior

`crates/noema-core/src/daemon/runtime/turn.rs` currently bounds provider tool
continuations with `MAX_PROVIDER_TOOL_CONTINUATIONS = 6`. Each continuation can
feed one or more local tool results back to the provider. Memory reads, web
search, web fetch, and calibrated MCP tool results require provider
continuation; `update_own_name` does not.

This keeps loops bounded, but it treats a legitimate multi-step workflow and a
pathological loop the same once the counter is exhausted. The user sees a
provider protocol error even when the agent may have been making progress.

## Proposed Runtime Policy

Replace the single small continuation limit with a progress policy:

```rust
struct ContinuationProgressPolicy {
    audit_interval: usize,          // 20
    max_continuations: usize,       // 80 initially
    repeated_call_threshold: usize,
    failure_streak_threshold: usize,
    stale_window_threshold: usize,
}
```

Initial defaults:

- `audit_interval = 20`
- `max_continuations = 80`
- repeated identical tool-call fingerprints stop or force an audit before the
  hard ceiling
- repeated failures stop or force an audit before the hard ceiling
- no new successful tool result across an audit window should bias toward
  `ask_human`, `checkpoint`, or graceful stop

The runtime still enforces the absolute ceiling. The ceiling is a final
invariant for safety, not the normal user experience for long work.

## Progress Digest

At each audit boundary, the runtime builds a compact
`ContinuationProgressDigest` from structured evidence. The digest should be
small enough to keep the audit model fast and predictable; the audit prompt
must not include full tool payloads, full result bodies, raw transcript text, or
the complete list of tool calls from the last 20 steps.

Recommended digest shape:

```rust
struct ContinuationProgressDigest {
    user_goal: String,                  // short truncated original request
    current_goal: Option<String>,       // latest rolling goal summary
    step: usize,
    window: ProgressWindowDigest,
    whole_turn: ProgressTurnDigest,
    recent_events: Vec<ProgressEvent>,  // capped at 3 to 5 brief summaries
}
```

The window and whole-turn digests should prefer counters over text:

- tool counts by name
- success and failure counts
- current failure streak
- repeated argument fingerprint count
- novel result count, such as new URLs, resource ids, or object ids
- successful side-effect count
- elapsed time bucket, not necessarily exact timing
- provider token usage bucket when available

`recent_events` should be short model-facing summaries produced by the runtime
from normalized tool metadata. Each event should be capped to a small character
budget, for example 160 to 240 characters. The whole digest should have a
target budget around 1,000 to 1,500 tokens, with deterministic truncation that
keeps counters, the current goal, and the newest high-signal events first.

When tool results are large, the audit receives only their existing normalized
summary, result kind, success flag, and novelty or side-effect signals. Sensitive
provider ids, raw tool diagnostics, and large payloads stay out of the prompt.

## Auxiliary Audit Model

Add a new auxiliary model preference task id:

```rust
pub const TOOL_PROGRESS_AUDIT_TASK_ID: &str = "tool_progress_audit";
```

Reuse the storage and GraphQL shape established by the web fetch summarizer
where practical:

- provider kind
- provider account id
- model profile
- available model options
- effective default model profile

Provider-specific defaults:

- Codex provider accounts default to `gpt-5.4-mini`.
- OpenAI provider accounts default to `gpt-5.4-mini`.
- Foundation Local must not default to `gpt-5.4-mini`; it should use a valid
  provider-native lightweight or default profile exposed by the account.
- If no usable audit model is available, the runtime should skip model-based
  audit and rely on deterministic guardrails.

The main chat model should not implicitly become the audit model just because it
is configured for the agent. This mirrors the web fetch summarizer principle:
auxiliary compression/classification work is separately configurable.

## Audit Prompt And Contract

The audit prompt should treat tool outputs as untrusted data. It should ask the
model to classify progress from the runtime digest, not to execute tools or
answer the user directly.

The model must return strict JSON:

```json
{
  "decision": "continue",
  "confidence": "high",
  "user_summary": "I found several relevant options and am checking whether they are already in Notion.",
  "reason": "The last window produced new search results and a Notion database lookup; the next write depends on those results.",
  "next_goal": "Create the restaurant entries in Notion with concise notes."
}
```

Allowed `decision` values:

- `continue`: the tool chain is making progress and may proceed.
- `finalize`: stop tool use and ask the main provider to produce a final answer
  from gathered context.
- `ask_human`: stop and ask the user a concrete question before continuing.
- `checkpoint`: pause with a resume-oriented summary because the work is too
  long or needs a fresh run boundary.

Allowed `confidence` values:

- `low`
- `medium`
- `high`

Invalid audit output is treated as audit execution failure. The runtime records
the parse failure in system diagnostics, makes one no-tools finalization
attempt, and falls back to the latest progress summary if finalization fails.

## Runtime Decisions

At each continuation boundary:

1. Execute the current batch of tool calls.
2. Append structured tool results to the continuation state.
3. Update deterministic counters and fingerprints.
4. Stop early if deterministic guardrails prove the loop is unhealthy.
5. If the step is an audit boundary, emit a visible progress-check marker.
6. Run the auxiliary audit model when available.
7. Apply the audit decision.
8. Continue, finalize, ask the human, or checkpoint.
9. If the hard ceiling is reached or the audit execution fails, make one
   no-tools finalization attempt so the agent can deliver a coherent final
   message to the user.
10. Stop gracefully if finalization fails, using the latest progress summary.

Decision behavior:

- `continue`: proceed with the normal provider continuation loop.
- `finalize`: perform a final provider continuation with instructions to answer
  using the gathered results and without calling more tools.
- `ask_human`: persist a visible assistant question and stop the turn.
- `checkpoint`: persist a visible pause summary. Until durable run checkpoints
  exist, this is a graceful stop with enough context for a follow-up user turn.

Hard-ceiling and audit-failure finalization must disable provider tool use. The
agent gets one opportunity to explain what it accomplished, what remains, and
whether the user should continue in a new turn. The runtime should not allow the
audit model or the finalization attempt to override hard safety stops.

## Transcript Markers

Long-running work should surface progress checks as compact normal transcript
markers, not raw logs.

When an audit starts:

- show `Checking progress`
- mark the item as in progress

When it completes:

- update the marker with one short user-facing summary
- expose a small decision label in the expanded marker state

Recommended labels:

- `Still making progress`
- `Ready to wrap up`
- `Needs your input`
- `Paused with checkpoint`
- `Progress check unavailable`

Default collapsed marker text should stay short. Expanded details may include
recent tool count, elapsed time, and the audit summary, but not provider call
ids or raw diagnostic payloads.

## Error Handling

The normal user-facing outcome for long work should not be
`tool continuation limit exceeded`.

Graceful outcomes:

- repeated identical calls: stop with a concise loop explanation
- repeated failures: stop with the failing tool category and next useful action
- audit unavailable before execution, such as no usable configured model:
  continue only while deterministic guardrails remain healthy
- audit execution failure: give the agent one no-tools finalization attempt,
  then stop with the latest progress summary if that attempt fails
- hard ceiling reached: give the agent one no-tools finalization attempt, then
  stop with the latest progress summary if that attempt fails
- provider audit failure details: log diagnostics and keep raw errors out of
  the default transcript

Raw provider and MCP diagnostics should continue to live in system diagnostics,
not default chat transcript text.

## Settings

Settings should expose the audit model under a new Safety settings page:
`Settings > Safety > Usage`, with route `/settings/safety/usage`. This keeps
runtime budget and self-monitoring controls with safety/governance behavior
rather than first-party Web tools. The page should use the same model picker
component as other auxiliary model controls where possible.

The UI should show:

- current saved audit model preference, if any
- effective provider-specific default
- available provider/profile options
- disabled reasons when a provider account or profile cannot be used

The Web settings page should continue to own `web.search`, `web.fetch`, and the
fetch summarizer model. It should not render the tool progress audit control.

The copy should not imply `gpt-5.4-mini` is globally valid. It is only the
Codex/OpenAI default.

## Testing

Unit and focused integration tests should cover:

- no audit before step 20
- audits trigger at steps 20, 40, 60, and so on
- hard ceiling still stops runaway loops
- repeated identical calls stop or force an audit before the hard ceiling
- repeated failures stop or force an audit before the hard ceiling
- Codex and OpenAI default to `gpt-5.4-mini`
- Foundation Local does not default to `gpt-5.4-mini`
- saved auxiliary audit model preference is used when available
- unavailable audit model falls back to deterministic guardrails
- progress digest stays within the configured size budget and excludes raw tool
  payloads
- invalid audit JSON is handled as audit execution failure
- `continue`, `finalize`, `ask_human`, and `checkpoint` map to the expected
  runtime behavior
- hard ceiling triggers exactly one no-tools finalization attempt before the
  graceful stop path
- audit execution failure triggers exactly one no-tools finalization attempt
  before the graceful stop path
- progress-check transcript markers are emitted and updated

## Open Implementation Notes

- The first implementation can keep checkpoint behavior as a graceful pause
  summary rather than a durable resumable run.
- The hard ceiling value of 80 is intentionally conservative enough to prevent
  unbounded loops while giving complex workflows room to complete.
- If later runtime run envelopes land, `checkpoint` should become a durable
  resume point rather than a final assistant pause message.
