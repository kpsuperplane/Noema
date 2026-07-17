# Future Roadmap — Extending Noema Work Without Splitting Its Model

**Status:** product sequencing contract
**Scope owner:** deferred expansion order, extension seams, and invariants that
survive each phase. The first-release boundary remains authoritative in
[00-product-contract.md](00-product-contract.md).

## Purpose and sequencing rule

This roadmap describes the order in which Noema Work may grow after the first
release. It is not permission to begin every feature in parallel. Each phase
must preserve the current task model, land its extension points deliberately,
and meet the preceding phase's operational guarantees before the next phase
becomes product work.

The order is intentional:

1. Make a task graph and controlled multi-agent work reliable.
2. Add personal planning and repeated work after task truth is reliable.
3. Add shared scopes and configurable workflow only after single-user behavior
   is stable.
4. Add richer project knowledge only after scope and governance rules are
   explicit.
5. Add external intake and optional domain adapters last, so integrations adapt
   to Work rather than defining its core.

## Invariants that every phase preserves

| Invariant | Reason it must survive |
| --- | --- |
| `tasks.stage_id` remains the one persisted task-level workflow state. | Relations, schedules, collaboration, and adapters may add facts and derived badges, but none may introduce a parallel lifecycle/execution phase. |
| Workflow behavior, not display text, controls runtime semantics. | Renaming or localizing a stage cannot change whether work may run, needs a human, accepts a result, or is terminal. |
| Semantic commands remain the only mutation path. | A new feature must add a command or extend an existing command; it cannot expose arbitrary stage assignment. |
| Contract versions, run records, submissions, reviews, gates, and messages stay immutable evidence. | New context or automation must not rewrite what an earlier worker was asked to do or what it produced. |
| Generation fencing protects task advancement. | A relation, scheduler, collaborator, or adapter cannot let old work complete after a request has changed, been cancelled, or reopened. |
| `work_events` remains the ordered Work ledger. | New activity and notification types use the same cursor/replay/subscription contract. |
| The server remains the source of truth. | Browsers, integrations, and agents use bounded server projections rather than local task state machines. |
| Capability and approval policy remain independent from workspace/project membership. | Collaboration and context must not become accidental external-write authority. |
| Completion remains a human acceptance decision. | No child task, schedule, collaboration signal, or adapter result may silently turn reviewer approval into Completed. |
| The global primary conversation remains unscoped unless the product explicitly adds a scoped conversation feature. | Project membership must never be inferred from a recent chat turn or a background event. |

## Cross-cutting extension protocol

Before a roadmap phase changes product code, its implementation proposal must
answer five questions:

1. **What new durable fact exists?** Define a concrete record owned by a
   semantic crate and its immutable/auditable fields. Do not hide the fact in a
   JSON payload or a browser cache.
2. **How does it affect dispatch?** State whether it changes only a derived
   eligibility decision, opens a HumanGate, or causes a semantic command. It
   may not add a hidden task stage.
3. **How does it fence stale work?** Specify whether the change increments a
   task generation, creates a new contract version, or is safely compatible
   with the current contract.
4. **How is it evented and presented?** Add a `work_events` kind, bounded
   GraphQL projection, cursor behavior, and notification rule. A new
   subscription channel is justified only if it is a filtered projection of
   the same ledger.
5. **What authority does it grant?** The default answer is “none.” If it needs
   memory, a tool, an external action, or an approval, it must integrate with
   the governance runtime through an explicit policy contract.

Every phase should add its tests first around these questions. New schema work
may follow Noema's pre-V1 direct-reset policy while that policy remains in
force, but no feature gets a compatibility facade merely to avoid making the
extension decision explicit.

## Phase 1 — Task relations and multi-agent orchestration

### Goal

Allow a task to express durable relationships to other tasks and allow an
agent to coordinate bounded fan-out without making the board or task runtime
ambiguous.

### Product scope

This phase introduces:

