# Service-First Tool Routing Plan

**Status:** Approved for implementation after contract compatibility

**Goal:** Let the active model choose the relevant connected service from a
small structured directory before it must choose among operation-level tools.

**Outcome:** Public-web, mail, calendar, storage, and other source decisions are
explicit and inspectable, without brittle English matching or a separate
intent-routing model.

## Boundaries

- Service selection is model guidance, not authorization. Existing role,
  scope, connection revision, action review, and execution policy still decide
  whether an operation can run.
- Do not infer service choice from keywords, prefixes, provider names, or
  English-only rules.
- Do not add a model preference or another LLM call owned by a new subsystem.
  The selected Primary/Executor model performs routing through a native tool.
- Built-in local tools needed for ordinary chat—presentation, task commands,
  agent identity, and memory—remain directly visible.

## Service Directory

- Derive one immutable entry per exact `CapabilityDestination` using existing
  `CapabilityServiceContext`: connection id, display name, optional connection
  label, bounded service description, and aggregate read/write availability.
- Add virtual entries for Noema-owned web search/fetch and browser access when
  those tools are eligible. Hosted web search is represented by the same
  `public_web` identity.
- Never place credentials, provider account ids that grant authority, raw
  manifests, operation tokens, or private tool results in the directory.
- Sort entries by stable service id. The directory is a request snapshot and
  cannot widen after the turn begins except through an explicit catalog
  refresh on a later turn.

## Native Selection Contract

Add one built-in tool, `capability.select_services`, with this canonical input:

```json
{
  "service_ids": ["exact-service-id"],
  "purpose": "concise reason these services are needed"
}
```

- `service_ids` is an exact enum from the request snapshot, contains one to
  three unique values per call, and cannot name unavailable services.
- The tool returns selected service identities and operation counts, not tool
  schemas or external data.
- It performs no network access, persistence, authorization, or side effect.
- The model may answer directly without calling it when no external service is
  needed.
- Cross-service work selects all immediately known participants together—for
  example, mail plus calendar. A later selection may add services if a tool
  result reveals a genuinely new dependency.

## Prompt Contract

- Replace operation-heavy routing prose with one compact rule: select the
  service that owns the required source or destination before using its tools.
- State that another service is not evidence for a named service and that
  service selection conveys no permission to mutate it.
- Public facts route to `public_web`; private messages route to the exact mail
  connection; existing calendar state routes to the exact calendar connection.
  These are semantic examples in model guidance, never runtime match rules.
- When multiple same-kind connections exist, expose their connection labels.
  If the user and context do not identify one, the model asks one blocking
  question rather than selecting arbitrarily.

## Runtime Behavior

1. Build the full immutable authority snapshot exactly as today.
2. Build the compact service directory and selector schema from that snapshot.
3. Send core built-ins, the selector, and the directory on the initial model
   request; do not send external operation schemas yet.
4. Validate a selector call against exact service ids.
5. Return a local structured result and hand the selected ids to progressive
   disclosure for the next provider round.
6. Allow at most two selector calls and six distinct activated services in one
   turn. Beyond that, stop for a focused scope clarification or delegate the
   work when the user already authorized a broad task.

## Failure and Resume Rules

- Unknown or stale service ids return a typed invalid-selection result and one
  repair opportunity; they never fall back to a similarly named service.
- An unavailable/auth-required service preserves the existing intervention
  flow. Selecting it does not bypass setup or authentication.
- Persist the selector call/result as ordinary local tool activity. Its result
  is sufficient to reconstruct activated service ids after an interaction
  resume; no database schema change is required.
- If a provider cannot call the selector natively, the route is incompatible
  with external tools and fails before execution rather than receiving the
  entire catalog as a fallback.

## Expected Touchpoints

- `crates/noema-runtime/src/daemon/runtime/model_tools.rs`: derive the compact
  directory from the existing immutable binding snapshot.
- `crates/noema-runtime/src/daemon/runtime/model_context.rs` and
  `prompt_context.rs`: render the directory and compact routing contract.
- `crates/noema-runtime/src/daemon/runtime/local_tools.rs` and
  `local_tool_results.rs`: define and execute the local selector contract.
- `crates/noema-runtime/src/daemon/runtime/turn/continuations.rs` and
  `interaction_resume.rs`: accept selection results and reconstruct them.
- `crates/noema-capabilities/src/binding.rs` remains the behavior and
  destination authority; the selector must consume it rather than mirror it.

## Tests and Acceptance

- Synthetic public-event requests select public web, not mail or calendar.
- Synthetic email-to-calendar requests select both exact mail and calendar
  services without activating unrelated storage tools.
- Same-kind unlabeled ambiguity produces a question and no selection.
- Selection never changes execution decisions or grants.
- Unknown, stale, unavailable, and over-limit selections fail safely.
- Selector state reconstructs from durable tool activity on resume.

**Budget:** 220–380 production lines, 140–220 test lines, 6–9 focused tests.

**Stop condition:** If the selector cannot reliably choose service identities
across the representative Primary candidates after contract compatibility is
fixed, do not add phrase routing. Keep the directory and evaluate a separately
scoped router design.
