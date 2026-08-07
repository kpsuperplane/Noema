# Runtime Harness Architecture

Noema's runtime harness is the governed execution layer that turns an
incoming trigger into accountable agent work.

It is not the agent's personality, the model provider, the memory system, the
task system, or the dashboard. It is the shell around all of those pieces: the
place where context is assembled, permissions are enforced, tools are invoked,
approvals are persisted, memory use is recorded, artifacts are stored, and the
run ledger is written.

The harness exists because Noema should be able to answer a hard question after
every agent action:

```text
Who or what caused this to happen, under which scopes and permissions,
with which context, using which tools, sharing which information,
and with what durable record?
```

In current persistence, actor/principal and scope are behavior contracts on
concrete objects. Harness records should store typed object refs rather than
implying universal `principals` or `scopes` root tables.

## Reading map

This file defines the top-level architecture and vocabulary.

Detailed subdocuments:

- [Runtime architecture](harness/runtime.md): run envelopes, worker kernel,
  lifecycle, recovery, handoffs, scheduling, and execution.
- [Security model](harness/security.md): trust boundaries, ingress handling,
  egress protection, policy composition, durable approvals, and auditability.
- [Governed actions](harness/action-governance.md): action-centered LLM review,
  durable blocked actions, approval delivery, and observed URL matching.
- [Capability registry](harness/capabilities.md): tools, operations, adapters,
  contextual grants, schemas, and invocation lifecycle.
- [Event ledger](harness/events.md): append-only run events, projections,
  replay, recovery, privacy, and export.
- [Memory and context](harness/memory-context.md): context packets, memory
  retrieval, memory-use records, memory proposals, and provenance.

Related core docs:

- [Project overview](project.md)
- [Memory plan index](memory.md)
- [Retired Postgres schema](postgres.md)

## Architectural thesis

Noema should treat agent execution as a governed runtime, not as a free-form
chat loop with tools bolted on.

The harness should provide:

- A single run primitive for chat, tasks, schedules, proactive rules, imports,
  system maintenance, and agent handoffs.
- A durable ledger of run events instead of only mutable current state.
- A policy-enforced boundary between agent reasoning and external effects.
- A capability registry that makes tools explicit, typed, scoped, auditable,
  revocable, and explainable.
- A context assembly process that is repeatable enough to debug and flexible
  enough to support multiple humans, agents, projects, and workspaces.
- A memory boundary where the harness retrieves, uses, and proposes memory,
  while the memory subsystem owns durable memory truth.
- A personal-server worker kernel that can later grow into sandboxed, remote, or
  distributed execution without changing the run model.

The harness should be opinionated in construction and impartial in use. It
should make the common local single-human, single-agent case easy, but every
core abstraction should already fit Noema's north star: multiple humans,
multiple agents, shared workspaces, project-level context, delegated tasks,
explicit policies, approval gates, and exportable audit trails.

## What the harness owns

The harness owns runtime coordination.

It should own:

- Trigger normalization.
- Run creation and run status projection.
- Run envelope construction.
- Worker leasing and cancellation.
- Context packet assembly.
- Memory retrieval requests.
- Memory-use records.
- Memory proposal submission.
- Capability discovery and operation invocation.
- Tool input validation and output normalization.
- Policy checks before context use, tool invocation, memory use, and egress.
- Durable approval requests.
- Egress classification and enforcement.
- Artifact registration.
- Append-only run event writing.
- Recovery and replay workflows.
- Dashboard-facing explanations of what happened during a run.

The harness should not own:

- Long-term memory truth, promotion, or deletion decisions.
- The canonical semantics of tasks, projects, conversations, or workspaces.
- Model provider behavior.
- Tool-specific business logic.
- The user interface.
- Permanent derived indexes.

The harness coordinates these systems without absorbing them. This matters
because Noema should remain inspectable. A dashboard should be able to show
the run record, but the run record should point back to memory, task, tool,
conversation, and project objects rather than duplicating their authority.

## Core flow

