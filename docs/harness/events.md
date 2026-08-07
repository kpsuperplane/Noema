# Run Event Ledger

The run event ledger is the durable explanation of agent execution.

Noema should not rely on mutable run status alone. Mutable status is useful for
dashboards, but the source of trust should be the append-only sequence of
events that produced that status.

The ledger should let Noema answer:

```text
What happened?
In what order?
Who or what caused it?
Which scopes and policies applied?
Which context was used?
Which tools were invoked?
Which approvals were requested?
Which data left the boundary?
Which artifacts and memory proposals were produced?
How can this run be resumed, replayed, explained, or exported?
```

## Ledger principles

- Events are append-only.
- Events are structured.
- Events are attributed to principals and components.
- Events reference durable objects instead of duplicating all content.
- Events preserve enough context to debug and audit.
- Secret material is excluded. Private payloads remain intact behind governed
  references when they do not belong inline.
- Current state is derived from events.
- Recovery uses events as the source of progress.
- Export should preserve human-readable and machine-readable trails.

## Event envelope

Every event should share a common envelope.

Conceptual shape:

```json
{
  "event_id": "evt_01...",
  "run_id": "run_01...",
  "sequence": 42,
  "event_type": "tool.invocation_completed",
  "occurred_at": "2026-06-23T20:01:12Z",
  "actor": { "object_type": "agent", "object_id": "agent:primary" },
  "component": "capability_gateway",
  "governable_context": {
    "object_type": "project",
    "object_id": "project:harness"
  },
  "causation_event_id": "evt_01...",
  "correlation_id": "toolcall_01...",
  "visibility": "private",
  "information_class": "private",
  "payload": {},
  "secret_exclusions": [],
  "content_hash": "sha256:..."
}
```

Required semantics:

- `event_id`: globally unique event identity.
- `run_id`: run that owns the event.
- `sequence`: monotonic per-run sequence.
- `event_type`: typed name.
- `occurred_at`: timestamp.
- `actor`: typed actor ref responsible where applicable.
- `component`: harness component that emitted the event.
- `payload`: event-specific structured data.

Useful optional semantics:

- `governable_context`: typed object ref for the primary context of the event.
- `causation_event_id`: event that directly caused this event.
- `correlation_id`: groups related events across runs or tools.
- `visibility`: who can see this event.
- `information_class`: `ordinary` or `private`; secret values are invalid event
  payloads.
- `content_hash`: integrity reference for payload or artifact.

## Event categories

### Run lifecycle

| Event | Meaning |
| --- | --- |
| `run.started` | Run envelope created |
| `run.queued` | Run is available for a worker |
| `run.leased` | Worker claimed the run |
| `run.lease_renewed` | Worker renewed lease |
| `run.paused` | Run paused intentionally |
| `run.resumed` | Run resumed after pause |
| `run.cancel_requested` | Cancellation was requested |
| `run.cancelled` | Run was cancelled |
| `run.completed` | Run completed successfully |
| `run.failed` | Run failed terminally |
| `run.retry_scheduled` | Retry was scheduled |
| `run.interrupted` | Worker or system interruption detected |

### Context

| Event | Meaning |
| --- | --- |
| `context.assembly_started` | Context gathering began |
| `context.source_considered` | Candidate context source evaluated |
| `context.source_included` | Source included in packet |
| `context.source_omitted` | Source omitted with reason |
| `context.packet_created` | Context packet created |
| `context.packet_shown_to_agent` | Packet or subset sent to agent executor |
| `context.compacted` | Context was summarized or reduced |

### Memory

| Event | Meaning |
| --- | --- |
| `memory.retrieval_requested` | Harness requested memory |
| `memory.retrieval_completed` | Memory runtime returned results |
| `memory.shown_to_agent` | Memory included in model-visible context |
| `memory.used_in_reply` | Memory influenced a reply |
| `memory.used_for_action` | Memory influenced an action |
| `memory.proposal_created` | Harness submitted memory proposal |
| `memory.proposal_rejected` | Proposal rejected before submission |

