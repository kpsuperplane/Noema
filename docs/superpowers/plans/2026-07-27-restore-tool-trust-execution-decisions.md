# Restore Tool Trust and Execution Decisions

**Mode:** Plan only

**Status:** Complete

**Date:** 2026-07-27

## Outcome

Restore the original per-provider MCP trust behavior and remove the later
`effect`/`admission` taxonomy from the shared capability, review, GraphQL, and UI
path. Tool behavior remains the four existing hints, provider trust remains the
two existing policies, and their only runtime policy result is:

```text
EXECUTE_IMMEDIATELY | HUMAN_REVIEW | LLM_REVIEW
```

This plan is a bounded correction and simplification of current authorities. It
does not redesign MCP setup, add API policies, or build unified Settings. The
unified management plan starts only after this plan is complete.

## Baseline and demonstrated drift

The original MCP contract persists:

- `readOnly`, `idempotent`, `destructive`, and `openWorld` per tool, with
  provenance and revision fencing;
- `allow_automatically` or `review_every_call` per provider connection;
- `always_ask`, `reviewer_may_approve`, or `never_ask` per provider connection.

The MCP catalog currently derives both `CapabilityEffect` and
`CapabilityAdmissionPolicy`, the action gateway interprets those derived
values, and the router separately rejects some combinations. That split drifted
from the original matrix: a safe additive closed-world mutation is derived as
an external write that should execute directly, but the router rejects it
because the effect taxonomy independently demands review.

The current API adapter manifest also contains source-specific `effect` and
`admission` fields. They remain migration-only source input during this plan so
the runtime correction does not absorb the API policy migration. The unified
management plan removes those final fields when APIs adopt the four hints and
two connection policies.

## Authoritative policy

Use the original definitions exactly:

```text
risky =
  !readOnly
  && (destructive || openWorld)

unsafe =
  risky
  || dataSharingPolicy == review_every_call
```

Resolve one execution decision:

```text
if !unsafe:
  EXECUTE_IMMEDIATELY
else if unsafeActionPolicy == never_ask:
  EXECUTE_IMMEDIATELY
else if unsafeActionPolicy == always_ask:
  HUMAN_REVIEW
else:
  LLM_REVIEW
```

Reject `review_every_call + never_ask` before a connection becomes callable.
An LLM review may authorize execution or escalate to human review; malformed,
timed-out, unavailable, or inconclusive review always escalates to the human.

Consequences:

- Safe reads and safe additive closed-world mutations execute immediately.
- `never_ask` changes the handling of an unsafe call; it does not make the tool
  behavior safe.
- `review_every_call` makes every call unsafe, including otherwise-safe reads.
- `idempotent` remains inspectable behavior evidence. It does not grant retry
  permission in this restoration slice.
- The four hints and both provider-policy revisions remain attached to the
  exact advertised operation authority and are revalidated before invocation.

## Shared capability contract

Replace `CapabilityEffect`, `CapabilityAdmissionPolicy`, and their compound
`CapabilityAccess` use with the facts each consumer actually needs:

- complete `CapabilityToolBehavior`: the four booleans;
- `CapabilityExecutionDecision`: `ExecuteImmediately`, `HumanReview`, or
  `LlmReview`;
- the existing exact operation target and schema;
- the existing ownership scope used for local task/conversation writes;
- an optional exact revision-fenced destination for connection-backed and web
  tools;
- the existing lifecycle sanitizer.

Do not introduce a replacement write/read/export class, risk-level enum, or a
renamed admission enum. `risky` and `unsafe` are derived booleans, not stored
taxonomies. Human-facing explanations use the original hint and policy terms.

Role-based model-tool visibility remains a separate authorization concern. Keep
the existing builder-supplied role allowlist, rename
`GovernedExternalAction` to the accurate `ExternalTool`, and stop deriving role
access through `CapabilityEffect`. Connected tools are external because they
have a destination, while local ownership and terminal/control roles remain
explicit source facts. No tool role may be inferred from names or prose.

