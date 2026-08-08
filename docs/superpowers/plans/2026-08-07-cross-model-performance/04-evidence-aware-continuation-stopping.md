# Evidence-Aware Continuation Stopping Plan

**Status:** Independent implementation slice

**Goal:** Stop treating every successful tool response as novel progress, then
use the existing progress audit after two proven no-progress reads.

**Observable outcome:** Exact repeated read results stop after a bounded audit
without changing the existing periodic audit or hard continuation ceilings.

## Scope

- Replace unconditional success-based novelty and operation-name side-effect
  inference inside the existing continuation tracker.
- Keep classification private to the tracker rather than adding another field
  to every `LocalToolResult` producer.
- Trigger the existing progress audit after two consecutive proven no-progress
  reads.
- Measure continuation count and stop reason in affected stateful cases.

## Non-Goals

- Do not change the existing periodic audit interval, hard continuation limit,
  or generic failure threshold in this slice.
- Do not add retry/repair behavior to progress audit.
- Do not persist result fingerprints or additional private result content.
- Do not infer progress from English field names, operation names, or assistant
  prose.
- Do not let progress classification authorize an external action.

## Tracker-Private Classification

Classify completed calls as:

```text
Evidence | StateChange | NoProgress | Unknown
```

- Failure remains represented by `LocalToolResult.success` and existing
  failure counters; it is not duplicated in this classification.
- A successful capability mutation is `StateChange` only when the immutable
  binding's reviewed behavior is not `read_only`.
- For read-only results, compare a request-local SHA-256 digest of the exact
  canonical model-visible payload with earlier results from the same operation.
  The first unseen payload is `Evidence`; a repeated payload is `NoProgress`.
- Typed built-ins with an authoritative empty result may report `NoProgress`
  directly through existing result-kind handling. Generic external capability
  payloads do not gain semantic meaning from JSON field names.
- Other successful results are `Unknown`. Unknown does not reset a proven
  no-progress streak and is never promoted to evidence merely because the call
  succeeded.
- Digests are discarded with the turn and never enter events, spans, audit
  prompts, transcripts, or persistence.

## Policy

- Two consecutive `NoProgress` read results trigger the existing progress
  audit.
- `Evidence` or `StateChange` resets the no-progress streak.
- Preserve blocked approval, authentication, uncertain outcome, cancellation,
  and deadline exits before applying progress policy.
- Preserve existing idempotency and review fencing for completed mutations.
- If the existing audit fails, retain its current fail-safe behavior; this plan
  does not add a repair loop or new finalization path.

The audit digest replaces misleading novelty totals with bounded evidence,
state-change, no-progress, unknown, and failure counts plus the current streak.
It includes no arguments or result text.

## Implementation Sequence

1. Replace unconditional novelty and `result_side_effect(name)` in the existing
   tracker with the private classification.
2. Add the two-no-progress audit trigger without changing other thresholds.
3. Use the same tracker in foreground and background paths if both currently
   share it; do not create a second adapter merely for symmetry.
4. Run only repeated-evidence and representative successful-action scenarios.
5. Propose threshold changes later only if the resulting continuation data
   demonstrates a separate problem.

## Expected Touchpoints

- `crates/noema-runtime/src/daemon/runtime/progress.rs`
- `crates/noema-runtime/src/daemon/runtime/progress_audit.rs`
- Existing local result/binding access used by the tracker
- Existing stateful evaluator fixtures

## Tests and Acceptance

1. The first unseen canonical read payload is evidence; two subsequent calls
   producing that same payload under different arguments count as no progress
   and trigger one audit on the third result.
2. Binding behavior—not operation spelling—identifies a state change and resets
   the streak.
3. New read evidence resets the streak; fingerprints never appear in persisted
   or debug output.
4. A representative grounded action scenario still completes.

## Deferred Missing-Collection Case

The current missing-appointment fixture returns two different empty Gmail
envelopes, so exact-payload comparison correctly cannot classify them as the
same result. Closing that case requires the Gmail capability's canonical result
contract to supply explicit evidence disposition, such as an adapter-owned
item count and continuation state. Plan that only when the production adapter
can supply it without parsing English field names or adding a generic metadata
registry. Until then, report the case as an open gap rather than claiming this
slice fixes it.

**Budget:** 80–180 production lines, 70–150 test lines, 3–4 focused tests.

**Stop condition:** If exact payload digests are unstable because an adapter
adds volatile fields, leave that result `Unknown`. Do not add heuristic
field-name stripping; typed adapter metadata requires a separate demonstrated
slice.