These run events complement, but do not replace, memory subsystem events.

### Model and agent execution

| Event | Meaning |
| --- | --- |
| `agent.step_started` | Agent execution step began |
| `model.requested` | Model provider request started |
| `model.completed` | Model provider request completed |
| `model.failed` | Model provider request failed |
| `agent.proposal_created` | Structured proposal produced |
| `agent.final_output_proposed` | Final output proposed before egress review |
| `agent.handoff_proposed` | Agent proposed handoff |

Events should not duplicate full prompts or completions when a governed source
already owns them. Private prompt/completion content may live behind an
authorized reference. Secret content must never be stored. Ordinary diagnostic
metadata should remain intact.

### Policy and approvals

| Event | Meaning |
| --- | --- |
| `policy.evaluation_requested` | Policy check requested |
| `policy.decision_recorded` | Policy returned allow, deny, or approval |
| `approval.requested` | Durable approval created |
| `approval.updated` | Approval changed, commented, or modified |
| `approval.granted` | Approval granted |
| `approval.denied` | Approval denied |
| `approval.expired` | Approval expired |
| `approval.revoked` | Prior approval revoked |

Policy events should include enough reason data to explain outcomes.

### Capabilities and tools

| Event | Meaning |
| --- | --- |
| `capability.invocation_proposed` | Agent proposed tool operation |
| `capability.schema_validated` | Input validated |
| `capability.invocation_blocked` | Policy or validation blocked invocation |
| `capability.invocation_started` | Adapter call started |
| `capability.invocation_completed` | Adapter call succeeded |
| `capability.invocation_failed` | Adapter call failed |
| `capability.result_normalized` | Output normalized and labeled |
| `capability.result_shown_to_agent` | Tool result returned to agent context |

### Egress

| Event | Meaning |
| --- | --- |
| `egress.review_requested` | Output/effect reviewed before leaving boundary |
| `egress.allowed` | Egress allowed |
| `egress.blocked` | Egress blocked |
| `egress.redacted` | Egress allowed after an explicit policy-selected derived redaction; the governed source is unchanged |
| `egress.sent` | External send/write/share occurred |
| `egress.failed` | External effect failed |

Egress events are security-critical.

### Artifacts

| Event | Meaning |
| --- | --- |
| `artifact.created` | Durable artifact created |
| `artifact.updated` | Artifact changed |
| `artifact.linked` | Artifact linked to object |
| `artifact.shown` | Artifact shown to human or agent |
| `artifact.exported` | Artifact exported |
| `artifact.deleted` | Artifact deleted or tombstoned |

### Worker and system

| Event | Meaning |
| --- | --- |
| `worker.started` | Worker process started |
| `worker.stopped` | Worker process stopped |
| `worker.heartbeat` | Worker heartbeat recorded |
| `worker.error` | Worker-level error |
| `system.recovery_started` | Recovery process began |
| `system.recovery_completed` | Recovery process completed |
| `system.projection_rebuilt` | Projection rebuilt from events |

## Projections

Projections are query-optimized views derived from the ledger.

Useful projections:

- `runs_current`: current status, owner, agent, trigger, timestamps.
- `run_timeline`: ordered display timeline.
- `run_errors_current`: latest blocking error.
- `approvals_current`: pending approval inbox.
- `capability_invocations_current`: tool call summaries.
- `memory_use_by_run`: memory retrieved, shown, and used.
- `external_effects_by_run`: sent or published outputs.
- `task_activity`: run activity by task.
- `agent_activity`: run activity by agent.
- `project_activity`: run activity by project.
- `security_events`: denials, approvals, egress events, revocations.

Projection rebuild should be possible from the event ledger.

## Ordering and concurrency

Within one run, events should have monotonic sequence numbers.

Across runs, ordering can use timestamps and correlation IDs, but global
ordering should not be required for correctness.

Child runs should reference parent runs. Related events should use correlation
IDs. Handoffs should preserve causation links.

If multiple workers can operate in the future, the run lease should prevent two
workers from executing the same run step at the same time.

