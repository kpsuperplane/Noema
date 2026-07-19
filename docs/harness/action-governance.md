# Governed Actions, Approvals, and Web Provenance

This document is the working source of truth for how Noema reviews and executes
capability actions that can create an external effect or disclose information.
It refines the broader [security model](security.md),
[capability registry](capabilities.md), and [event ledger](events.md).

The design is intentionally implementation-facing, but it is not a statement
that every control described here is already implemented. Decisions still under
discussion are listed explicitly at the end.

## Design thesis

Noema's primary conversation is long-lived by design. Once it has consumed web
pages, messages, documents, or tool results, a conversation-level taint flag
will remain set forever and cannot make useful execution decisions.

Noema therefore governs effects at the capability boundary:

```text
agent proposes an exact action
  -> deterministic preflight
  -> LLM action review when judgment is required
  -> execute, deny, or persist a blocked action
  -> human decision when required
  -> revalidate the exact action
  -> execute and record the outcome
```

Untrusted context remains labeled and attributable, but its presence does not
automatically disable capabilities. Provenance informs review of a particular
action rather than becoming ambient permission state.

## Core invariants

1. Models may assess risk, but they do not create authority.
2. Every write or export is represented as an exact structured action before
   adapter execution.
3. An approval authorizes one immutable action fingerprint, not a prose intent
   or a future model-generated replacement.
4. Deterministic policy cannot be overridden by an LLM judgment.
5. A failed or uncertain review becomes a human decision, not an automatic
   execution.
6. Approval consumption and execution admission are revalidated atomically as
   closely as the adapter boundary permits.
7. Waiting for a human never depends on an in-memory model request, worker
   lease, or browser connection remaining alive.
8. A capability result remains untrusted context even when the action that
   obtained it was authorized.
9. External effects with an uncertain outcome are never blindly retried.
10. Product clients render server-owned action and approval state; they do not
    infer policy or workflow transitions.

## Context exposure and action provenance

Noema records three related but distinct facts.

### Context exposure

A run may record that it saw external or otherwise untrusted material. This is
useful for audit and for selecting review policy, but it is not an execution
gate by itself.

### Field provenance

Security-relevant action fields may carry provenance independently. Important
examples include a destination, recipient, URL, resource identifier, local
path, and data selected for export.

Provenance confidence has three levels:

- **Verified:** the harness can prove the value came from a specific adapter
  result or governed object, normally through an opaque reference.
- **Model-claimed:** the model cites source references, but the harness cannot
  prove that the generated value is an exact derivation.
- **Unknown:** no useful lineage is available.

Model-claimed provenance is evidence for the action reviewer, not a policy
grant. Exact structural provenance should use opaque handles wherever possible
so the harness can verify it without semantic inference.

### Action assessment

The action gateway compares one canonical proposal against:

- The current human instruction or immutable task contract.
- Active grants, denies, revocations, and action-class policy.
- The exact capability, operation, resource, destination, payload, and diff.
- Field provenance and relevant context exposure.
- The information leaving Noema and its expected audience.
- Current capability metadata, schema, calibration, and authentication state.

This assessment, rather than ambient conversation taint, decides whether the
action can execute automatically.

## Action gateway

The gateway owns the transition from a model proposal to an adapter call.

### Deterministic preflight

Deterministic validation runs before any LLM review. It includes:

- Exact capability and operation identity.
- Input schema validation and payload bounds.
- Resource and destination constraints.
- Active grants, hard denies, and revocations.
- Tool metadata and calibration fingerprints.
- Secret and credential handling rules.
- Network destination and SSRF policy where applicable.
- Origin task generation, contract lineage, and cancellation state.
- Whether the action class always requires human approval.

A hard violation is denied. The LLM reviewer cannot override it.

### LLM action review

The reviewer answers semantic questions that deterministic policy cannot
reliably answer, especially whether an action is supported by the human's
intent or has been shaped unexpectedly by untrusted content.

The reviewer receives a bounded, structured packet rather than an unrestricted
conversation transcript:

- The relevant human instruction or task contract.
- The canonical proposed action and human-readable diff or preview.
- The deterministic policy result and active authority.
- Provenance for security-relevant fields.
- Relevant untrusted excerpts, clearly separated as evidence.
- The proposed egress, destination, and audience.

The reviewer has no capabilities or tools. Its response uses a closed schema:

```text
authorization: supported | ambiguous | contradicted
untrusted_influence: none | data_only | action_shaping | unknown
egress: expected | unexpected | none | unknown
recommendation: allow | manual_review
reason_codes: [closed vocabulary]
```

The audit record stores the reviewer model, prompt-policy version, action
fingerprint, structured verdict, and evidence references. It should not copy a
full sensitive transcript into an ordinary log.