- Parent/child task relationships.
- Blocking/dependency relationships.
- Cycle detection and clear invalid-relation errors.
- Dispatch eligibility that respects unsatisfied blocking relationships.
- Bounded fan-out and bounded recursive delegation.
- Durable child-result delivery to a coordinating task.
- A task-detail relationship view and derived dependency/fan-out activity.

A parent remains a normal task. If it coordinates children, it has its own
execution contract, events, and eventual human acceptance path; it is not a
special “project” object disguised as a task.

### Model and dispatch decisions

Relations are first-class rows such as `parent_child` and `blocks`, with stable
source and target task identities, creator, causation/correlation identifiers,
and timestamps. A relation never copies or owns either task's stage.

An unsatisfied prerequisite makes a Queue task **dispatch-ineligible**. The
task stays in Queue and surfaces a derived “blocked by” explanation; it does
not move to a new Blocked stage and does not acquire a second workflow field.
When the prerequisite reaches a compatible accepted outcome, the reconciler
observes the durable relation and ensures the now-eligible task's next run.

A coordinating task that has dispatched children remains Doing while automated
coordination is under way. “Waiting for children” is a derived condition from
relations and terminal child events, not a task stage. When a child concludes,
the system appends a durable task message or task event addressed to the
coordinator and queues its safe continuation. The coordinator still submits a
result for review; child completion alone cannot accept the parent.

### Boundaries and limits

Fan-out is a semantic command with an explicit parent, bounded child count,
maximum depth, aggregate active-run limit, and correlation ID. The runtime
must reject a request that would exceed any configured bound rather than
creating an unbounded agent tree. A child receives an explicit immutable
contract and only the required bounded context; it does not inherit its
parent's full transcript or capabilities by accident.

The first version must define how cancellation propagates:

- Cancelling a child informs its parent through durable evidence; it does not
  automatically cancel siblings or the parent.
- Cancelling a parent fences its coordinating run and prevents new children,
  while already-created children require an explicit, visible propagation
  policy.
- A future “cancel descendants” action must be a separate semantic command
  with a deterministic target set and event per affected task.

### Extension points

| Layer | Extension |
| --- | --- |
| Domain | Relation identity/type contracts, cycle-validation rules, fan-out command, derived dispatch-eligibility input, child-conclusion message shape. |
| Store | Relation tables/indexes, atomic relation command transactions, efficient prerequisite queries, relation-aware reconciliation reads. |
| Runtime | Bounded child delegation policy, parent continuation scheduling, aggregation context, cancellation handling. |
| API | Relation queries, semantic relation/fan-out mutations, relation-filtered Activity events. |
| Web | Relationship graph/list in task detail, blocked-by explanation, child progress summary, no drag-based dependency semantics. |

### Phase exit criteria

The phase is ready only when cycle creation fails atomically, a prerequisite
cannot be bypassed after restart, duplicate child conclusion delivery is
idempotent, depth/run bounds hold under concurrent fan-out, and a stale
coordinator cannot advance a parent after Request Changes, Cancel, or Reopen.

## Phase 2 — Personal planning, scheduling, and recurrence

### Goal

Make Work useful for planned and repeated personal activity without turning a
task stage into a calendar state.

### Product scope

This phase may add priority, labels, dates, reminders, recurrence templates,
recurrence instances, a scheduler, and calendar/timeline representations.
These are planning attributes and dispatch inputs; they do not create a
“scheduled,” “overdue,” “snoozed,” or “blocked” task stage.

### Model and dispatch decisions

Priority, labels, and dates are durable task metadata with actor and event
history. They are independently editable under explicit optimistic revision
rules. A due date affects sorting, attention, and reminder policy; it does not
authorize an agent or make a task terminal.

A recurrence rule belongs to a recurrence template, not to one continuously
mutating task. When a scheduled occurrence becomes due, the scheduler
idempotently creates a new task instance with provenance pointing to the
template and a deterministic occurrence key. Each instance gets its own
contract, stage, generation, review, and acceptance; completing one occurrence
does not rewrite a prior one.

Scheduling affects Queue eligibility through a derived `not_before` condition.
A task can be in Queue but intentionally not dispatchable until its schedule
allows it. The board and detail show that condition as derived information;
they do not use a second status to represent it.