All built-in, web, MCP, and adapter bindings must carry a complete behavior
snapshot. Built-ins use reviewed static values. MCP uses its persisted effective
hints. Adapter manifest v2 temporarily translates at the adapter boundary:
retrieval-only operations set `readOnly`, an explicit safe-read retry contract
may set `idempotent`, and every unproven value uses the pessimistic default.
Its current direct/human/reviewer choice maps to the corresponding three-way
decision without entering shared types. This preserves adapter behavior until
the unified plan replaces the manifest fields with native hints and connection
policies; no adapter vocabulary leaks into the shared binding.

## Router and reviewed execution

The action gateway owns the execution-decision lifecycle:

1. `EXECUTE_IMMEDIATELY` dispatches without creating a reviewed-action record.
2. `HUMAN_REVIEW` persists the exact proposal and waits for the human without
   calling the reviewer.
3. `LLM_REVIEW` persists the exact proposal, runs the existing reviewer and
   deterministic authorization/risk policy, then either executes or waits for
   the human.

Rename the one-shot `GovernedCapabilityAdmission` token to a neutral reviewed
execution authorization. It continues to bind the action ID, action revision,
and exact canonical argument digest. Rename governed dispatch methods to
reviewed dispatch methods where they expose the retired terminology.

The router enforces only the exact derived decision:

- ordinary dispatch is accepted only for `EXECUTE_IMMEDIATELY`;
- `HUMAN_REVIEW` and `LLM_REVIEW` require the exact one-shot reviewed execution
  authorization;
- connection-backed and web calls require their exact destination regardless
  of the decision;
- any supplied authorization must match the canonical arguments;
- unknown, stale, mismatched, or missing authority fails closed.

The router does not independently reinterpret tool behavior. This removes the
current second policy decision while retaining exact target, destination,
arguments, and revision checks.

## Failure, authentication, and retry behavior

Mutation uncertainty follows the original tool fact rather than a synthetic
effect:

- an ambiguous connection-backed send with `readOnly == false` becomes
  `outcome_uncertain` and is never automatically repeated;
- a read-only transport failure remains an ordinary unavailable/failed result;
- `idempotent == true` alone never permits retry;
- API transport retry remains governed by the adapter's reviewed transport
  contract until the unified plan joins it with connection-specific
  idempotency.

Authentication pauses retain the exact operation token, behavior/policy
revision, destination, arguments digest, provider route, and any reviewed
action identity. Resume re-resolves the binding and rejects any changed
decision, behavior revision, destination, schema, or credentials before send.
The canonical-result contract remains unchanged: connection invokers produce
one bounded secret-redacted result used by model, transcript, replay, and
continuation.

## Durable reviewed actions and projections

Remove `GovernedActionEffect` from the store, runtime reviewer, GraphQL, and
frontend. A new reviewed-action record stores:

- the originating execution decision (`human_review` or `llm_review`);
- the exact tool behavior snapshot when available;
- the exact operation token, schema, arguments/digest, destination and policy
  revision context;
- the existing safe summary, lifecycle state, assessment, approval, output,
  and failure fields.

Append a forward SQLite migration that rewrites `governed_actions` without the
`effect` column. Derive the old decision from the stored policy context:
`always_ask` becomes `human_review` and `reviewer_may_approve` becomes
`llm_review`. Existing nonterminal rows lack a trustworthy four-hint snapshot,
so mark them superseded and let their owning foreground/task recovery path
resume safely. Preserve terminal history and its existing safe summary without
inventing behavior values.

The GraphQL and pending-action UI remove write/export labels. They expose the
review route and, for new records, the four original behavior hints. Default
copy answers what the tool may do, where it will act, and why review is needed;
raw IDs and policy revisions remain under disclosure.

## Implementation sequence and budgets

Each numbered unit is one commit. Measure every unit against its starting
commit and stop at the stated budget.