A timeout, provider failure, malformed response, unavailable reviewer, or
unknown result becomes `manual_review`. The reviewer may clear automation only
inside authority already supplied by deterministic policy, a human instruction,
or a task contract.

### Decision composition

The initial decision model is:

```text
hard deterministic violation
  -> deny

deterministic approval requirement
  -> persist blocked action

authorized action + clear LLM review
  -> execute

review concern, contradiction, failure, or uncertainty
  -> persist blocked action
```

Some action classes may always require a human even when the review is clear.
Candidate classes include public publication, destructive operations,
financial commitments, account or permission changes, and any action exposing
secrets. The exact initial set remains an open product decision.

## Durable governed actions

A governed action is the canonical record of one proposed effect. It is shared
by the primary conversation, Work tasks, and future proactive runs.

The record should contain:

- Action ID and revision.
- Origin kind and origin reference.
- Requesting agent and approving human.
- Capability, operation, effect class, resource, and destination.
- Canonical payload snapshot or governed payload reference.
- Payload, schema, capability, calibration, and policy fingerprints.
- Field provenance references.
- Deterministic decision and LLM assessment reference.
- Current state, creation time, expiration, and supersession reason.
- Continuation reference.
- Adapter idempotency key when supported.

Suggested action states are:

```text
proposed
  -> assessed
  -> executable | awaiting_approval | denied
  -> executing
  -> succeeded | failed | outcome_uncertain
```

Cancellation, expiration, revocation, origin-generation changes, and material
action changes can also move a nonterminal action to `superseded` or
`cancelled`.

## Approvals

An approval is a durable decision about one governed action. It is not a chat
message, task-stage inference, notification, or browser-local state.

The initial approval lifecycle is:

```text
pending -> approved | declined | expired | superseded
approved -> consumed | revoked
```

An approval binds to the action ID, action revision, and complete action
fingerprint. Changing the payload, destination, recipient, resource selector,
capability metadata, policy version, or relevant origin state requires a new
assessment and normally a new approval.

"Approve with changes" creates a new action rather than mutating an approved
payload. Initial implementation should support one-shot approvals only.
Reusable authority belongs in a separate grant model so approval history does
not silently become a permission system.

Immediately before execution, the gateway rechecks:

- Approval state, owner, expiry, and consumption state.
- The complete action fingerprint.
- Capability availability, authentication, calibration, and current schema.
- Active grants, denies, revocations, and policy version.
- Origin cancellation, task generation, and contract lineage where relevant.
- Destination and network safety.

Admission consumes a one-shot approval atomically with the durable transition
to execution where possible. The adapter call then uses the saved canonical
payload; the model does not regenerate it.

## Primary conversation and task resumption

Waiting for approval is durable asynchronous work.

For the primary conversation:

1. Persist the governed action and finish or suspend the proposing generation.
2. Render the pending action inline without blocking new conversation turns.
3. After approval or decline, append the exact outcome and start a fresh,
   bounded continuation when a model response is useful.

For a Work task:

1. Persist the action and release the active worker lease.
2. Mark the run as waiting with a reference to the governed action.
3. Keep the task gate and capability approval as distinct authorities: task
   gates govern task workflow, while governed actions authorize capability
   execution.
4. After resolution, execute or decline the saved call and requeue a bounded
   continuation under the same valid task generation and contract lineage.

No client or runtime may reconstruct approval from free-form text. Cancellation
or a new task generation supersedes pending actions. An adapter result whose
external outcome is uncertain moves to `outcome_uncertain` and requires an
explicit recovery decision rather than automatic retry.

## Human attention and delivery

Approval truth and notification delivery are separate concerns.

The web application initially consumes a canonical pending-action query and a
delivery-neutral attention stream. A human-attention record contains only the
recipient, source object, safe summary, urgency, and deep-link locator needed
to reach the governed action.

This supports three product placements from the same source:

- An inline approval card in the owning primary conversation.
- The same decision component at the top of an affected task detail.
- A global **Needs you** projection across conversations and tasks.

Settings remains the secondary surface for approval history, grants, and
policy administration. It is not the main inbox for live decisions.

Future mobile delivery should consume the same durable attention event through
a channel adapter. Push payloads must contain only a safe summary and opaque
deep link; proposed messages, document content, export payloads, and sensitive
diffs remain behind authenticated retrieval. Notification delivery never
changes approval state.

## Approval interaction

The inline card leads with the decision the human must make, expressed as a
concrete verb and target. For example:

```text
Send this message to Alex
Update roadmap.md
Export the selected memories
```

The default view shows:

