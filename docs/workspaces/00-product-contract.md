# Product Contract — Noema Work

**Status:** implementation contract
**Scope owner:** product behavior, user language, release boundary, and user
journeys. The exact Rust types live in
[02-domain-model.md](02-domain-model.md); persistence lives in
[03-storage-and-events.md](03-storage-and-events.md); command mechanics live in
[04-commands-and-reconciliation.md](04-commands-and-reconciliation.md).

## Product promise

Noema Work turns durable agent work into something a person can start in chat,
inspect in a dedicated surface, and deliberately finish. A task is one durable
object throughout that experience: the task mentioned in chat, the card shown
in Work, the record dispatched to an agent, and the history shown after it is
accepted are all the same task.

The first release is intentionally narrow. It gives one person a Personal
workspace, optional project containers, one primary conversation, and a
reliable path from a request to a reviewed result. It does not try to make
projects into mini-organizations or turn the board into a generic personal
productivity application.

## Settled release decisions

| Concern | Decision | Consequence |
| --- | --- | --- |
| Home surface | The global primary conversation remains Noema's home. | Work extends chat; it does not replace it with a project-scoped chat model. |
| Exposed workspace | The product exposes exactly one seeded workspace, **Personal**. | Storage models workspaces and memberships now, but there is no workspace switcher, sharing flow, or membership management in this release. |
| Project | A project is an optional named container for tasks. It has a name, a description, and an archive state. | A project provides organization and bounded descriptive context, not a separate agent, memory corpus, tool grant, or authority boundary. |
| Task owner | Every task is intended for agent execution. | A task can remain in Inbox without a run, but Queue authorizes Noema to plan or execute it. There are no human-assigned work items in this release. |
| Completion | A reviewer approves the work; a human accepts it. | Reviewer approval moves a task to Review. Only **Accept** moves it to Completed. |
| Control model | People use semantic actions, never an unrestricted stage picker. | There is no drag-and-drop stage change, generic mutation, or “set status” API. |
| Durable history | Task, run, review, gate, artifact, and activity history are retained. | Cancel and Reopen are historical actions, not destructive reset operations. |
| Notifications | Chat receives only attention-worthy task cards and completion notices. | Routine planner, executor, and reviewer progress belongs in Work. |

## One task stage, with live machinery kept separate

`tasks.stage_id` is the sole persisted, task-level workflow state. It answers
the product question “where is this work now?” and is the only state used to
place a task in a Work view.

The task does **not** persist another lifecycle, execution phase, worker state,
or duplicated status enum. Current activity is a projection over run records;
for example, “planner running,” “executor queued,” and “reviewer running” are
derived from the active `agent_runs` record. A gate, latest review, and
execution contract likewise answer different questions without becoming
parallel task states.

| Stage | Stable behavior | Meaning to the person | Allowed next actions |
| --- | --- | --- | --- |
| Inbox | `Intake` | Captured work that has not been authorized to run. | Edit, Queue, Cancel |
| Queue | `Dispatch` | Work is authorized and waiting for the planner or executor. | Cancel |
| Doing | `Active` | Noema is planning, executing, reviewing, or making an automated revision. | Cancel |
| Waiting | `HumanGate` | Noema needs a clarification, approval, or recovery decision. | Answer, Retry when offered, Cancel |
| Review | `Acceptance` | A reviewer approved a result and a human must decide whether to accept it. | Accept, Request Changes, Cancel |
| Completed | `TerminalSuccess` | The human accepted the reviewed result. | Reopen |
| Cancelled | `TerminalCancelled` | Work was intentionally stopped. | Reopen |

The labels are seeded display names, while their behaviors are stable system
semantics. Runtime logic must resolve the behavior from the stage definition,
not compare a display string such as “Doing” or “Review.”

The five active board columns are Inbox, Queue, Doing, Waiting, and Review.
Completed and Cancelled are history, shown in Completed rather than consuming
permanent board columns.

## The objects people see

### Personal workspace

Personal is implicit in the first release. Every task belongs to it, even when
the task has no project. The user should not have to select “Personal” before
capturing or delegating work.

The workspace exists as a durable identity because later sharing and
customization need a sound base. That modeling choice does not change the
current authority model: workspace membership is not a capability grant and
does not enable external writes, memory retrieval, or model selection.

### Projects

A project groups tasks whose relationship is useful to the person. A task may
belong directly to Personal or to one project; it cannot be in multiple
projects in this release. Project assignment is explicit: Noema never infers a
project from conversational recency, a file path, or a prior task.

