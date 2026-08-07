# Evidence-Aware Continuation Stopping Plan

**Status:** Approved for implementation after contract compatibility

**Goal:** Stop tool loops based on whether calls produced new evidence or an
effect, rather than treating every successful response as progress.

**Outcome:** Empty and repeated searches finalize promptly, failures do not run
for dozens of rounds, and legitimate multi-step work retains a bounded path to
completion.

## Boundaries

- Do not infer semantic intent from tool names, English payload fields, or
  assistant prose.
- Do not persist raw result fingerprints or additional private result content.
- Do not let progress classification authorize an external action.
- Keep one auxiliary progress-audit model and the existing no-tools
  finalization path; do not add another background agent.

## Structured Progress Signal

Add a request-local `ToolProgressSignal` to `LocalToolResult`:

```text
NewEvidence | ChangedState | Empty | Unchanged | Failure | Unknown
```

- Built-in memory and web tools set the signal from their typed result
  structures: returned item/page counts, continuation cursor, and successful
  state transitions.
- Capability dispatch sets `ChangedState` from the binding's reviewed
  `read_only` behavior plus successful completion, never from operation-name
  substrings such as `create` or `update`.
- For read-only gateway results without typed progress metadata, compare an
  in-memory SHA-256 digest of the exact canonical model-visible payload with
  prior results from the same operation. The first unseen digest is
  `NewEvidence`; a repeated digest is `Unchanged`.
- A successful result with unknown semantics remains `Unknown`, not automatic
  novelty. It may continue once but cannot reset a non-progress streak.
- Digests and raw payloads are discarded with the turn and never enter debug
  spans, audit digests, events, or persistence.

## Policy

- Immediate audit after two consecutive `Empty`, `Unchanged`, or repeated-exact
  argument results without `NewEvidence` or `ChangedState`.
- Immediate audit after three consecutive failures.
- Periodic audit every eight provider continuations instead of twenty.
- Hard ceiling of 32 provider continuations instead of 80.
- Any `NewEvidence` or `ChangedState` resets the non-progress and failure
  windows. `Unknown` does not.
- A completed side effect never triggers automatic replay. The next round may
  summarize or continue with dependent reads, but an identical mutation remains
  protected by existing idempotency/review fencing.

Audit outcomes remain `continue`, `finalize`, `ask_human`, or `checkpoint`.
When an immediate audit is unavailable or malformed after its one repair, use
the existing no-tools finalization path rather than resuming the loop.

## Audit Digest

Replace the misleading success/novelty counters with bounded counts for all six
signals, the current non-progress streak, failure streak, side-effect count,
and the last five operation/status pairs. Include no arguments or result text.

The audit prompt must state:

- Empty or unchanged reads are not progress.
- A different query is not progress by itself.
- Completed requested state plus adequate grounding means finalize.
- Ask the human only when a consequential value cannot be resolved by another
  already-available source.

## Runtime Integration

- Use the same tracker in foreground turns and background task execution.
- Compute progress after canonical dispatch and before transcript compaction so
  compaction cannot erase the stopping signal.
- Preserve the current blocked-approval, authentication, uncertain-outcome,
  cancellation, and deadline exits before progress policy runs.
- Record only signal counts and stop reason in runtime debug spans.
- Replace `result_side_effect(name)` and unconditional `novel_result_count`
  rather than adding a parallel tracker.

## Evaluation Changes

- Run stateful cases through the production continuation tracker.
- Add deterministic scenarios for two empty searches, repeated identical
  results under different arguments, a new cursor/page, a successful mutation,
  three failures, and one unknown result followed by empty evidence.
- The missing-appointment scenario passes when two bounded searches produce no
  evidence, no write occurs, and the final response states that the source was
  not found without inventing details.
- Report provider continuation count and stop reason per stateful scenario.

## Expected Touchpoints

- `crates/noema-runtime/src/daemon/runtime/local_tool_results.rs`: carry the
  request-local structured progress signal.
- `crates/noema-runtime/src/daemon/runtime/local_tools.rs`, `web_actions.rs`,
  and `action_gateway.rs`: classify typed reads and reviewed state changes.
- `crates/noema-runtime/src/daemon/runtime/progress.rs` and
  `progress_audit.rs`: replace unconditional novelty and name-based effects.
- `crates/noema-runtime/src/daemon/runtime/turn/continuations.rs` and
  `background_task.rs`: share thresholds and the no-tools finalization exit.
- Stateful evaluator fixtures remain in their existing runtime/eval support
  authorities; do not build a second continuation simulator.

## Tests and Acceptance

- Empty successful results never increment novelty.
- Different arguments returning the same canonical payload become unchanged.
- Binding behavior, not the operation name, determines a state change.
- Two non-progress reads trigger one audit; audit failure finalizes without
  another external tool call.
- New pages/cursors reset the streak and allow legitimate continuation.
- Foreground and background loops share identical thresholds.
- Missing-appointment restraint improves without regressing the six concrete
  read/write scenarios.

**Budget:** 220–360 production lines, 160–240 test lines, 7–10 focused tests.

**Stop condition:** If generic canonical digests classify provider-controlled
volatile envelopes as novel, do not add field-name stripping. Leave those
results `Unknown` until their authoritative adapter supplies typed progress.
