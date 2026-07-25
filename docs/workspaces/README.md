# Work Contract

This document records the current durable contract for Noema Work. It replaces
the completed design program and multi-agent implementation packets that
originally built the subsystem. Git history owns those execution details; new
work should follow the current code and this contract rather than reconstructing
the old horizontal program.

## Product boundary

Work is Noema's durable system for capturing, organizing, executing, reviewing,
and completing tasks. Chat is the simplest entry point, while `/work` provides a
denser management surface. Both operate on the same task and semantic command
model.

The current product has:

- one seeded Personal workspace and one seeded Personal workflow;
- optional projects that organize tasks without changing their execution;
- one canonical task object shared by chat, runtime, API, and UI;
- planner, executor, and reviewer runs supervised by the runtime;
- explicit human gates for clarification, approval, and recovery;
- immutable execution contracts, submissions, and reviews;
- a monotonic event ledger for audit and client invalidation.

Work is a general personal-task system. Repository scanning, branches,
worktrees, terminals, commits, and pull requests are adapter concerns rather
than core task concepts.

## State authority

Each question has one authority:

| Question | Authority |
| --- | --- |
| Where is a task in the workflow? | `tasks.stage_id` |
| What is an agent doing now? | Current `agent_runs.run_kind` and `agent_runs.status` |
| What governs this attempt? | Current immutable `task_execution_contracts` generation |
| Why does the human need to act? | The unresolved `task_gates` record |
| What completed successfully? | The latest immutable submission and approving independent review |
| What happened? | Monotonic `work_events` audit records |

Attention labels, valid actions, completion labels, and board groupings are
derived projections. They must not become mutable status fields or alternate
state machines. The event ledger is audit and invalidation data, not a replay
authority.

## Workflow

The Personal workflow maps six fixed behaviors to user-facing stages:

| Stage | Behavior | Meaning |
| --- | --- | --- |
| Inbox | `Intake` | Captured but not authorized to run |
| Queue | `Dispatch` | Authorized and waiting for the appropriate role |
| Doing | `Active` | Planning, execution, or automated review is active |
| Waiting | `HumanGate` | A structured human response is required |
| Done | `TerminalSuccess` | The reviewer approved the result |
| Cancelled | `TerminalCancelled` | The human cancelled the task |

Done and Cancelled remain available through history.
Stage behavior, rather than display text or English intent matching, controls
transitions and available operations.

The normal path is:

1. Capture creates an Inbox task. Queue authorizes planning, while primary-chat
   delegation may atomically capture and authorize a task.
2. A Planner produces a complete execution plan or opens a structured gate. A
   complete delegated intent may skip planning and queue an Executor directly.
3. The accepted plan becomes a new immutable execution contract. An Executor
   produces a submission or opens a gate.
4. A Reviewer independently evaluates the submission. Requested changes queue
   another bounded execution attempt; approval completes the task in Done.
5. The human may reopen Done with additional direction, which creates a new
   contract generation and queues another bounded execution attempt.

## Commands and transactions

Public mutations use semantic Work commands: capture, update Inbox, queue,
answer, retry, cancel, reopen, delegate, and the
project create/update/archive/reopen operations. Do not expose a generic
`set_stage` operation.

Task commands carry the expected task revision and execution generation.
Project commands carry the expected project revision. Idempotency keys,
correlation IDs, actor identity, and causation IDs are command metadata, not
resolver-local conventions.

The store command service owns each mutation transaction. A successful command
commits all of the following together when applicable:

- canonical task, gate, contract, run, submission, review, or project state;
- optimistic revision and generation changes;
- the audit event and required notification outbox records;
- the idempotency receipt and stable result.

An idempotent retry returns the original result. A stale revision, stale
generation, invalid stage, wrong gate, or superseded run fails without partial
effects. API resolvers, runtime workers, and tools must not assemble these
cross-table transitions themselves.

Cancellation and reopen increment the generation so old runnable work cannot
mutate the new task lifetime. Reopening Done creates an immutable amended
contract from the completed contract plus the human's required direction;
reopening Cancelled queues planning with that direction. Answer and retry
preserve the generation while resuming the role explicitly recorded by the gate.

## Runs, gates, and reconciliation

`RunKind` is Planner, Executor, or Reviewer. `RunStatus` is run-local queue and
lease state; it is never task workflow state. A run persists its exact provider
selection, execution policy, generation, contract lineage, parentage, attempt,
and review round before it becomes runnable.

