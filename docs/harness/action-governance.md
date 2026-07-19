# Governed Actions, Approvals, and Observed URLs

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

Noema universally assumes that the active context may contain untrusted data
and prompt injections. There is no runtime transition from clean to tainted and
no attempt to reset or declassify the primary conversation. The action review
compares one exact proposal with trusted human authority and the deterministic
policy at the capability boundary.

## Core invariants

1. Models may assess risk, but they do not create authority.
2. Every write or export is represented as an exact structured action before
   adapter execution.
3. An approval authorizes one immutable action revision and saved payload, not
   a prose intent or a future model-generated replacement.
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

## Universal exposure and trusted authority

Every agent run is treated as though it has already observed malicious or
prompt-injected content. Noema does not persist a context-exposure boolean,
ambient taint classes, or a clean/tainted session state because the long-lived
primary conversation makes those distinctions permanently converge.

The action reviewer instead receives a narrow trusted-authority channel:

- The authenticated human message that initiated the primary turn.
- A bounded human-only message history when the latest message is too
  elliptical to establish intent.
- For Work, the originating human intent and the immutable task contract.
- Explicit grants and prior human decisions.

A model-produced task contract may narrow the originating human authority, but
it cannot broaden it. Assistant text, model plans, tool results, web content,
MCP content, and memory are always untrusted evidence rather than authority.

### Structured evidence

Noema makes provenance claims only for values whose origin can be reproduced
through structured matching. Examples include:

- A URL exactly matching a URL previously emitted by a web adapter.
- A recipient or object ID copied unchanged from a typed connector result.
- A normalized phone number or email address matching a structured connector
  field.
- A file, task, artifact, or resource ID selected through a typed capability
  result.

Finding the same value in arbitrary prose does not establish provenance. Model
citations, textual similarity, and semantic classifiers cannot prove that a
generated or reworded payload came from a particular source.

### Payload identity is not payload provenance

The gateway retains the exact canonical arguments proposed for execution so a
human can inspect them and an approved action can later execute without model
regeneration. The action ID, revision, and immutable payload snapshot provide
the required identity. Noema does not claim which source sentences, memories,
or tool results caused free-form payload content to be written.

### Action assessment

The action gateway compares one canonical proposal against:

- Authenticated human intent, constrained by the immutable task contract when
  the action belongs to Work.
- Active grants, denies, revocations, and action-class policy.
- The exact capability, operation, resource, destination, payload, and diff.
- Verified structured evidence where it exists.
- The information leaving Noema and its expected audience.
- Current capability metadata, schema, calibration, and authentication state.

The reviewer assumes all other context is potentially hostile. This assessment
decides whether the action can execute automatically.

## Action gateway

The gateway owns the transition from a model proposal to an adapter call.

### Deterministic preflight

Deterministic validation runs before any LLM review. It includes:

- Exact capability and operation identity.
- Input schema validation and payload bounds.
- Resource and destination constraints.
- Active grants, hard denies, and revocations.
- Current tool metadata identity and calibration.
- Secret and credential handling rules.
- Network destination and SSRF policy where applicable.
- Origin task generation, contract lineage, and cancellation state.
- Whether the action class always requires human approval.

A hard violation is denied. The LLM reviewer cannot override it.

### LLM action review

The reviewer answers semantic questions that deterministic policy cannot
reliably answer, especially whether an action is supported by the human's
intent or has been shaped unexpectedly by untrusted content.

