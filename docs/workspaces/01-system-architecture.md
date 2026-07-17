# System Architecture — Noema Work

**Status:** implementation contract
**Scope owner:** subsystem boundaries, authoritative data flow, consistency
rules, recovery, and authority separation. This document does not redefine
public record shapes or SQL columns; those belong to
[02-domain-model.md](02-domain-model.md) and
[03-storage-and-events.md](03-storage-and-events.md).

## Architectural intent

Work extends Noema's existing durable background-task runtime instead of adding
a second card system beside it. The server owns canonical state; chat and the
web client are two views over that state. A command changes a task only through
one transactional command path, and agents advance work through the same path
as humans rather than writing a task field directly.

The architecture keeps three questions deliberately separate:

| Question | Authoritative record | Why it is separate |
| --- | --- | --- |
| Where is the work in the human workflow? | `tasks.stage_id` | A task has exactly one persisted stage. |
| What is a worker doing right now? | Current `agent_runs` record and run-local status | Queueing, leasing, running, retrying, and cancellation belong to the run machinery. |
| What did Noema ask a worker to do? | Immutable task execution-contract version | A historical result must remain attributable to the request and policy that produced it. |
| Does a person need to decide something? | Active task gate or approved latest review | Attention is derived rather than copied into a second task state. |
| What happened and in what order? | `work_events` | The event ledger supports Activity, replay, subscriptions, notifications, and audit. |

The resulting invariant is simple: a task is in one stage, may have at most one
active run, and may have many historical contracts, runs, messages, reviews,
gates, artifacts, and events.

## Non-negotiable architecture rules

1. **No second task state axis.** There is no task lifecycle, execution phase,
   machine phase, or copied `TaskStatus` beside `stage_id`. UI chips such as
   “Planner running” are projections of run data.
2. **Commands are semantic.** Callers request Capture, Queue, Answer, Retry,
   Accept, Request Changes, Cancel, or Reopen. Neither GraphQL nor a tool may
   set an arbitrary stage identifier.
3. **SQLite mutations are transactional.** A command commits its task
   projection changes, related records, work events, queued/cancelled runs, and
   required notification outbox entries in one store transaction.
4. **Runs are fenced.** A run completion must match the task generation and
   execution-contract ID/version it was created for. A stale terminal attempt
   returns `run_fenced` and may be logged as a safe runtime diagnostic, but it
   cannot create durable task evidence or change the task.
5. **Recovery converges from durable facts.** The reconciler derives the next
   safe action from task, contract, gate, submission, review, and run records.
   It never parses an English response, checks for a process, reads an
   artifact, or inspects a Git/worktree state to infer task meaning.
6. **Project context is descriptive, not authority.** Workspace/project names
   and descriptions are snapshots in a contract context packet. Membership or
   project assignment never selects a model, retrieves scoped memory, grants a
   tool, or approves an external action.
7. **Events are durable, clients are disposable.** `work_events` is the
   replayable Work ledger. WebSocket/subscription state, chat cards, Board
   caches, and worker wake signals are all recoverable projections.

## Component boundaries

| Component | Owns | Must not own |
| --- | --- | --- |
| `crates/noema-workspaces` | Validated `WorkspaceId` and `ProjectId`, workspace/project records, membership and project validation contracts. | Task stage semantics, agent selection, memory retrieval, tool permissions, or external-action authority. |
| `crates/noema-tasks` | Workflow/stage domain contracts, task contracts, gates, messages, run kinds, event vocabulary, semantic command inputs/results, and pure transition/reconciliation decisions. | SQLite connections, GraphQL resolvers, chat presentation, or live worker supervision. |
| `crates/noema-store` | Schema/bootstrap, repositories, bounded read projections, the single transactional `WorkCommandService`, run claiming/lease persistence, event/outbox persistence, and idempotency storage. | Provider calls, prompt construction, tool execution, and browser-specific presentation. |
| `crates/noema-runtime` | Planner/executor/reviewer workers, lease renewal, worker supervision, idempotent reconciliation scheduling, typed task/project tools, context assembly, and notification delivery. | Direct task-stage SQL writes or a private alternate workflow. |
| `crates/noema-api` | GraphQL query projections, semantic mutations, error mapping, cursor subscriptions, and authorization of the API surface. | Business-state mutation outside `WorkCommandService` or handwritten frontend mirror types. |
| `apps/web` | Primary chat integration, Work routes and views, shared task detail, action controls, and subscription-driven refetching. | Client-owned task state machines, stage transitions, or independent task caches presented as truth. |
| Existing capability/governance runtime | Capability eligibility, approval, audit, side-effect control, and external-action policy. | Any project-membership shortcut that changes its authority decision. |

`noema-workspaces` remains a contract-focused crate in the first release.
`noema-tasks` is the canonical domain crate for workflows, tasks, execution
contracts, gates, messages, submissions, reviews, runs, and events; a task
does not move into a general-purpose “workspace work item” abstraction.