Reminders are generalized notification-outbox deliveries keyed by task,
reminder instance, and destination. A missed or duplicated scheduler wake must
not create duplicate tasks or repeated notices.

### Extension points

| Layer | Extension |
| --- | --- |
| Domain | Priority/label/date values, recurrence-template and occurrence identities, scheduling policy, reminder commands. |
| Store | Metadata/template/occurrence/reminder tables, deterministic occurrence uniqueness, due-task indexes, event/outbox writes. |
| Runtime | Supervised scheduler, idempotent clock/recovery scan, dispatch-eligibility integration, notification delivery. |
| API | Filter/sort inputs, recurrence and reminder mutations, calendar/timeline projections. |
| Web | Planning controls, date-aware List/Board filters, calendar/timeline views, accessible reminder and recurrence editing. |

### Phase exit criteria

The scheduler must tolerate a clock jump, restart, duplicate wakeup, and
delayed delivery without creating duplicate occurrences. Editing a recurrence
rule must affect only future occurrence creation under a documented boundary,
and every generated instance must remain independently reviewable and
acceptance-gated.

## Phase 3 — Multiple workspaces, collaboration, and workflow customization

### Goal

Expose the workspace model to multiple people and let workspaces tune
organization without weakening Noema's deterministic behavior and authority
boundaries.

### Product scope

This phase introduces additional workspaces, memberships, collaboration
surfaces, workspace/project access presentation, workflow editing/remapping,
and optional project-specific agent defaults. It may add human actors to task
discussion and work ownership only after the product separately defines how a
human assignment differs from agent execution.

### Workspace and collaboration decisions

Membership answers who can view or participate in a workspace. It does not by
itself grant an agent a capability, share an external account, provide memory
access, or authorize an egress destination. Those effects remain governed by
the existing capability and approval systems and must be configured
explicitly.

Collaboration commands carry actor identity, expected revision/generation,
causation, and correlation like every other Work command. Concurrent human
edits return stable stale-revision errors and require a refreshed projection;
the client must not overwrite another person's task message, gate answer, or
acceptance decision optimistically.

### Workflow customization decisions

Workflow customization is data-driven but constrained. A workspace may rename,
reorder, or add stage definitions only when every stage declares a stable
system behavior and valid semantic-command policy. The runtime still decides
dispatch, active work, human gates, acceptance, and terminal behavior from
that behavior contract, never from a user-entered label.

Changing a workflow with existing tasks requires an explicit, audited remap
plan. The plan maps every old stage to a compatible new stage behavior or
rejects the change; it does not silently reinterpret active work. The first
customization release should prefer a small set of behavior-preserving edits
before allowing arbitrary topology changes.

### Project-specific defaults

Project-specific agent/model defaults are defaults only. When a task is
queued, its resolved selection is copied into the immutable contract snapshot.
Changing the project default affects future contracts and cannot retarget a
running task. Project defaults never grant capabilities or bypass approvals.

### Extension points

| Layer | Extension |
| --- | --- |
| Domain | Membership/collaboration actors, workflow-version/remap contracts, stage-behavior validation, project default-selection policy. |
| Store | Multi-workspace indexes and authorization reads, workflow version/remap persistence, collaboration event writes. |
| Runtime | Resolved default snapshots, actor-aware notifications, no membership-to-capability shortcut. |
| API | Workspace selector/projections, collaboration mutations, workflow-editor validation/errors, actor attribution. |
| Web | Workspace switcher, membership surfaces, shared activity, conflict UI, workflow editor with compatible-remap guidance. |

### Phase exit criteria

The phase is ready only when a collaborator cannot gain a tool or memory
capability through membership alone, a workflow label change cannot change
runtime meaning, active tasks retain deterministic semantics across a remap,
and a project-default change cannot alter an already-created contract.

## Phase 4 — Project context hubs and durable knowledge

### Goal

Let projects become useful knowledge and coordination hubs while keeping
context ownership, agent authority, and task evidence legible.

### Product scope