```text
Trigger
  human message, command, task, cron, proactive rule, external event
        |
        v
Run Coordinator
  creates RunEnvelope, writes run.started, leases work
        |
        v
Context Runtime
  resolves scopes, retrieves memory, gathers task/project/conversation state
        |
        v
Governance Runtime
  checks policies, proactivity, data access, and egress limits
        |
        v
Agent Execution
  model/planner loop proposes replies, tool calls, memory changes, handoffs
        |
        v
Capability Gateway
  validates operations, gates approvals, invokes adapters, normalizes results
        |
        v
Output and Memory Mediation
  egress review, artifact registration, memory-use records, memory proposals
        |
        v
Event Ledger and Projections
  append-only events plus dashboard/task/current-state projections
```

The agent execution step is deliberately inside the harness, not above it.
Agents are powerful participants, but they should not be trusted to enforce
Noema's safety, privacy, permission, memory, or audit rules on their own.

## The run as the central primitive

A run is one attempt by one executing agent to do work under a specific
governed context.

Runs can be initiated by:

- A human conversation message.
- A command palette action.
- A task transition.
- A scheduled job.
- A proactive rule.
- An external event.
- A tool callback.
- A system maintenance job.
- Another agent handoff.
- Recovery from a previous interrupted run.

Noema should not create separate execution architectures for each trigger
type. Instead, every run should use a **scope envelope**.

The run envelope records:

- The trigger that started the run.
- The principal that requested or caused the run.
- The agent selected to execute the run.
- The owner object or governable home context for the run.
- The active human, workspace, project, conversation, task, relationship, tool,
  and custom scopes that are allowed to shape context and policy.
- Proactivity limits.
- Capability grants visible to this run.
- Memory access constraints.
- Egress constraints.
- Parent/child relationships to other runs.
- Links to durable objects such as conversations, tasks, projects, artifacts,
  and approval requests.

This model lets a chat run attach to a conversation, a cron run attach to a
proactive rule, and a task run attach to a task without forcing every run to
pretend it belongs first to chat or first to tasks.

## Core runtime objects

These are conceptual contracts. They do not have to become code types with
these exact names, but implementation should preserve the boundaries.

### RunEnvelope

The normalized execution envelope for a run.

It answers:

- What started this run?
- Who or what requested it?
- Which agent is executing it?
- Which scopes are active?
- Which object is the run primarily serving?
- Which policies and proactivity limits apply?
- Which capabilities may be considered?
- Which memory may be retrieved?
- Which external effects are allowed, blocked, or approval-gated?

### ContextPacket

The bounded, provenance-aware package of information supplied to the executing
agent.

It can include:

- Current user request or trigger payload.
- Relevant conversation transcript.
- Task state.
- Project state.
- Workspace policy.
- Agent instructions and skills.
- Retrieved memories.
- Documents or document excerpts.
- Tool result summaries.
- Capability summaries.
- Approval context.
- Security labels and trust annotations.
- Output requirements.

The context packet should have a manifest. The manifest should record what was
included, why it was included, where it came from, which information class it had, and
which policy allowed it to be used.

### Capability

A registered operation the harness can invoke through an adapter.

Capabilities include local tools, MCP tools, HTTP APIs, filesystem operations,
desktop actions, model-side tools, importers, exporters, search providers, and
internal Noema services.

Capabilities are not raw functions. They are governed resources with schemas,
operation classes, access grants, approval rules, audit requirements, and
egress classifications.

### PolicyDecision

The result of asking the governance runtime whether a proposed action is
allowed.

Policy decisions should be structured, not buried in prose. They should state:

- `allow`
- `deny`
- `require_approval`
- `require_more_context`
- `require_human_input`
- `require_safer_alternative`

They should also include the reason, the policies consulted, the scopes that
contributed, and any approval request or constraint that follows.

### ApprovalRequest

A durable object representing a blocked or pending action that requires human
or delegated approval.

Approvals should outlive the current chat turn. A run may pause on an
approval, another interface may approve it, and the worker may later resume
the run from the ledger.

### RunEvent

An append-only ledger event emitted by the harness.

Run events are the durable explanation of execution. They should be sufficient
to reconstruct what happened, project current run state, drive dashboard
surfaces, support recovery, and export an audit trail.