Workers claim bounded leases, heartbeat while active, and publish only through
role-specific terminal contracts. Generation, lease token, run kind, and
contract lineage are rechecked when accepting terminal output. Provider calls,
tool calls, token counts, timing, and errors remain attributable to the run.

Human gates are typed records with one open gate at a time. Clarification and
approval gates resume the recorded role after a valid answer. Recovery gates
encode a closed recovery reason and an explicit safe continuation role when a
retry is allowed. Free-form text does not decide gate semantics.

Reconciliation derives one next action from durable facts. It may queue the
role compatible with the current contract, materialize a completed plan, move
an approved review to terminal Done, resume a resolved gate, open a recovery gate,
or fence stale runnable work. Contradictory state fails closed or opens an
invariant-recovery gate; it does not guess from event text.

An Active task whose current run is waiting for a governed action decision is
intentionally idle. The run resumes through the governed-action continuation;
reconciliation must not reinterpret that pause as missing durable work.

The essential compatibility rule is simple: Planner runs exist before a
contract, while Executor and Reviewer runs require a contract. Terminal and
human-gated tasks cannot retain runnable work.

## Persistence and events

SQLite is canonical and is opened only by the Noema server. Schema changes
append forward-only store migrations so persisted application rows survive
upgrades; applied migrations are immutable.

Work persists concrete workspace, project, workflow, stage, task, contract,
criterion, gate, run, run-item, submission, review, command-receipt, event, and
notification records. Foreign keys and unique indexes enforce identity and
lineage where SQLite can express them; command transactions enforce the
cross-record behavioral invariants.

`work_events` has a monotonic cursor and stable event identity. It supports
audit, subscriptions, and invalidation, but clients recover canonical state
through bounded reads after reconnect. Event payloads should identify affected
objects and facts needed by those consumers, without duplicating the full
aggregate.

Runtime queue claims use transactional leases. Expired or interrupted work is
reconciled against current generation and durable outputs before retrying, so a
crash cannot silently duplicate a completed terminal effect.

## Runtime and tools

The runtime owns worker supervision, provider dispatch, bounded context,
role-specific prompts and tools, lease renewal, and reconciliation scheduling.
The store remains the authority for admission and transition validity.

Task workers receive only the context needed for their role: the current task,
contract, relevant prior output, bounded transcript, provider snapshot, and
execution policy. They do not receive unrestricted primary-chat authority.
For chat-originated planning, the exact authenticated source request is shown
alongside the captured task description so the Planner can unfold necessary
work without silently expanding the requested outcome or delivery depth. The
resulting immutable contract remains the Executor's sole request authority;
contract complexity calibrates its research effort and user-facing detail.
For a simple contract, the default execution shape is one discovery batch and
at most one focused verification batch, stopping as soon as every criterion has
adequate evidence. Its user-facing result should normally stay below roughly
180 words when the human did not request depth; detailed validation belongs in
structured criterion evidence. Execution-policy values are safety ceilings for
runaway work, not effort targets, so they do not authorize broader research.

Reviewers are independent from Executors but bound to the submitted evidence:
the immutable contract, result, criterion evidence, and submitted artifacts.
Each Executor submission is a complete replacement deliverable. A revision run
may reuse relevant work and passed evidence from prior Executors, but its result
and artifact manifest must contain the full accepted work; prior submissions are
not inherited into the completed result. Criterion evidence locates support in
that deliverable and cannot substitute for required result or artifact content.
They may reject demonstrated omissions, internal contradictions, artifact
mismatches, or explicitly required missing evidence, but they cannot invent an
external contradiction from background knowledge or demand research dimensions
the criterion did not require. Reviewers receive no web tools and can read an
artifact only when the submission contains one.

Executor and Reviewer terminal schemas enumerate the current contract's exact
opaque criterion ids and require one entry for each id. The first malformed
terminal payload is returned to the same provider conversation for a
terminal-only repair; a second malformed payload fails non-retryably and opens
recovery instead of repeating the whole run.

Tool visibility follows capability and approval policy. The primary agent may
delegate a task through the semantic composition; task agents may publish only
the structured outputs allowed for their run kind. External writes remain
governed by the capability system and exact approval state.

Task progress and task-originated notices appear in the primary conversation as
agent-authored updates alongside a durable task attachment. The attachment is
stored as a `task_reference` conversation item containing only the task id;
GraphQL clients hydrate the current task projection and subscribe to Work events
so the card stays live. Detailed run transcripts remain attached to the task
and should not flood the main chat. A successful completion notice also attaches
every artifact from the accepted submission as a durable artifact reference.

