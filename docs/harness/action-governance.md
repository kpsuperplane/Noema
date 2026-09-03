# Action Requests, Approvals, and Observed URLs

This document defines how Noema reviews and executes capability actions. It
refines the [security model](security.md) and
[capability contract](capabilities.md).

## Boundary

Noema treats active model context as potentially hostile. It does not use a
conversation-level clean or tainted state. It governs effects at the capability
boundary.

```text
agent proposes saved exact arguments
  -> source input check and deterministic policy
  -> immediate execution, human review, or LLM review
  -> live revalidation
  -> one execution attempt
  -> saved outcome and audit events
```

## Invariants

1. Models assess authorization and risk. They do not create authority.
2. Each reviewed effect uses one immutable action request and revision.
3. Human approval applies only to that saved revision and its exact arguments.
4. An LLM cannot override a deterministic hard rule.
5. A failed or invalid review requires a human decision.
6. Execution admission revalidates current policy and origin state.
7. Waiting for a human does not retain a worker lease or model request.
8. Capability results remain untrusted context after an authorized call.
9. Noema does not retry an external effect with an uncertain outcome.
10. A human decline blocks an equivalent browser effect in that Task generation.
11. Clients render server-owned state and valid actions.

## Authorization context

The action reviewer receives one bounded authorization context.

- Primary chat supplies the latest seven completed user-facing messages. The
  excerpt ends at the authenticated human item that started the turn.
- Human entries can create authority. Assistant entries can resolve a reference
  later adopted by the human, but cannot create authority alone.
- A chat-created Task saves the same bounded excerpt when created.
- A manually created Task uses its authenticated title and description.
- Task review uses its saved excerpt and current task identifiers. It does not
  reread mutable conversation history.
- Explicit grants and prior human decisions remain separate authority inputs.

A model-created task contract can narrow human authority. It cannot broaden
that authority. Tool results, external content, memory, and model output cannot
create authority independently.

### Structured evidence

Noema makes provenance claims only when structured matching can reproduce the
origin. Examples include:

- An exact URL from a web result.
- A recipient copied from a typed connector result.
- A normalized address matching a structured connector field.
- A selected task, artifact, file, or resource identifier.

Text similarity does not prove provenance. The action ID, revision, and saved
exact arguments prove action identity, not the origin of free-form content.

## Gateway

The gateway owns the transition from a proposal to an adapter call.

### Deterministic preflight

Preflight checks:

- The exact capability binding and operation.
- The source input schema and payload bounds.
- Resource and destination constraints.
- Current tool behavior and connection policy.
- Grants, hard denies, and authentication state.
- Secret handling and secure credential bindings.
- Network and SSRF policy when applicable.
- Task generation, current-run check, and cancellation state.

A hard violation fails before adapter execution. An LLM review cannot clear it.

The current connection policy selects one execution route:

- `ExecuteImmediately` invokes after preflight.
- `HumanReview` saves the request for a human decision.
- `LlmReview` saves the request and runs the reviewer model.

Risky behavior includes a non-read-only operation that is destructive or
open-world. A connection can always ask the human, let the reviewer decide, or
allow immediate execution. Review-every-call data sharing cannot combine with
never-ask execution.

### LLM review

The reviewer receives:

- The bounded authorization context, origin, and execution route.
- Destination and service metadata when the binding provides them.
- The saved exact arguments, safe summary, and argument-shape projection.
- Current capability, behavior, review route, and source input schema.
- Trusted session facts for supported browser actions.
- Descriptive browser page and target evidence for supported interactions.

Browser page and target evidence remains untrusted content.
It does not create human authority.

The reviewer has no tools. It returns one closed classification:

```text
authorization: explicit | substantive | weak | absent
risk: low | medium | high | critical
reason_codes: [closed vocabulary]
explanation: bounded text
```

Authorization and risk are independent.

- `explicit`: The human directly requested the action.
- `substantive`: The requested result clearly covers the action.
- `weak`: The action is a necessary step that the human did not state.
- `absent`: The action conflicts with, exceeds, or is unrelated to the request.

If the human requests work on a dynamic set, they do not need to name each
member. A read-only action on a plausible member has substantive authority.
Untrusted content can show set membership. It cannot define or broaden the
human-authorized set.

Noema applies one deterministic matrix after classification:

```text
explicit or substantive + low or medium risk -> execute
weak + low risk                              -> execute
all other completed classifications          -> human decision
invalid or unavailable review                 -> human decision
```