The reviewer follows the useful boundary demonstrated by
[Hermes Guardian](https://github.com/kpsuperplane/hermes-guardian): authenticated
owner intent is separated from model and tool context, deterministic hard rules
run first, and the reviewer sees the real proposed action through a closed
schema. Noema does not carry over Guardian's session-taint or semantic
payload-provenance model.

The reviewer receives a bounded, structured packet rather than an unrestricted
conversation transcript:

- The trusted-authority channel described above.
- The canonical proposed action and human-readable diff or preview.
- The actual bounded action arguments.
- The deterministic policy result and active authority.
- Verified structured evidence where available.
- The proposed egress, destination, and audience.

The reviewer has no capabilities or tools. Its response uses a closed schema:

```text
authorization: explicit | substantive | weak | absent
risk: low | medium | high | critical
recommendation: auto_execute | require_approval
reason_codes: [closed vocabulary]
```

The reviewer assesses whether the proposed payload and destination are
consistent with trusted human intent. This is semantic judgment, not a claim
that Noema can trace paraphrased content to its source.

The audit record stores the reviewer model, prompt-policy version, action ID and
revision, structured verdict, authority references, and verified structured
evidence. It should not copy the payload or a full sensitive transcript into an
ordinary log.

A timeout, provider failure, malformed response, unavailable reviewer, or
unknown result becomes `require_approval`. The reviewer may clear automation
only inside authority already supplied by deterministic policy and the
authenticated human instruction. A task contract can constrain that authority
but cannot create it.

When the current human message is too elliptical to decide authorization, the
reviewer may request one retry with bounded human-only history. If the expanded
context remains insufficient, the action requires approval. Reviewer results
that request approval may be cached briefly for the same action revision;
automatic-execution results are never cached across actions or revisions.

### Reviewer model selection

The reviewer can use one of three model-selection policies:

1. **Inherit the acting model:** use the same provider and model as the agent
   proposing the action. This requires no additional setup and keeps the
   payload inside the same provider boundary, but the action and review share
   the same model weaknesses and latency profile.
2. **Dedicated reviewer model:** select one configured model globally for
   action review. This permits a faster or independently chosen reviewer, but
   may disclose the proposed payload to a second provider and introduces a
   separate readiness dependency.
3. **Local-only reviewer:** require a local model for action review. This keeps
   review payloads on the machine, but review quality and latency depend on
   available local hardware and models.

The initial UI can expose `Same as acting model` plus the ordinary configured
model choices through one global Safety preference. It should not silently
fall back across providers: if the selected reviewer is unavailable or returns
an invalid verdict, the action becomes a blocked action requiring approval.

### Decision composition

The initial decision model is:

```text
hard deterministic violation
  -> deny

deterministic approval requirement
  -> persist blocked action

authorized action + `auto_execute` LLM review
  -> execute

review concern, contradiction, failure, or uncertainty
  -> persist blocked action
```

The LLM reviewer does not issue non-approvable denials. Deterministic security
rules are the sole authority for a hard deny; the reviewer either clears
automation or asks the human.

No action class categorically requires manual approval after deterministic
preflight. A write or export may execute automatically when the LLM review is
clear and existing human authority covers it. Deterministic hard rules remain
non-approvable regardless of action class or reviewer output.

## Durable governed actions

A governed action is the canonical record of one proposed effect. It is shared
by the primary conversation, Work tasks, and future proactive runs.

The record should contain:

- Action ID and revision.
- Origin kind and origin reference.
- Requesting agent and approving human.
- Capability, operation, effect class, resource, and destination.
- Immutable canonical payload snapshot.
- Schema, capability, calibration, and policy versions or identities.
- Verified structured-evidence references, when present.
- Deterministic decision and LLM assessment reference.
- Current state, creation time, update time, and supersession reason.
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

Cancellation, revocation, origin-generation changes, and material action
changes can also move a nonterminal action to `superseded` or `cancelled`.

## Approvals

An approval is a durable decision about one governed action. It is not a chat
message, task-stage inference, notification, or browser-local state.

The initial approval lifecycle is:

```text
pending -> approved | declined | superseded
approved -> consumed | revoked
```

Approvals do not expire on a timer. A pending action remains available until a
human resolves it or canonical state cancels or supersedes it. An approved
action is normally consumed immediately; if execution is delayed, the gateway
still performs the complete live revalidation before admitting it.

An approval binds to the action ID and immutable action revision. Changing the
payload, destination, recipient, resource selector, capability metadata, policy
version, or relevant origin state creates a new revision, which requires a new
assessment and normally a new approval.

"Approve with changes" creates a new action rather than mutating an approved
payload. Initial implementation should support one-shot approvals only.
Reusable authority belongs in a separate grant model so approval history does
not silently become a permission system.

Immediately before execution, the gateway rechecks:

- Approval state, owner, and consumption state.
- The exact saved action revision and payload snapshot.
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

The human-attention projection is the read-only **Needs you** inbox derived from
canonical state. It answers which blocked actions and Work gates currently
require this human without becoming another action, approval, or task
authority. The server may compute it with a bounded query or materialized read
model; clients cannot mutate it directly.

The web application initially consumes this canonical query and a
delivery-neutral update stream. A projected item contains only the recipient,
source object, safe summary, urgency, and deep-link locator needed to reach the
owning governed action or Work gate.

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

Existing Work notification records remain task-specific. The shared attention
surface aggregates canonical Work and governed-action state instead of
generalizing the Work notification outbox into a new cross-domain authority.

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

Judge evidence, verified structured matches, policy versions, and raw adapter
details remain available through disclosure. Primary chat and task detail reuse
one decision component and preserve the same information order at mobile
widths.

The UI sends a semantic approval command with an expected action revision. It
does not optimistically execute, infer approval from prose, or mirror the action
state machine in TypeScript.

## Observed URLs

Web requests receive a narrow automatic fast path when the exact URL has
already appeared in a governed web result. Whether the source calls the URL
public is not important; the useful fact is that Noema's adapter previously
observed the same structured value and the model has not added arguments to it.

The web adapter maintains a bounded local `observed_urls` table. Candidate
sources include:

- Search-result destinations.
- Links extracted from fetched response bytes.

Each row stores the normalized URL, source kind, source event reference,
observation time, and expiration. The table stores ordinary URLs and compares
them directly. It does not introduce an opaque `url_ref`, URL fingerprint, or
HMAC layer.

The model continues to call `web.fetch` with a normal URL. The gateway applies
conservative normalization and performs an exact database match. Only the web
adapter may insert observed rows; a model claim that a URL was previously seen
has no authority.

### Exact observed-URL predicate

Fetching a URL may bypass LLM and human review only when:

1. The normalized URL exactly matches a fresh `observed_urls` row created by
   the web adapter.
2. The request uses `GET` or `HEAD` with no body.
3. No credentials, cookies, private headers, or additional user-supplied
   arguments are added.
4. The URL is unchanged except for conservative normalization such as dropping
   a fragment.
5. Current DNS, SSRF, redirect, scheme, and public-address checks pass again at
   execution time.

The fast path means `safe to retrieve automatically`, not `trusted content`.
Fetched output remains untrusted, and URLs extracted from it are independently
recorded in `observed_urls`.

Changing query parameters, adding headers or a body, using authentication,
submitting a non-read method, or supplying an unobserved URL returns the request
to ordinary action review.

### Search is two egress decisions

A web search has two distinct security decisions:

1. Sending the query may disclose private context and can require redaction or
   LLM review.
2. Following an exact returned destination is normally low risk once URL
   matching proves that Noema added no additional request data.

A result URL can contain tracking or reflected query information. If the
ancestor search disclosed private material, a cross-origin follow does not
automatically receive the observed-URL fast path.

### Future structured form continuations

The `GET`/`HEAD` rule is the first policy predicate, not a permanent statement
that every request body is dangerous. A future browser adapter may record a
structured form continuation containing the form action, method, field names,
page-supplied values, and the values Noema proposes to add.

That would let the gateway verify an unchanged form target and page-supplied
fields while the action reviewer judges added values and the form's semantic
effect. A harmless `POST`-backed search or navigation could then be treated as a
read without treating arbitrary bodies or browser submissions as safe. The
first implementation should not build this machinery.

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
approval.granted | approval.denied
capability.invocation_started
capability.invocation_completed | capability.invocation_failed
egress.sent | egress.failed
```

Events reference governed payloads and redacted evidence rather than copying
full prompts, completions, secrets, or export bodies into ordinary event JSON.
The audit trail must distinguish the proposed, approved, attempted, and
observed action states.

## Implementation sequence

1. **Action contract:** define governed-action, assessment, approval, immutable
   revision, trusted-authority, structured-evidence, and revalidation invariants
   in the domain and store authority.
2. **Primary write/export slice:** take one foreground MCP write or export from
   proposal through review, inline approval, exact execution, continuation,
   and audit.
3. **Task and attention integration:** allow task runs to wait without holding
   leases, reuse the same approval component, and expose a global pending-action
   projection.
4. **Observed URLs:** persist bounded normalized URLs from search results and
   links extracted from fetched pages, then implement the exact-match fast
   path.
5. **Adversarial evaluation:** measure action-shaping injection detection,
   expected-data transformations, false escalations, provider failures, stale
   approvals, cancellation, and uncertain external outcomes.

Each slice should preserve exact action identity end to end. A later slice must
not require replacing the approval authority introduced by the first.

## Remaining open decision

Choose the initial reviewer model policy:

1. Inherit the acting model with no separate preference.
2. Expose `Same as acting model` plus a configurable dedicated model, with
   `Same as acting model` as the default.
3. Require a local reviewer model.

Regardless of selection, reviewer failure requires human approval and never
silently falls back to a different provider.