## Primary data flow

~~~mermaid
flowchart LR
    Chat["Primary Chat"] --> Tools["Typed Task/Project Tools"]
    WorkUI["Work UI"] --> GraphQL["GraphQL API"]
    Tools --> Commands["Semantic Work Commands"]
    GraphQL --> Commands
    Commands --> Store["SQLite Transactions"]
    Store --> Events["Work Event Ledger"]
    Store --> Runs["Queued Agent Runs"]
    Runs --> Worker["Planner / Executor / Reviewer"]
    Worker --> Commands
    Events --> GraphQL
    Events --> Outbox["Chat Notification Outbox"]
    Outbox --> Chat
~~~

There are two entry paths and one mutation path:

- The primary agent calls typed task/project tools. Trusted runtime context
  supplies the actor, source conversation, source turn, tool-call identity,
  and requested-by identity; the model does not manufacture those identifiers.
- Work calls GraphQL mutations. The API resolves the authenticated actor and
  forwards expected revision/generation and client mutation identity.
- Both paths invoke the same semantic command service. The command service
  validates current durable state, applies a pure domain decision, writes a
  projection update, appends events, and schedules durable follow-up work
  atomically.

Workers and the reconciler are command callers too. They never have a
privileged “write stage” route. A worker terminal contract yields a submission,
review, blocked condition, or run failure; the appropriate semantic command
decides whether the task stays Doing, moves to Waiting, moves to Review, or
queues a follow-up run.

## Canonical state and projections

The architecture uses a transactional projection model, not an event-sourced
rewrite. SQLite task/workspace/project rows are the canonical current state,
and `work_events` is an append-only ledger committed in the same transaction.
Events may rebuild read models or feed clients, but replaying an event must not
be required to find a task's current stage.

| Need | Read from | Projection rule |
| --- | --- | --- |
| Board/List | Tasks joined to workflow stages, optional projects, active gate/latest review/current run summary. | Bounded server query; no per-card browser fetch fan-out. |
| Valid action buttons | Task stage behavior plus gate/review/terminal facts. | Server derives actions; the client renders only offered actions. |
| Needs You | Waiting task with unresolved gate, or Review task with approved review awaiting acceptance. | Derived query, not a mutable attention queue. |
| Activity | `work_events` ordered by global monotonic cursor. | Cursor pagination and replay use one order for all Work views. |
| Current worker activity | Active `agent_runs` row. | Run kind/status is displayed separately from task stage. |
| Completion/recovery chat notice | Notification outbox keyed by durable work event and destination. | Delivery is retryable and idempotent; failure does not change a completed task. |

The store may create query-optimized SQL views or joins, but those are
implementation details. They must derive only from canonical rows and may be
rebuilt after a restart.

## Semantic command transaction boundary

~~~mermaid
sequenceDiagram
    participant Caller as Chat tool / GraphQL / worker
    participant Command as WorkCommandService
    participant Domain as noema-tasks decision
    participant DB as SQLite transaction
    participant Wake as runtime wake signal

    Caller->>Command: semantic command + actor + preconditions
    Command->>DB: read current task/project/workflow facts
    Command->>Domain: validate and decide transition
    Domain-->>Command: records to create/change + event kinds
    Command->>DB: update projections, runs, events, outbox, idempotency
    DB-->>Command: commit with event cursor
    Command-->>Caller: resulting task + cursor + valid actions
    Command-->>Wake: best-effort local notification
~~~

The local wake signal is intentionally outside the correctness boundary. A
missed signal is repaired by the reconciler's scan. Conversely, a wake may be
received more than once without creating duplicate work because run creation
and idempotency keys are persisted under the same transaction.

Each command carries the following cross-cutting metadata:

- An authenticated or trusted runtime actor.
- The expected task revision and generation whenever the command targets an
  existing task.
- A causation ID identifying the triggering action and a correlation ID that
  follows a user intent across its runs and notifications.
- An optional idempotency key. Chat delegation uses the source
  conversation/tool-call identity; GraphQL uses the client mutation identity.

The command result returns the updated task projection and the cursor of the
event it committed. A duplicated successful command returns that same durable
outcome rather than emitting another event or run.

## Execution topology

Queue authorizes execution. It does not mean every Queue task has an executor
immediately: a complete contract dispatches an Executor, while an incomplete
request dispatches a Planner. The planner reuses the task-executor identity and
the enabled executor model pools; there is no planner-specific model pool.