This policy applies to reviewed external writes. A write does not always need a
human decision when current connection policy selects LLM review.

The reviewer cannot issue a hard denial. Deterministic policy owns hard
failures. An automatic result is never reused across requests or revisions.

The reviewer model is selected at **Settings → Safety → Privacy**. Noema saves
the exact provider, account, model, profile, and reasoning selection used for
the assessment. It does not fall back to the acting model.

## Durable action request

An action request is the saved record of one proposed effect. Primary chat and
Tasks use the same record.

The record contains:

- Action ID and revision.
- Conversation or Task origin.
- Owner and requesting agent.
- Capability, operation, behavior, resource, and destination.
- Saved exact arguments and source schema.
- Current policy and capability revision identities.
- Reviewer assessment and decision route.
- Continuation identity and task current-run check when applicable.
- Execution state, timestamps, and outcome details.

The current states are:

```text
proposed
awaiting_approval
executable
executing
awaiting_authentication
succeeded
failed
outcome_uncertain
declined
superseded
cancelled
```

`awaiting_authentication` retains the approved execution claim while an MCP
credential flow is pending. `outcome_uncertain` records a possible external
effect whose result is unknown.

## Human decision

Production supports two decisions for a pending action request:

- Approve the saved revision once.
- Decline the saved revision.

A pending approval becomes `approved` or `declined`. Execution admission
consumes an approved decision. Live revalidation can supersede a pending or
approved request before execution.

For browser actions, a decline also blocks an equivalent effect in the same
Task generation. Snapshot numbers, page titles, and element references do not
make an effect different. The operation, page URL, target, destination, method,
and visible submitted values must show a material change before another action
request can enter review.

The decision checks the action revision and owner. A stale decision fails.
Noema does not infer a decision from free-form text.

Before execution, the gateway checks:

- The exact action revision and saved arguments.
- The approving human and one-shot decision state.
- Current capability, authentication, behavior, and source schema.
- Current grants, connection policy, and destination policy.
- Current Task generation and run when applicable.

The adapter receives the saved arguments. The model does not regenerate them.

## Continuation

Waiting for a decision is durable asynchronous work.

For primary chat:

1. Save the action request and its exact approval item.
2. Finish the proposing generation without a failed tool result.
3. After a decision, append one terminal result under the original call ID.
4. Start one bounded continuation without a synthetic human message.

The continuation identity prevents duplicate execution and duplicate follow-up
generation. A continuation failure does not change a completed action outcome.

For Tasks:

1. Save the action request and release the worker claim.
2. Mark the run as waiting for approval.
3. Keep the task gate and capability decision as separate authorities.
4. Resume under the same current Task generation and run.

Task recovery and cancellation invalidate stale requests. An executing external
effect can become `outcome_uncertain`, but it cannot be replayed automatically.

## Human attention and interaction

The **Needs you** projection derives pending decisions from stored state. It is
not a second action or task authority.

The same decision component appears in primary chat and Task detail. Its
default view shows:

- The proposed verb and target.
- The destination or resource.
- The payload, diff, or export summary.
- Information leaving Noema and its expected audience.
- `Approve once` and `Decline` controls.

Reviewer evidence and policy details remain behind disclosure. The client sends
one semantic decision with the expected action revision. It does not execute
optimistically or mirror the state machine.

## Observed URLs

The web adapter stores exact normalized URLs from search results and fetched
links. These values are ordinary information. The model still calls
`web.fetch` with a normal URL.

A URL fetch can execute without LLM or human review only when:

1. The normalized URL exactly matches a stored adapter observation.
2. The request uses `GET` or `HEAD` without a body.
3. It adds no credentials, cookies, private headers, or request-shaping data.
4. Conservative normalization changes only details such as a fragment.
5. Current DNS, redirect, scheme, and SSRF checks pass again.

This path means safe to retrieve automatically. It does not make fetched
content trusted. A changed query, header, body, method, or unobserved URL returns
to normal execution policy.

A web search sends its query to the configured search provider without a human
decision. The query remains visible in normal tool activity and audit data.
Fetching a returned destination still uses the exact observed-URL predicate.

## Audit events

The action event ledger records:

```text
proposed
reviewed
approval_requested
approved | declined
execution_started
succeeded | failed | outcome_uncertain
superseded | cancelled
```

Events reference the action request instead of duplicating private payloads.
They exclude secrets and preserve ordinary diagnostic metadata.