Archiving a project hides it from default project filters and prevents it from
being selected for newly created tasks. It does not cancel, move, or rewrite
the stages of its existing tasks. Existing work remains inspectable and can
finish normally; Reopen makes the project available for new task assignment
again.

### Tasks

A task has a concise title, a fuller description, an optional project, durable
source/provenance, a stage, and derived execution information. The release
does not add user-maintained priority, date, label, owner, dependency,
recurrence, subtask, or assignee fields.

While a task is in Inbox, its title, description, and project can be edited
directly because there is no frozen execution request yet. After Queue, the
executed request must remain auditable. New clarification, correction, and
request-changes input becomes a durable task message and, where necessary, a
new immutable execution-contract version rather than silently rewriting
history.

### Execution contracts, runs, and evidence

An incomplete request may first receive one bounded Planner run with no
execution contract. The Planner cannot perform the work; it either supplies
the normalized request, validation criteria, and complexity needed to freeze
a contract or opens a human gate. Before an Executor or Reviewer acts, Noema
freezes an execution contract containing the immutable request, validation
criteria, selected models and policy, and workspace/project context snapshots.
A run is one bounded Planner, Executor, or Reviewer attempt. Submissions,
reviews, criterion evidence, artifacts, and transcripts remain immutable
evidence of what happened.

This separation is deliberate: the contract says what was requested, a run
says what one worker is doing, and the stage says where the task is in the
human-facing workflow.

## How chat decides between answering and creating work

The primary agent chooses among three structured actions under product policy:

| Choice | Use when | Durable result |
| --- | --- | --- |
| Foreground response | The request can be completed promptly and safely in the primary conversation. | No task is created. |
| Capture | The person asks to save, track, or revisit work without authorizing execution. | An Inbox task is created. |
| Delegate | The person explicitly asks Noema to do work asynchronously, or the primary agent judges the work likely to exceed roughly fifteen seconds. | A Queue task is created atomically with the information needed to start. |

The approximately-fifteen-second guide is a product policy given to the
primary model and its typed tools. It is not a phrase matcher, a literal timer,
or an English-language heuristic. The agent must make a structured choice, and
the server validates that choice against task, model-pool, and authority
requirements.

An explicit project reference is carried into the task. In every other case,
the task belongs directly to Personal; the primary conversation remains global
and is never implicitly scoped to a project.

## Human authority and attention

Noema may plan, execute, review, retry safe transient work, and make bounded
automated revisions while a task is Doing. It stops for a person when the
decision would alter the requested work, needs information only the person can
provide, would repeat an ambiguous external effect, or requires accepting a
result.

Waiting holds three gate kinds:

- **Clarification** asks for missing facts or an ambiguous choice.
- **Approval** asks for a decision required by the existing capability or
  governance policy.
- **Recovery** asks how to proceed after retry or revision limits are
  exhausted, or after an unsafe-to-replay failure.

Review is intentionally distinct from Waiting. It means a reviewer supplied a
complete passing review and the only remaining decision is whether the human
accepts the result. A request to change the result is durable feedback and
returns the task to Queue for a new execution-contract revision; it never
edits an approved submission in place.

## Work and chat together

Work is the operational surface for durable tasks:

- **Board** shows active work by its five active stages.
- **List** is a dense, searchable view of the same tasks.
- **Needs You** derives clarification, approval, recovery, and
  review-acceptance items from task records; it is not a separate inbox table.
- **Activity** replays the authoritative work-event ledger.
- **Completed** shows accepted and cancelled history, including Reopen.
- **Task detail** is the shared inspector for the task request, contract
  versions, gates, messages, runs, reviews, artifacts, result, and valid
  actions.

Chat uses the same task-detail component and shows compact cards only when a
task is created, needs a person, reaches Review, needs recovery direction, or
is accepted. A task card opens the same durable task detail that Work uses.
Chat must not duplicate planner/executor/reviewer transcripts or pretend that
an ephemeral progress message is task truth.

## Canonical user journeys

### 1. Complete a short request in the foreground

The person asks a question or requests a small action that the primary agent
can complete promptly. The primary agent responds in the current conversation
without creating a task, preserving chat's usefulness for ordinary
conversation. A person can still explicitly ask to track the work, in which
case the Capture path applies.

### 2. Capture something for later

The person says they want to remember or track a piece of work but does not
authorize action now. The primary agent creates an Inbox task with title,
description, provenance, Personal workspace, and an explicit project only if
one was named. Chat shows a creation card; Work shows the task in Inbox; no
planner or executor run exists.

The person may edit the Inbox task or press Queue from chat or Work. Queue is
the explicit authorization boundary.

