# Progressive Tool Disclosure Plan

**Status:** Approved for implementation after service-first routing

**Goal:** Send only the operation schemas needed for the current service-level
step, while retaining the full immutable capability snapshot as execution
authority.

**Outcome:** Models no longer repeatedly ingest every connected operation or a
duplicate prose catalog, and newly selected services can be activated without
weakening policy or replay safety.

## Boundaries

- `CapabilityCatalogSnapshot` remains the complete immutable authority for the
  turn. Disclosure controls visibility only; it never adds executable power.
- Do not introduce a second capability registry, generic execute-anything tool,
  or free-form operation name.
- Canonical operation schemas remain unchanged and are still validated before
  dispatch.
- Service routing from plan 02 is the only way to activate external operation
  groups inside a turn.

## Active Catalog Model

Refine `ModelTools` into two explicit views backed by one snapshot:

- **Authority catalog:** every role-eligible binding pinned at turn start.
- **Active catalog:** core built-ins plus operations belonging to activated
  service ids and currently allowed by `ToolPolicy`.

The active view owns provider tool definitions, callable names, prompt rows,
and the catalog digest used for interaction fencing. Dispatch resolves against
the authority snapshot and additionally requires the operation to exist in the
active view.

## Disclosure Sequence

1. Initial request: core built-ins, `capability.select_services`, unavailable
   notices, and the compact service directory.
2. Selection continuation: monotonically add every eligible operation for the
   selected service ids.
3. Later selection: add another service group while retaining prior groups;
   never remove a tool during an in-flight provider/tool sequence.
4. Once six services are active, prohibit further expansion for that turn.

Web search/fetch/browser operations activate as one `public_web` group. A
service group includes both reads and writes so discovery can naturally lead to
an action, but policy and review continue to govern each call.

## Provider Continuation Rules

- A catalog expansion is a provider-chain boundary. Clear
  `previous_response_id`, replay canonical messages plus native selector
  call/result items, and send the newly active schemas as a fresh request.
- Encrypted reasoning is replayed only where the existing provider contract
  permits it; provider-neutral message reconstruction remains sufficient.
- After expansion, keep the provider catalog stable until another explicit
  selector call. Ordinary tool results do not rebuild or reorder schemas.
- Prompt-cache keys remain stable by conversation. Put stable core schemas
  first and activated service schemas afterward in deterministic name order.

## Remove Catalog Duplication

- Native tool definitions are the operation-description authority. Remove
  per-operation name/description rows from `ToolVisibilityContext` when native
  transport is active.
- Keep only transport, exact callable names, compact service rows, unavailable
  notices, and cross-tool safety/routing instructions in model context.
- For a non-native provider, retain the existing explicit no-tools disclosure;
  do not revive prose-encoded execution.
- Runtime debug records `authority_tool_count`, `active_tool_count`, activated
  service ids, catalog expansion count, and provider input tokens. It must not
  record schemas, arguments, or results.

## Interaction and Recovery

- Persist selector tool results through the normal transcript path. Rebuild
  the active service set from those typed results before checking the existing
  interaction catalog digest.
- An approval/authentication interaction pins the active catalog at proposal
  time. Resume fails closed if the authority revision or reconstructed active
  digest differs.
- A catalog source refresh never alters an existing turn; it applies only to a
  new turn or task run.
- Unknown calls, hidden-but-authorized calls, and active calls denied by policy
  all fail closed through the current gateway.

## Evaluation Changes

- Replace flat distractor operations in stateful Primary fixtures with
  synthetic service groups and a real selector step.
- Grade both routing and execution: exact services selected, unrelated schemas
  withheld, source read completed, identifiers/times grounded, and final action
  correct.
- Report initial and peak active tool counts plus aggregate input tokens per
  workflow.
- Re-run only stateful Primary, context-window, cache-accounting, and
  interaction-resume cases after implementation.

## Expected Touchpoints

- `crates/noema-runtime/src/daemon/runtime/model_tools.rs`: authority and active
  views over the existing catalog snapshot.
- `crates/noema-runtime/src/daemon/runtime/turn/provider_request.rs` and
  `turn/continuations.rs`: deterministic active schemas and provider-chain
  reset on expansion.
- `crates/noema-runtime/src/daemon/runtime/prompt_context.rs`: remove native
  operation-description duplication.
- `crates/noema-runtime/src/daemon/runtime/interaction_lifecycle.rs` and
  `interaction_resume.rs`: pin and reconstruct the active catalog digest.
- `crates/noema-model-evals/src/matrix_manifest.rs`, `matrix_runner.rs`, and
  `matrix_report.rs`: grouped distractor fixtures and tool/token measurements.

## Tests and Acceptance

- Initial requests contain no external operation schemas.
- Selecting mail and calendar reveals only those groups plus core tools.
- Expansion resets provider chaining and preserves canonical tool history.
- Hidden tools cannot dispatch even though they exist in authority.
- Approval and authentication resumes reconstruct the same active digest.
- Native prompt context no longer duplicates operation descriptions.
- Representative stateful workflows reduce median aggregate input tokens by at
  least 60% without reducing concrete-action pass rate.

**Budget:** 320–520 production lines, 180–280 test lines, 7–10 focused tests.

**Stop condition:** If interaction resume requires copying the capability
snapshot or schemas into a new persisted authority, stop. Reconstruct from
typed selector results or narrow the first slice to turns without intervention.
