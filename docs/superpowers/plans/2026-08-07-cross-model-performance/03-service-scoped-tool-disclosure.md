# Service-Scoped Tool Disclosure Plan

**Status:** Proposed vertical slice after wire compatibility

**Goal:** Let the active model select exact connected destinations from a small
directory, then expose only those destinations' operation schemas for the rest
of the turn.

**Observable outcome:** An already-authenticated calendar workflow selects the
exact calendar connection, resets the provider chain with only that group's
schemas, and completes without exposing unrelated mail or storage operations.

## Scope

This plan combines routing and disclosure because neither is useful alone. The
first milestone supports one selector call followed by one or more operations
from the selected destinations. It reuses the immutable capability snapshot,
existing `ModelTools` derivation, policy checks, and interaction catalog digest.

## Non-Goals

- Do not create a service registry, active-catalog authority, generic execute
  tool, free-form operation name, phrase router, or separate routing model.
- Do not support a second selection later in the same turn in the first
  milestone. The initial call selects every immediately required connection.
- Do not invent numeric service limits, delegation rules, or token-reduction
  gates before evaluation demonstrates a need.
- Do not change grants, connection revision checks, action review, egress, or
  gateway dispatch authorization.
- Authentication and human-intervention resume are a follow-up milestone. The
  first milestone exercises already-authenticated read-only or LLM-reviewed
  turns without a mid-turn human interaction.

## Exact Destination Directory

- Derive one entry per existing exact `CapabilityDestination` from the turn's
  immutable binding snapshot and `CapabilityServiceContext`.
- The selectable value is the existing `connection_id`, never the provider or
  service-family `service_id`.
- Each entry exposes the exact connection id, bounded display name, optional
  user-visible connection label, bounded description, and aggregate read/write
  availability already derivable from its bindings.
- Add `public_web` as the only reserved virtual destination when Noema web
  tools are eligible.
- Sort by exact identifier. Include no credentials, operation arguments,
  private results, or raw manifests.

## Selector Contract

Add local built-in `capability.select_connections`:

```json
{
  "connection_ids": ["exact-connection-id"]
}
```

- Values are a unique array drawn from the request's exact enum, including
  `public_web` only when available.
- The local result echoes the validated selected identifiers. It performs no
  network access, domain persistence, authorization, or side effect; its
  ordinary transcript call/result is persisted like other local tool activity.
- The model may answer without calling it when no external service is needed.
- Unknown or stale identifiers return the existing invalid-tool-result shape;
  they never resolve by display name or to a similar connection.

## Disclosure and Execution

1. Build the complete immutable capability snapshot exactly as today.
2. Derive initial `ModelTools` containing core built-ins, the selector, and the
   compact destination directory, with no external operation schemas.
3. Validate the selector call, record its ordinary local call/result items,
   and retain the selected connection-id set in turn-local state.
4. Derive a new `ModelTools` value by filtering the same immutable bindings to
   the selected connection ids. Do not store schemas or create another catalog
   object.
5. Treat changed tool definitions as a provider-chain boundary: clear
   `previous_response_id`, replay canonical messages plus selector call/result,
   and send the filtered schemas in deterministic order.
6. Dispatch still resolves through the immutable snapshot and requires the
   called operation to be present in the filtered `ModelTools` policy view.

Native tool definitions become the operation-description authority. Remove
duplicate per-operation name/description rows from native prompt context, but
retain the compact directory, unavailable notices, and cross-tool safety rules.

## Interaction Follow-Up

After the already-authenticated vertical path passes, extend reconstruction to
approval and authentication resume using the persisted typed selector result.
The existing interaction digest continues to hash the actual exposed tool
definitions. If exact reconstruction is impossible, stop rather than persist a
second capability snapshot or schema authority.

## Expected Touchpoints

- `crates/noema-runtime/src/daemon/runtime/model_tools.rs`
- `crates/noema-runtime/src/daemon/runtime/model_context.rs`
- `crates/noema-runtime/src/daemon/runtime/local_tools.rs`
- `crates/noema-runtime/src/daemon/runtime/turn/provider_request.rs`
- `crates/noema-runtime/src/daemon/runtime/turn/continuations.rs`
- Existing interaction resume code only in the follow-up milestone

## Tests and Acceptance

1. Initial provider requests contain core tools and the selector but no
   external operation schemas.
2. Selecting one of two same-family connections reveals only the exact chosen
   connection, proving account isolation.
3. Catalog expansion clears provider chaining and preserves canonical selector
   history.
4. A hidden operation cannot dispatch even though its binding exists in the
   immutable authority snapshot.
5. One already-authenticated calendar read or LLM-reviewed write scenario
   selects, executes, and finishes without unrelated schemas or human
   intervention.

Report initial/peak tool counts and aggregate input tokens as benchmark data;
functional correctness, not a fixed percentage reduction, is the gate.

**Budget:** 250–450 production lines, 120–220 test lines, 4–5 focused tests.

**Stop condition:** If the vertical path requires a new persisted catalog or a
second execution authority, stop and narrow the first milestone. Do not copy
bindings or schemas into routing state.