## Idempotency

Events should support idempotent recovery.

For operations that may be retried, Noema should record:

- Idempotency key.
- Adapter request ID.
- External operation ID.
- Whether the effect is confirmed.
- Whether the effect outcome is unknown.
- Whether retry is safe.

If the harness cannot determine whether an external effect happened, recovery
should pause for inspection rather than duplicate the effect.

## Replay

Replay can mean different things:

- Reconstruct current projection.
- Recreate a human-readable timeline.
- Resume from last safe step.
- Re-run a deterministic context assembly.
- Re-run an agent step for debugging.
- Export a run record.

Not all replay is deterministic. Model calls, external services, and time can
change. The ledger should distinguish between replaying the record and
re-executing effects.

Re-executing external effects should require explicit approval unless the
operation is proven idempotent and policy allows it.

## Information handling

The event ledger should not become a secret dump.

Secret material is invalid in event payloads. For private payloads, events may
prefer the following when the full content is already owned elsewhere or does
not belong inline:

- Object references.
- Artifact references.
- Hashes.
- Governed summaries that preserve useful non-secret information.
- Information-class labels.
- Count and size metadata.
- Diff references behind access control.

Export applies scope, authorization, retention, deletion, and egress rules.
It never includes secrets. Private information is preserved when the export is
authorized; redacted derivatives are explicit export products, not the stored
default.

## Relationship to audit events

Run events are the detailed execution ledger.

Audit events are the cross-system compliance and inspection layer.

Some run events should also create or feed audit events, especially:

- Policy denials.
- Approval requests and decisions.
- External effects.
- Memory use for action.
- Capability invocations.
- Secret access.
- Revocations.
- Exports.
- Deletions.

The two systems can share storage or not, but the conceptual distinction is
useful:

- Run ledger: "What happened inside this run?"
- Audit log: "What important system actions happened across Noema?"

## Example timeline: conversation answer with memory

```text
run.started
run.queued
run.leased
context.assembly_started
memory.retrieval_requested
memory.retrieval_completed
context.source_included
context.packet_created
context.packet_shown_to_agent
agent.step_started
model.requested
model.completed
agent.final_output_proposed
memory.used_in_reply
egress.review_requested
egress.allowed
run.completed
```

## Example timeline: tool write requiring approval

```text
run.started
run.leased
context.packet_created
agent.step_started
capability.invocation_proposed
capability.schema_validated
policy.evaluation_requested
policy.decision_recorded
approval.requested
run.paused
approval.granted
run.resumed
policy.evaluation_requested
policy.decision_recorded
capability.invocation_started
capability.invocation_completed
capability.result_normalized
egress.sent
run.completed
```

## Example timeline: prompt injection blocked

```text
run.started
context.source_included
context.packet_created
agent.step_started
capability.invocation_proposed
policy.evaluation_requested
policy.decision_recorded
capability.invocation_blocked
egress.review_requested
egress.blocked
run.completed
```

The run may still complete with a safe explanation to the human.

## Retention

Retention should be configurable by scope and information class.

Possible retention policies:

- Keep full ledger.
- Keep private payloads behind governed references.
- Keep summaries only after a time window.
- Keep approval and external-effect events permanently.
- Delete model payload artifacts after a time window.
- Tombstone deleted objects.

Retention should never silently destroy the only record of an external effect
that still matters for audit.

## Initial event slice

Minimum useful initial-slice events:

- `run.started`
- `run.queued`
- `run.leased`
- `context.packet_created`
- `memory.retrieval_requested`
- `memory.retrieval_completed`
- `memory.shown_to_agent`
- `model.requested`
- `model.completed`
- `capability.invocation_proposed`
- `policy.decision_recorded`
- `approval.requested`
- `approval.granted`
- `approval.denied`
- `capability.invocation_started`
- `capability.invocation_completed`
- `egress.allowed`
- `egress.blocked`
- `memory.proposal_created`
- `run.completed`
- `run.failed`

This is enough to make the first harness inspectable without overbuilding the
entire north-star ledger.
