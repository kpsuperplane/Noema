# Context Compaction Design

## Summary

Noema should respect each selected model's context window by compacting conversation context into durable, inspectable checkpoints instead of repeatedly trimming recent transcript text. The first slice should use rolling durable compaction checkpoints: one active summary per conversation and context profile, plus verbatim transcript items after that checkpoint.

This preserves stable prompt prefixes for model caching, supports daemon restart and resume, keeps original transcript items canonical, and fits Noema's principle that memory and context should be human-owned and inspectable.

## Goals

- Prevent provider context-window failures, especially Foundation Models' 4,096-token system limit.
- Avoid constant transcript trimming that changes prompt shape and harms model caching.
- Preserve restart and resume by storing compacted context in Noema's canonical store.
- Keep summaries derived and inspectable, never replacements for original transcript items.
- Support local-first behavior for `foundation_local` while allowing future maintenance-model overrides.
- Keep the first UI slice minimal: show recoverable runtime errors in chat, but do not add compaction management controls yet.

## Non-Goals

- Hierarchical summary trees.
- User-facing summary editors or dashboard controls.
- Provider-session-only compaction hidden inside the Swift bridge or another adapter.
- Automatic deletion or mutation of original conversation items.
- Cross-conversation or global memory synthesis.

## Architecture

Add a durable `conversation_context_summaries` table to the store. Each row is a derived checkpoint for a conversation and context profile. A context profile is the selected `provider_kind` plus `model_profile`; future implementations may add provider-specific prompt-shape versioning if needed.

Fields:

- `summary_id`
- `conversation_id`
- `provider_kind`
- `model_profile`
- `summary_text`
- `covered_item_start_sequence`
- `covered_item_end_sequence`
- `source_item_ids`: all covered transcript item ids when the covered range is small, or a bounded sample plus the exact covered sequence range when the range is large
- `input_token_estimate`
- `summary_token_estimate`
- `compaction_provider_kind`
- `compaction_model_profile`
- `status`: `pending`, `active`, `failed`, or `superseded`
- `error_code`
- `error_message`
- `created_at`
- `updated_at`

The runtime prompt composer should use:

1. Stable system and developer instructions.
2. The latest active summary matching the selected conversation, provider, and model profile.
3. Verbatim transcript items after the summary checkpoint.
4. The current user turn.

The original transcript remains canonical. A summary is derived context state, similar to an index: useful, inspectable, regenerable, and safe to supersede.

## Provider Context Metadata

The provider contract should expose context metadata for prompt planning:

- context window tokens
- optional provider-native token counting
- default output token reserve
- optional compact-summary target guidance

Providers that cannot report metadata can return `None` for context window and continue with existing behavior until they opt in. `foundation_local` should advertise a 4,096-token context window and an explicit output reserve.

Token counting should prefer provider-native counters when available. Foundation Models exposes token counting in the Swift API, so the Swift bridge can eventually provide exact counts. The initial Rust-side fallback can use conservative estimates only for planning, not as the authority when a provider-native counter exists.

## Data Flow

On every turn, Noema resolves the selected agent provider and model. It then builds a candidate context using the latest active summary plus post-checkpoint verbatim transcript items.

If the candidate fits within the model context budget, the turn proceeds normally.

If the candidate would exceed the budget, Noema performs synchronous preflight compaction before sending the turn to the agent model. The compaction input is the previous active summary, if any, plus transcript items after that checkpoint. The compaction output becomes a new active checkpoint that covers the prior active summary's range plus the newly compacted transcript range.

After a successful compaction:

- insert the new summary as `active`
- mark prior active summaries for that conversation/provider/model profile as `superseded`
- reassemble the candidate prompt
- send the turn only if the prompt now fits

Noema should also schedule background compaction after turns when the projected next prompt crosses a threshold, initially 70 percent of the model context window. Background compaction uses the same code path but does not block the completed user turn.

## Summary Content Contract

The compaction prompt should request structured output with:

- concise rolling summary
- open loops and active user goals
- durable decisions
- unresolved references that matter for future turns
- recently active entities, projects, files, and tools
- explicit uncertainty and "do not infer" caveats

The summary should not invent memories or convert conversation-local context into graph memory. Normal memory extraction remains responsible for durable memory claims.

## Maintenance Model Selection

Compaction should use a context-maintenance model setting. The default is the agent's selected provider and model, which keeps local-only agents local. Later, Noema can expose overrides for cheaper, faster, or larger-context maintenance models.

For the first implementation, storing the selected compaction provider and model on each summary row is enough. The settings surface can come later.

## Error Handling

Background compaction failures should create a `failed` summary row and leave the current active checkpoint untouched.

Synchronous preflight compaction failures should fail closed. Noema should not send an over-limit prompt and should not silently drop transcript context. The user sees a recoverable chat error such as: "Context compaction failed before this turn could run." The safe provider error should be attached for inspection.

If compaction succeeds but the assembled prompt still does not fit, Noema should retry once with a smaller summary target. If it still cannot fit, the turn fails with an explicit "context could not be compacted enough for the selected model" error.

## Web Dashboard Behavior

The first web slice should not add compaction controls. Chat should display compaction-related runtime errors through the existing recoverable turn-error path.

Future inspection surfaces can show summaries as derived context artifacts, including covered ranges, provider/model provenance, status, and regeneration actions.

## Testing

Runtime and store tests should cover:

- `foundation_local` advertises a 4,096-token context window.
- active summaries are persisted with covered sequence ranges and provider/model provenance.
- prompt assembly uses the latest active summary plus post-checkpoint verbatim turns.
- synchronous compaction runs before an over-limit turn.
- background compaction creates a durable checkpoint when the projected next prompt crosses the threshold.
- failed background compaction leaves the prior active checkpoint untouched.
- failed synchronous compaction blocks the over-limit turn with a recoverable error.
- restart and resume reuse the latest active summary.
- original conversation items remain visible and canonical after compaction.

## Implementation Notes

Keep the first implementation scoped:

- Add the store table and repository helpers directly, consistent with pre-V1 schema policy.
- Add provider context metadata to the Rust provider contract and Foundation bridge only as needed for planning.
- Introduce a small prompt-context planning module rather than burying compaction decisions in `turn.rs`.
- Use existing provider generation paths for compaction so the daemon owns lifecycle and provider selection.
- Preserve cross-platform builds by making Foundation-specific metadata and token counting unavailable on non-macOS rather than introducing Apple tooling as a global dependency.