This phase introduces project documents, scoped artifacts and memory,
milestones, decisions, risks, constraints, and open loops. It may add a project
overview that organizes those objects around tasks and activity.

### Context and governance decisions

Every project-owned object has explicit ownership, provenance, retention, and
access semantics. A task may reference project objects through a bounded
context manifest, then snapshot the selected material into its execution
contract. The system must say which document version, artifact version,
decision, or memory was included and which was deliberately omitted.

Project memory is a retrieval source only when a task's contract or explicit
context policy requests it. It is not injected because a task happens to be
linked to a project. Retrieved memory remains source-attributed and governed
by the same privacy and visibility rules as the rest of Noema.

Project documents and artifacts may enable a capability only through a
separate explicit grant. A document that says “send this” is untrusted content,
not authorization for the task to send it.

### Extension points

| Layer | Extension |
| --- | --- |
| Domain | Project document/artifact/milestone/decision/open-loop identities, context-manifest references, provenance contracts. |
| Store | Project-owned metadata/version tables, context-manifest persistence, bounded retrieval indexes. |
| Runtime | Context selection and snapshotting, source-attributed memory/artifact assembly, capability-policy separation. |
| API | Project overview, object drill-ins, context-manifest inspection, provenance links. |
| Web | Project hub, linked-object navigation, decisions/open loops, task-context inspection without a hidden chat scope. |

### Phase exit criteria

The phase is ready only when a historical task reveals exactly which
project-owned context it received, a project-memory result cannot leak across
workspace boundaries, and no project document or membership fact can
implicitly authorize a tool action.

## Phase 5 — External intake and optional domain-specific adapters

### Goal

Let trusted external systems create or enrich Work without making a provider,
repository, or integration the definition of a task.

### Product scope

This phase may add governed intake from email, calendars, forms, issue
trackers, and other integrations. It may also add optional domain-specific
execution adapters, including a coding adapter that can use repositories,
worktrees, branches, terminals, diffs, or pull requests.

### Intake decisions

External data arrives with provenance and trust classification. It may create
an Inbox task, propose a Queue task for human confirmation, append a task
message, or surface a Needs You item through an explicit semantic command.
It may not set a task stage directly or silently authorize execution merely
because an external system sent an event.

Deduplication keys are owned by the intake connector and persist with the
resulting Work command. Retries, webhook redelivery, and replay must converge
on one task/message/event outcome. External content is treated as untrusted
input for planner/executor/reviewer prompts.

### Domain-adapter decisions

A domain adapter consumes a task execution contract and produces ordinary
run/transcript/submission/review evidence. It may add adapter-specific
artifacts and capability checks, but it cannot add a parallel task stage or
replace human acceptance. Coding concepts remain adapter data; the general
Work Board does not require a repository, branch, terminal, worktree, diff, or
pull request to render a task.

### Extension points

| Layer | Extension |
| --- | --- |
| Domain | Intake provenance, connector idempotency identity, adapter capability and evidence contracts. |
| Store | Intake deduplication records, adapter-specific artifact links, event/outbox persistence. |
| Runtime | Connector supervision, trust classification, domain adapter invocation, governance and approval mediation. |
| API | Intake provenance, connector health, adapter evidence and safe action projections. |
| Web | Intake cards, provenance inspection, optional adapter-specific detail panels that remain subordinate to generic task detail. |

### Phase exit criteria

The phase is ready only when webhook replay cannot duplicate work, an
integration cannot self-authorize execution or external effects, and disabling
an adapter leaves the task's generic history, stage, result, and human
controls intact.

## What does not change as the roadmap grows

No later phase changes the meaning of the first release's core path:

~~~text
Capture -> Inbox
Queue -> Planner or Executor
Doing -> bounded automated work and review
Waiting -> a person must decide
Review -> reviewer-approved result awaits acceptance
Accept -> Completed
Cancel -> Cancelled
Reopen -> Inbox with preserved history and a new generation
~~~

The product can grow richer around that path, but it should never ask a person
to understand overlapping workflow, lifecycle, and execution-phase axes in
order to know whether their work is underway, needs them, or is finished.