### MemoryUseRecord

A record that a specific memory was retrieved, shown to an agent, used in a
reply, or used to justify an action.

This record is separate from the memory itself. The memory subsystem owns
truth; the harness records runtime use.

### MemoryProposal

A structured suggestion that the memory subsystem create, update, confirm,
supersede, or archive a memory.

The harness may propose; the memory subsystem decides lifecycle and authority.

## Runtime boundaries

### Human interfaces

Dashboards, chat clients, command palettes, filesystem views, and mobile or
desktop clients create triggers and display projections. They should not
directly execute agent work.

Interfaces may:

- Submit triggers.
- Show run state.
- Present approvals.
- Render audit trails.
- Display context explanations.
- Let humans edit policies and grants.
- Let humans inspect memory proposals.

Interfaces should not:

- Bypass the harness to invoke tools.
- Create memory without provenance.
- Treat model output as approved external action.
- Mutate run state outside the ledger/projection system.

### Agent runtime

The agent runtime contains model adapters, prompts, planners, tool-selection
logic, response generation, and agent-specific behavior.

The agent runtime may propose:

- Replies.
- Tool invocations.
- Memory proposals.
- Task updates.
- Handoffs.
- Approvals.
- Clarifying questions.
- External actions.

The harness decides which proposals become effects.

### Governance runtime

The governance runtime evaluates access, proactivity, approvals, policy,
information class, trust labels, and egress constraints.

It should be deterministic wherever trust depends on it. A model can summarize
risk or recommend a policy outcome, but deterministic policy evaluation should
make the final enforcement decision.

### Context runtime

The context runtime gathers and packages the information available to a run.

It should preserve source references and trust labels. External content should
be marked as content, not instructions. Retrieved memory should carry
provenance, authority, confidence, information class, and allowed-use constraints.

### Capability gateway

The capability gateway mediates all tool calls.

It should validate schemas, apply policy, request approvals, invoke adapters,
normalize results, label result trust, store artifacts, and write ledger
events.

### Persistence layer

Canonical structured state lives in SQLite. Durable files live under the
object-owned filesystem hierarchy. Derived state lives under `system/`.

The harness should persist:

- Run records and current projections.
- Run event ledger entries.
- Approval requests.
- Capability invocation records.
- Artifact metadata.
- Memory-use records.
- Policy decisions and explanations where needed for auditability.

Forward-only SQLite migrations preserve the canonical database from the v9
baseline onward. Harness-specific tables should extend that source-of-truth
split through appended migrations rather than rewriting applied history.

## Egress protection as the primary safety model

The harness should screen and label untrusted information at ingress, but the
primary security model should protect egress.

Ingress controls reduce risk:

- Parse inputs through typed adapters.
- Label external content as untrusted.
- Preserve source provenance.
- Detect obvious prompt-injection patterns.
- Avoid treating external text as instructions.

Egress controls prevent harm:

- Classify every proposed output or side effect.
- Check whether scoped information is leaving its allowed boundary.
- Check whether the destination is allowed.
- Check whether the operation requires approval.
- Block unauthorized sharing or produce an explicitly policy-approved derived
  redaction without mutating the governed source.
- Persist approvals and denials.
- Audit all external effects.

This distinction is central. Noema should assume that some malicious or
misleading content will reach the model. The trusted harness should prevent
that content from causing unauthorized disclosure or action.

## Capability registry summary

Every tool or integration should enter the system through a registry.

A capability record should define:

- Stable capability ID.
- Human-readable name and description.
- Adapter type.
- Supported operations.
- Input and output schemas.
- Side-effect class.
- Egress class.
- Resource selectors.
- Required authentication.
- Supported actor objects and governable contexts.
- Default grants.
- Approval policy.
- Audit requirements.
- Rate limits and quotas.
- Revocation behavior.
- Data retention behavior.

Capabilities should be granted by context, not merely connected globally.

For example, a Google Docs capability exposed through MCP may be available to
one human, readable by one agent, restricted to a single document, writable only
inside a particular project, and approval-gated unless the run is serving a
specific pre-approved task.