1. **Decision core and MCP regression** — Add the behavior snapshot and
   three-way decision to `noema-capabilities`; replace MCP effect/admission
   derivation with the exact original resolver; adapt the binding/router
   contract; prove the safe additive closed-world mutation regression. Budget:
   net-negative production preferred, maximum +150 production, +140 tests, at
   most three new tests.
2. **Runtime, role policy, and source adapters** — Make the action gateway and
   reviewed dispatch consume only the decision; replace effect-derived role
   access; update built-in, web and adapter binding producers; keep adapter v2
   translation private to the adapter boundary; use `readOnly` for uncertain
   mutation outcomes. Budget: net-negative production, +120 tests, at most two
   new tests.
3. **Durable action vocabulary and cleanup** — Append the SQLite migration,
   update action persistence/replay/authentication, remove effect/admission from
   GraphQL and pending-action UI, regenerate types, delete obsolete mappings and
   update capability/frontend contracts. Budget: maximum +250 production before
   deletions, +160 tests, at most three new Rust tests; final non-generated
   production delta across all three units must be net-negative.

Likely authorities touched:

- `crates/noema-capabilities/src/{binding,router}.rs` and exports;
- `crates/noema-capabilities/mcp/src/{catalog,eligibility,invocation}.rs`;
- the adapter compiler/catalog/invoker boundary, without changing canonical
  manifest or connection schemas;
- `crates/noema-runtime` model-tool assembly, action gateway/resolution,
  authentication resume, and focused tests;
- `crates/noema-store` governed actions and schema migration;
- `crates/noema-api/src/graphql/governed_actions.rs` and the existing pending
  action component/operations plus generated artifacts;
- `docs/context/current.md` and the closest capability/frontend contract.

## Focused test matrix

- A table-driven resolver test covers every original safe/risky/sharing/policy
  branch and the invalid policy pair.
- An end-to-end MCP test proves `readOnly=false`, `destructive=false`,
  `openWorld=false`, and automatic sharing reaches the invoker immediately.
- The same end-to-end path proves unsafe `always_ask`,
  `reviewer_may_approve`, and `never_ask` choose human review, LLM review, and
  immediate execution respectively; reviewer failure reaches human review.
- Reviewed replay fails when the tool fingerprint, tool-policy revision,
  provider-policy revision, destination, schema, or canonical arguments change.
- An ambiguous mutable connection send records `outcome_uncertain` without a
  second send; idempotency does not change that behavior.
- Store migration preserves terminal reviewed-action history, supersedes active
  legacy actions, and leaves no callable legacy effect/admission authority.

Do not repeat pure enum/string mappings across crates. Extend the existing
router, MCP catalog/invocation, governed-action, and schema tests at their
authoritative layers.

## Acceptance

1. The original trust matrix is the only policy decision for MCP calls.
2. Safe additive closed-world mutations execute immediately in foreground and
   permitted task-executor runs.
3. Human and LLM review share the exact durable action, argument binding,
   approval, authentication, and replay lifecycle.
4. Changing any effective tool/provider policy or destination fences later
   execution and delayed resume.
5. Root capability, router, runtime, store, GraphQL, and frontend code no longer
   contain effect/admission policy types or write/export approval labels.
6. Adapter manifest v2's fields are isolated migration-era source input and are
   not runtime authority; their removal is explicitly owned by the dependent
   unified API/MCP plan.
7. The single canonical-result contract, destination fencing, DNS/redirect
   protections, credential isolation, and outcome-uncertainty behavior remain
   intact.

## Stop conditions and non-goals

Stop if the correction introduces another behavior classification, approval
route, router-local risk rule, or parallel reviewed-action lifecycle. Stop if
it requires weakening exact destination, argument, revision, authentication,
or result boundaries.

Do not change MCP setup UI, API connection policy, adapter canonical schemas,
OAuth flows, classifier behavior, retry features, or unified Settings in this
plan. Do not preserve a long-lived compatibility API for the retired runtime
terminology. The separate stuck OAuth intervention-card bug is also outside
this architectural correction.
