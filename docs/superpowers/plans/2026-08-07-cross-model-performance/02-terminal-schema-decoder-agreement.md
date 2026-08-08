# Terminal Schema and Decoder Agreement Plan

**Status:** Independent implementation slice after wire compatibility

**Goal:** Make every built-in terminal decoder accept the nullable optional
shape produced by Noema's strict-schema lowering.

**Observable outcome:** A payload that satisfies a provider-lowered built-in
terminal schema also reaches its production decoder, with `null` and absence
normalized at one domain boundary.

## Scope

- Audit built-in terminal tools whose canonical schemas contain optional
  fields.
- Make their production argument types represent optional values as
  `Option<T>` where strict lowering can produce explicit `null`.
- Normalize `None` to the existing domain default immediately after decoding.
- Keep the provider and runtime invariants in their existing authoritative
  crates rather than creating a cross-crate test seam.

The demonstrated starting case is `task.report_blocked`:

- `context_markdown: null` becomes `""`.
- `suggested_answers: null` becomes `[]`.

## Non-Goals

- Do not change canonical terminal schemas merely to match an overly strict
  decoder.
- Do not add a compatibility DTO, schema mirror, provider-specific decoder, or
  fallback prose parser.
- Do not add repair attempts to progress audit, action review, memory
  consolidation, or other auxiliary classifiers without a demonstrated eval
  failure that warrants the latency and cost.
- Do not change external action retry or idempotency behavior.

## Authority and Normalization

- Canonical tool schemas remain the provider-facing contract.
- Strict lowering may require optional properties while making their values
  nullable; production decoders must therefore accept both absence and
  explicit `null`.
- The terminal decoder is the one normalization boundary. Downstream task
  logic continues to receive its existing concrete domain values.
- Required fields and discriminators remain required. This plan must not turn
  a genuinely required value into a default.
- Existing one-repair behavior for Planner, Executor, and Reviewer remains
  unchanged. Agreement should allow valid lowered payloads to pass before
  repair is needed.

## Implementation Sequence

1. Enumerate built-in terminal schemas and identify only optional fields that
   strict lowering makes nullable.
2. Update the corresponding production argument types and normalize after
   successful decoding.
3. Retain the provider-layer strict-lowering test proving optional properties
   become required and nullable.
4. Add one runtime table that feeds explicit-null payloads through each
   affected real terminal decoder and verifies domain normalization.
5. Run the focused terminal/runtime tests and the typed cases affected by the
   demonstrated mismatch.

## Expected Touchpoints

- `crates/noema-providers/src/response_support/strict_schema.rs`
- `crates/noema-runtime/src/daemon/task_run_context.rs`
- `crates/noema-runtime/src/daemon/runtime/typed_terminal_tools.rs`
- Existing provider/runtime terminal test modules

## Tests and Acceptance

1. The existing provider-owned lowering test proves optional properties become
   required and nullable without exporting the crate-private lowering helper.
2. One runtime-owned table passes explicit-null payloads through each affected
   real terminal decoder; no test-only compatibility layer or public API is
   added.
3. Required fields and invalid discriminators still fail.
4. `task.report_blocked` normalizes nullable context and suggestions to their
   current domain defaults.

These tests own different risks: provider lowering shape and runtime decode
normalization. Do not duplicate either invariant in the evaluator.

**Budget:** 50–150 production lines, 60–120 test lines, 2–4 focused tests or
table rows grouped by distinct risk.

**Stop condition:** If agreement requires changing a domain-required field,
stop and correct the canonical schema instead. Do not silently default data the
runtime actually requires.