## API and UI

GraphQL exposes bounded workspace, project, task-list, task-detail, transcript,
and overview reads; semantic mutations; and a cursor-based Work event
subscription. Inputs map to domain commands, and resolver projections derive
attention and valid actions from canonical store facts.

Clients must use generated GraphQL types and server-owned valid actions. Do not
mirror stage-transition rules in TypeScript. After reconnect, clients refetch
canonical reads and use the event cursor only to invalidate or advance them.

The primary chat shows compact task markers and human decisions when action is
needed. Its task-only overview is labeled Tasks; Work names the broader `/work`
surface containing a task-first operational queue, history, project
organization, and task detail. The queue leads with decisions that need the
human, then groups active work by the existing Active, Dispatch, and Intake
stage behaviors. Both surfaces reuse the same task-detail and decision
components.
While a task is active, task detail presents one padded chronological
conversation stream as the primary surface. After reviewer approval completes
the task, its body presents the accepted final response by default with the
chronological transcript available in a neighboring tab. The outer rail header
is reduced to floating cancel, Work, and close controls, while a compact
floating task card carries the title and info trigger. Durable human task input
uses the human message lane. Planner,
Executor, and Review runs are marked inline with role, revision, status, and
duration, while their persisted transcript items use the existing response and
activity lanes. Each immutable executor submission appears once as the durable
result in that chronological stream, followed by its artifact references. Local
artifact cards open the shared artifact detail viewer; downloading remains an
explicit action inside that viewer. Artifacts opened from a task are nested
detail routes with a Back action that restores the mounted task view. Run
transcripts are merged into one scroll surface; tool activity remains
expandable in place, and bounded paging continues from the oldest available run
window. Executor commentary before a non-terminal tool batch is user-visible;
hidden provider reasoning is not rendered.

The synthesized initial task message uses the captured task description, never
the Planner's normalized execution contract. Contract request, plan, and
criteria remain execution authority and are inspectable through run disclosure
without being presented as human-authored input.

The floating task-context card keeps an optional needs-input row and compact
validation summary above the active-task transcript, and remains available on
the Transcript tab for completed tasks; the Result tab omits it. The info
popover exposes compact metadata only; the validation row discloses individual criteria. An
unresolved clarification, approval, recovery, or permission
action is attached to the needs-input row, keeping its prompt and controls
visible until resolved; after resolution, the decision is represented by the
chronological task stream. Recovery uses one response control: non-empty text
resolves an eligible Answer, while an empty response requests Retry when the
gate authorizes it.
Internal IDs, raw run counters, provider details, and audit evidence stay behind
progressive disclosure unless they directly explain the next human action.

UI hierarchy and behavior follow `docs/frontend/product-design.md`. Backend
field availability alone is not a reason to display a field, and controls stay
hidden until their mutation is implemented.

## Code ownership

The current implementation is organized by responsibility:

- `crates/noema-workspaces` owns workspace and project domain records;
- `crates/noema-tasks` owns Work task, workflow, command, planning, gate, run,
  event, contract, submission, and review vocabulary;
- Work modules in `crates/noema-store` own SQLite commands, reads, leases,
  reconciliation, events, and notifications;
- Work modules in `crates/noema-runtime` own supervised task execution and
  runtime tools;
- task modules in `crates/noema-api` own GraphQL projections and resolvers;
- `apps/web` owns chat and `/work` presentation using generated contracts.

These are responsibility boundaries, not a mandate to split feature work
horizontally. New behavior should be implemented as one small vertical slice,
reuse the existing command path, and follow `docs/development/simplicity.md`.

## Validation and deferred scope

Tests should concentrate on transition authority, transactional atomicity,
idempotency, revision/generation fencing, lease races, recovery decisions,
provider/tool boundaries, and demonstrated regressions. Pass-through mappings,
enum mirrors, and the same transition repeated at every layer do not need
separate tests.

The following remain outside the current contract until a concrete product
slice requires them:

- multiple user-configurable workflows or workspace administration;
- dependency graphs, recurring schedules, collaborative assignment, and
  multi-user permissions;
- generic event replay or event-sourced aggregate reconstruction;
- coding-specific Git, terminal, worktree, and pull-request concepts in core;
- arbitrary drag-and-drop stage mutation;
- compatibility machinery for obsolete pre-V1 schemas or APIs.

When behavior changes, update this contract only for durable product or
authority decisions. Keep implementation plans short, remove completed packets,
and rely on Git history for execution detail.