~~~mermaid
flowchart TD
    Queue["Task in Queue"] --> Contract{"Complete immutable contract?"}
    Contract -- no --> Plan["Queue Planner run"]
    Plan --> Question{"Planner blocks?"}
    Question -- yes --> Waiting["Open gate; stage Waiting"]
    Question -- no --> Freeze["Freeze contract version"]
    Freeze --> Executor["Queue Executor run"]
    Contract -- yes --> Executor
    Executor --> Submit["Immutable submission"]
    Submit --> Reviewer["Queue Reviewer run"]
    Reviewer --> Verdict{"Reviewer verdict"}
    Verdict -- changes --> AutoRevise["Queue Executor; stage Doing"]
    Verdict -- needs human --> Waiting
    Verdict -- approved --> Review["Stage Review"]
    Review --> Human{"Accept?"}
    Human -- accept --> Completed["Stage Completed"]
    Human -- changes --> HumanRevise["New contract revision; Queue"]
~~~

One task may have only one queued, leased, or running run, even while global
supervision permits up to eight leased/running task runs. The store enforces
both limits at claim/enqueue boundaries; the runtime does not rely on an
in-memory mutex for correctness.

The worker runtime retains the existing durable lease, continuation, transcript,
artifact, usage, progress-audit, capability, and terminal-contract machinery.
It changes the old task status handoffs into semantic commands and associates
each run with the task generation and contract version it is allowed to
advance.

## Restart and reconciliation

The runtime treats a restart as an ordinary recovery path. It does not try to
guess where a worker got to from a provider response or process list. On
startup and after each relevant Work event, reconciliation examines durable
facts and makes idempotent “ensure next action” decisions.

~~~mermaid
flowchart TD
    Start["Runtime starts or receives Work event"] --> Leases["Recover expired leases"]
    Leases --> Scan["Read candidate tasks and latest related records"]
    Scan --> Decide["Pure reconciliation decision"]
    Decide --> Idle{"Intentionally idle?"}
    Idle -- yes --> Done["No mutation"]
    Idle -- no --> Ensure["Idempotently ensure one required run or gate"]
    Ensure --> Commit["Store transaction + work event"]
    Commit --> Wake["Wake supervised workers"]
    Wake --> Start
~~~

The decision table is authoritative in
[04-commands-and-reconciliation.md](04-commands-and-reconciliation.md). At
this boundary the architecture requires these properties:

- A Queue task with no complete contract has exactly one eligible Planner run;
  one with a complete contract has exactly one eligible Executor run.
- Claiming the first run moves Queue to Doing in the same transactional claim.
- A successful planner, executor, or reviewer handoff creates the next
  durable unit of work before the current run can be considered settled.
- A resolved gate returns the task to Queue; no continuation reads uncommitted
  human input.
- A terminal stage is intentionally idle. The reconciler cannot resurrect it.

The reconciler can run repeatedly and concurrently with harmless duplicate
wakeups. It must not create more than one active run for a task, reopen a
resolved gate, or rewrite a contract. If stored facts are inconsistent, it
creates a safe Recovery gate through a command rather than selecting a
plausible state silently.

## Cancellation and generation fencing

Cancellation is an ordered transaction, not a best-effort process kill:

~~~mermaid
sequenceDiagram
    participant Human
    participant Command as CancelTask
    participant DB as SQLite
    participant Worker as active worker

    Human->>Command: Cancel(expected revision, generation)
    Command->>DB: generation += 1; cancel runnable runs
    Command->>DB: set stage Cancelled; append event
    DB-->>Command: committed
    Command-->>Worker: cancellation observed / lease no longer valid
    Worker->>Command: late success or failure for old generation
    Command->>DB: fenced terminal attempt
    DB-->>Command: reject all Work writes as stale
~~~

The command increments the task generation before a stale worker can
successfully advance the task. Workers check cancellation before each provider
request and capability effect, renew leases only while their generation is
current, and use their persisted task generation on every terminal call.

Cancellation never deletes evidence. It cancels queued/leased work where
possible, causes running work to stop at a safe boundary, and leaves a complete
event trail. A cancellation that races a reviewer approval deterministically
wins or loses by transaction ordering; whichever command commits second sees a
revision/generation mismatch and returns a stable stale-command error.

## Stale completions and request-change fencing

Generation fencing also protects Request Changes and Reopen. Both are semantic
boundaries that mean an older worker must not make a new request appear
complete. The historical run and its earlier transcript remain readable, but
a stale terminal call adds no submission, review, gate, transition,
current-result pointer, or notification.

~~~mermaid
flowchart LR
    Old["Run created at generation 12 / contract v3"] --> Work["Worker executes"]
    Change["Request Changes or Reopen"] --> Fence["Task generation becomes 13"]
    Fence --> New["New contract/run may be queued"]
    Work --> Finish["Old run attempts terminal command"]
    Finish --> Check{"Generation and contract match?"}
    Check -- yes --> Advance["Apply semantic outcome"]
    Check -- no --> Stale["Return run_fenced; optional runtime diagnostic only"]
~~~