## Memory integration summary

The harness should treat memory as governed context, not hidden model state.

During a run, the harness should:

- Request memory through the memory runtime using the run envelope.
- Include only allowed memories in the context packet.
- Mark memory provenance, participants, information class, confidence, and authority.
- Record which memories were retrieved.
- Record which memories were shown to the agent.
- Record which memories were used in a reply or action.
- Propose memory changes after relevant observations.

The harness should not:

- Promote candidate memories to confirmed status on its own.
- Delete memories on its own.
- Resolve contested memory truth on its own.
- Copy memories across scopes to make retrieval easier.
- Hide memory use from dashboard inspection.

## Ledger-first persistence

The harness should write append-only events for meaningful runtime transitions.

Examples:

```text
run.started
run.context_requested
run.context_assembled
memory.retrieved
memory.shown_to_agent
model.requested
model.completed
tool.invocation_requested
policy.decision_recorded
approval.requested
approval.granted
tool.invocation_started
tool.invocation_completed
egress.reviewed
memory.proposed
artifact.created
run.completed
```

Mutable state should be a projection, not the only record.

The dashboard can show a simple run status, but the source of truth should be
the sequence of events that produced that status. This supports debugging,
recovery, replay, audit, export, and user trust.

## Worker kernel summary

The initial slice should use a local worker kernel.

The worker kernel should:

- Lease queued runs.
- Load the latest run projection.
- Resume from the run ledger.
- Execute bounded steps.
- Respect cancellation and pause requests.
- Persist all meaningful transitions.
- Avoid keeping irreplaceable state only in memory.
- Treat adapters as replaceable execution backends.

The worker boundary should be stable enough that later versions can move
execution into a sandbox, another process, another machine, or a distributed
queue without changing the run envelope or event ledger.

## Dashboard implications

The harness should make these dashboard surfaces possible:

- Current runs.
- Run timeline.
- Run context manifest.
- Tool call history.
- Approval inbox.
- Policy decision explanations.
- Memory used by a run.
- Memory proposed by a run.
- External effects produced by a run.
- Failed and paused runs.
- Replay and recovery controls.
- Agent handoff graph.
- Capability access preview.
- Egress review history.

The dashboard should not need to infer trust-critical state from prose logs.
The harness should emit structured data that the dashboard can render.

## Initial implementation posture

The first implementation can be modest:

- One primary human.
- One primary agent.
- Embedded SurrealDB structured state.
- Local filesystem.
- Local worker loop.
- A small capability registry.
- Conversation-triggered runs.
- Manual approval gates.
- Memory retrieval and memory-use recording.
- Append-only run events.
- Basic dashboard or CLI inspection.

But the initial slice should not bake in assumptions that block:

- Multiple humans.
- Multiple agents.
- Workspace-shared policy.
- Project-scoped task execution.
- Proactive runs.
- Agent handoffs.
- Remote or sandboxed workers.
- Connector-specific resource grants.
- Contextual tool elevation.
- Durable approval workflows.
- Exportable audit trails.

The right initial slice is not a smaller architecture. It is a narrow vertical
slice of the full architecture.

## Non-goals

The harness architecture should not try to solve everything at once.

Non-goals for this doc set:

- Final database migrations.
- Final dashboard UI design.
- Final task schema.
- Final memory ranking algorithm.
- Final model provider abstraction.
- Final connector protocol.
- Final sandbox implementation.
- Final cryptographic storage design.

Those should be designed in later docs that reference this harness
architecture.

## Required invariants

The following invariants should hold across implementation phases:

- No external effect happens without a harness-mediated policy decision.
- No tool invocation bypasses the capability registry.
- No durable approval exists only in model context.
- No memory is shown to an agent without a retrievable grant path.
- No memory use is hidden from audit.
- No run relies on in-memory-only state for recovery.
- No untrusted external content is treated as system, developer, human, or
  policy instruction.
- No cross-scope data sharing is allowed merely because it appeared in context.
- No agent is the final authority on its own permissions.
- No derived index is the only source of structured truth.

These invariants are more important than any particular class name, table name,
or worker implementation.