- Destination or affected resource.
- Proposed payload, diff, or export summary.
- Data leaving Noema and expected audience.
- `Approve once` and `Decline` controls attached to the proposal.

Judge evidence, provenance, policy versions, fingerprints, and raw adapter
details remain available through disclosure. Primary chat and task detail reuse
one decision component and preserve the same information order at mobile
widths.

The UI sends a semantic approval command with an expected action revision. It
does not optimistically execute, infer approval from prose, or mirror the action
state machine in TypeScript.

## Web URL provenance

Web requests receive a narrow automatic fast path when Noema can prove that an
exact URL came from a public, anonymous web chain and that Noema added no
private request data.

Public origin does not make a page or its instructions trustworthy. It only
supports a lower egress-risk decision for retrieving the exact referenced URL.

### URL references

The web adapter should issue opaque `url_ref` values for:

- Search-result destinations.
- Links extracted from fetched response bytes.
- Validated HTTP redirects.

Each reference records:

- The exact and normalized URL.
- The source search result, page, or redirect event.
- The parent URL reference and provenance chain.
- The originating request's egress and sensitivity classification.
- Whether the source request was anonymous and public.
- Creation and validation times.

The model requests `web.fetch` using `url_ref` rather than copying the URL back
as an unverified string. Arbitrary URL strings remain supported through the
ordinary reviewed path.

### Exact public-follow predicate

Fetching a URL reference may bypass LLM and human review only when:

1. Noema's web adapter created the reference from response bytes, a structured
   search result, or a validated redirect.
2. Every ancestor request was an anonymous public `GET` or `HEAD`.
3. The new request remains `GET` or `HEAD` with no body.
4. No credentials, cookies, private headers, or user-supplied arguments are
   added.
5. The URL is unchanged except for safe normalization such as dropping a
   fragment.
6. The ancestor chain did not carry private request material that could be
   reflected into a cross-origin link.
7. Current DNS, SSRF, redirect, scheme, and public-address checks pass again at
   execution time.

The fast path means `safe to retrieve automatically`, not `trusted content`.
Fetched output remains untrusted, and links extracted from it receive their own
provenance records.

Changing query parameters, adding headers or a body, using authentication,
submitting a non-read method, supplying a model-generated URL, or copying URL
text breaks the exact chain and returns the request to ordinary action review.

### Search is two egress decisions

A web search has two distinct security decisions:

1. Sending the query may disclose private context and can require redaction or
   LLM review.
2. Following an exact returned destination is normally low risk once URL
   provenance proves that Noema added no additional private data.

A result URL can contain tracking or reflected query information. If the
ancestor search disclosed private material, a cross-origin follow does not
automatically receive the public fast path.

## Audit events

The governed action lifecycle should produce durable events compatible with
the harness event ledger:

```text
capability.invocation_proposed
capability.schema_validated
policy.evaluation_requested
policy.decision_recorded
egress.review_requested
approval.requested
approval.granted | approval.denied | approval.expired
capability.invocation_started
capability.invocation_completed | capability.invocation_failed
egress.sent | egress.failed
```

Events reference governed payloads and redacted evidence rather than copying
full prompts, completions, secrets, or export bodies into ordinary event JSON.
The audit trail must distinguish the proposed, approved, attempted, and
observed action states.

## Implementation sequence

1. **Action contract:** define governed-action, assessment, approval,
   fingerprint, provenance, and revalidation invariants in the domain and
   store authority.
2. **Primary write/export slice:** take one foreground MCP write or export from
   proposal through review, inline approval, exact execution, continuation,
   and audit.
3. **Task and attention integration:** allow task runs to wait without holding
   leases, reuse the same approval component, and expose a global pending-action
   projection.
4. **URL provenance:** introduce opaque references for search results, then
   page links and redirects, and implement the exact-public-follow policy.
5. **Adversarial evaluation:** measure action-shaping injection detection,
   expected-data transformations, false escalations, provider failures, stale
   approvals, cancellation, and uncertain external outcomes.

Each slice should preserve exact action identity end to end. A later slice must
not require replacing the approval authority introduced by the first.

## Open decisions

The following choices remain unresolved and should be decided before the first
runtime slice:

1. May a clear LLM review automatically execute a write or export already
   covered by an exact current human instruction or task contract, or do all
   writes and exports initially require a person?
2. Which action classes always require manual approval regardless of the LLM
   review?
3. Which existing Work notification records should be generalized into the
   shared human-attention projection, and which should remain task-specific?
4. How long should foreground and task approvals remain valid by default?
5. Which reviewer model preference and fallback behavior should the first
   implementation expose?
6. Should the first URL-reference slice support only search results, or search
   results plus links extracted from fetched pages?