The generation is a monotonic task fence, while a task revision protects
optimistic human/API edits. The implementation must not collapse the two:
revision identifies a conflicting current projection edit; generation rejects
work from an earlier execution authority.

## Event delivery to chat

`work_events` records the product fact. Notification delivery is a downstream,
retryable projection and cannot roll a task backward if it fails.

~~~mermaid
sequenceDiagram
    participant Command as Work command
    participant DB as SQLite transaction
    participant Outbox as notification worker
    participant Chat as global primary conversation

    Command->>DB: task/run/gate change + work event + outbox row
    DB-->>Command: one committed cursor
    Outbox->>DB: claim pending event/destination idempotently
    Outbox->>Chat: request compact task card or completion notice
    Chat->>DB: persist deterministic conversation item
    Outbox->>DB: mark delivered
    Note over Outbox,DB: retry after failure; duplicate key prevents duplicate card
~~~

The generalized outbox replaces a completion-only delivery path. It delivers
only creation, human-gate, Review-ready, recovery, and accepted-completion
notices. Routine activity remains in Work, so a long task does not flood the
primary conversation. The destination is the owning human's one global primary
conversation, resolved independently from task provenance; a Work-created task
therefore remains visible from chat when it needs attention. If the primary
conversation is temporarily unavailable, the outbox retries or records a
visible delivery problem without altering the underlying task stage.

## Work-event subscriptions and reconnects

The global `work_events` sequence is the cursor source for all Work
subscriptions. A task-specific subscription is a filtered projection of the
same ordered stream, never a separate event channel that can disagree with
Activity.

~~~mermaid
sequenceDiagram
    participant UI as Web client
    participant API as GraphQL API
    participant DB as work_events

    UI->>API: fetch bounded projection and latest cursor
    API->>DB: read tasks + cursor
    DB-->>API: projection
    API-->>UI: initial data + cursor
    UI->>API: subscribe workEvents(after: cursor)
    DB-->>API: later ordered events
    API-->>UI: event cursor + changed object references
    Note over UI: apply or refetch bounded projections
    UI->>API: reconnect with last durable cursor
    API->>DB: replay after cursor, then resume
~~~

Clients treat events as invalidation and ordered change hints, not a
client-owned reducer for task semantics. On a reconnect or a detected cursor
gap, the client refetches the affected bounded queries and task detail. This
keeps the server's current projection authoritative and avoids turning browser
state into a second workflow engine.

Cursor values are opaque, monotonic, scoped to the Personal workspace in this
release, and bounded by pagination limits. A client must not infer timestamps
or task ordering from a cursor string.

## Context, capabilities, and external authority

Task context assembly is intentionally additive and bounded:

1. The run receives the immutable request, criteria, selected model/policy
   snapshots, and relevant prior task evidence.
2. The run receives workspace/project name and description snapshots in
   separate labeled context sections.
3. The capability gateway independently determines which tools and external
   effects are allowed for the executing identity and current policy.

Project metadata cannot alter step 3. It cannot cause a task to retrieve
project memory, inherit a project agent, access a project filesystem, or
receive an external-write exception. Those future features require explicit
governance contracts, not a new implicit branch in the Work runtime.

Planner tools are deliberately narrower than executor tools: a Planner may
submit a plan or a blocking question, but has no general action tools.
Executor and reviewer tool restrictions continue to come from role-gated
capability policy. Review remains independently performed and read-only with
respect to task output.

## Claude-Kanban patterns: adopted boundary, rejected substrate

The local Claude-Kanban project informs operational shape, not Noema's domain.
Noema borrows its useful properties: one server-owned source of truth, semantic
commands across surfaces, append-only activity, durable inbox-style messages,
generation fencing, intent-plus-reconciliation recovery, attention queues, and
archive/reopen history.

Noema adapts a “card” into a general task and a live agent session into one
bounded agent run. It explicitly rejects worktrees, repositories, branches,
terminals, diffs, pull requests, tmux/session liveness, and coding-agent
assumptions as core Work concepts. A future coding adapter may consume the
same task/run contracts, but it must not reshape the general task model.

## Architecture acceptance checks

An implementation meets this architecture when:

- Every path that changes Work state calls the same command service.
- A task query exposes stage, current run, active gate, and latest review as
  separate fields and never exposes a second task-level execution state.
- Restart, duplicate delivery, duplicate mutation, and late worker completion
  converge without duplicate runs, notifications, or stage changes.
- A project can enrich a task context packet without changing agent identity,
  model selection, memory retrieval, capability eligibility, or approval
  authority.
- Chat and Work agree after reconnect because both read server-owned durable
  projections and the work-event ledger.

The concrete command and reconciliation cases are specified in
[04-commands-and-reconciliation.md](04-commands-and-reconciliation.md); the
end-to-end test matrix is in [08-validation-and-rollout.md](08-validation-and-rollout.md).