### 3. Delegate a substantive request

The person explicitly delegates work, or the primary agent chooses delegation
under the structured long-work policy. Noema validates the selected executor
configuration and atomically creates a Queue task, its initial immutable
contract when sufficient input exists, its provenance, and the event needed by
the dispatcher. The primary conversation acknowledges the handoff once; it
does not also perform a competing foreground implementation.

### 4. Plan an incomplete request

When the delegated request does not yet form a complete execution contract,
Noema dispatches a planner using the existing task-executor identity and
enabled executor model pool. The planner either supplies a bounded plan that
freezes a contract and continues automatically, or reports a concrete
blocking question. A blocking question opens a Clarification gate, moves the
task to Waiting, and creates a chat card and Needs You item.

The person answers through the task card, Work detail, or Needs You. The
answer is recorded as a task message, the gate is resolved, and the task
returns to Queue. The answer is delivered only at a safe worker continuation
boundary.

### 5. Execute and revise under review

The dispatcher claims Queue work in order and moves it to Doing when the first
run is claimed. The executor works under the frozen contract and submits
evidence. Noema dispatches a separate reviewer, which may request bounded
automated changes while the task stays Doing. The board can show derived
activity such as “Reviewer running,” but Doing remains the only task stage
through the internal handoff.

### 6. Present an approved result for acceptance

When the reviewer approves every required criterion, Noema moves the task to
Review and makes the approved result and evidence visible. A Review card and a
Needs You item tell the person that a decision is ready. The task is not yet
complete, even if chat delivery is delayed or temporarily unavailable.

### 7. Accept or request changes

Accept marks the reviewed result as accepted, changes the stage to Completed,
and creates an accepted-completion event for chat delivery. Request Changes
always records the feedback, creates a new immutable contract version,
increments the fencing generation, and returns the task to Queue. Omitted
amendment fields copy their prior values; the feedback remains part of the new
contract's immutable input. The prior reviewer approval and submission stay
in history.

### 8. Resolve a recovery decision

If an infrastructure failure is retryable, Noema retries it under the existing
policy while the task remains Doing. If retries or review rounds are exhausted,
or an external effect cannot safely be replayed, Noema opens a Recovery gate
and moves the task to Waiting. The person sees the reason, the known facts,
and the safe choices. Retry or a new answer records the decision and returns
the task to Queue; Noema never guesses whether an ambiguous external effect
occurred.

### 9. Cancel and reopen without erasing history

Cancel is available from every nonterminal stage. It fences the task's active
runs, stops runnable work, records the action, and moves the task to
Cancelled. It does not delete contracts, messages, submissions, reviews,
artifacts, or events.

Reopen is available from Completed and Cancelled. It starts a new execution
cycle with a higher generation, clears only the current terminal pointers,
preserves all history, and returns the task to Inbox. Reopen deliberately does
not re-authorize execution; the person queues the reopened task when ready.

### 10. Move seamlessly between chat and Work

A person can open a task card in chat, a Board/List row, a Needs You item, an
Activity event, or a Completed item and reach the same task detail. The task's
stage, valid actions, attention, current run summary, active gate, and latest
review agree across every entry point because they are projections of one
server-owned task.

## Explicit first-release non-goals

The following are deferred even though the storage and terminology leave room
for them later:

- Multiple exposed workspaces, sharing, collaboration, and human assignees.
- Priority, labels, dates, reminders, recurrence, and calendar scheduling.
- Task dependencies, parent/subtask relationships, fan-out, and multi-agent
  orchestration beyond the planner/executor/reviewer path.
- Custom workflows, arbitrary stage editors, and freeform board moves.
- Project-scoped chats, project-specific agents, project-specific model pools,
  project memory retrieval, or project-specific tool grants.
- Project documents, milestones, decisions, risks, and open-loop management as
  first-class Work objects.
- Coding-specific concepts such as repositories, branches, worktrees,
  terminals, diffs, pull requests, and session resurrection.

These omissions keep the initial promise clear: Work governs durable
agent-executed tasks and their human decisions. The extension sequence and
invariants are defined in [09-future-roadmap.md](09-future-roadmap.md).

## Release acceptance

The release is product-complete when a person can capture, queue, inspect,
answer, retry, accept, request changes, cancel, and reopen the same task from
the supported chat and Work entry points, with a consistent stage and complete
history after a runtime restart. It is not acceptable to ship a board that
shows a second task status, lets a client assign arbitrary stages, or calls
reviewer approval “completed.”

The implementation and validation contract is in
[08-validation-and-rollout.md](08-validation-and-rollout.md).
