# Tasks

This document defines the current durable behavior for Noema Tasks.
Git owns completed implementation history.

## Product boundary

Tasks captures, organizes, executes, reviews, and completes durable work.
Chat is the simplest entry point.
`/tasks` is the management surface.

The current product has:

- one seeded Personal workspace and workflow;
- optional projects with shared files and resources;
- one task record across chat, runtime, API, and UI;
- Planner, Executor, and Reviewer runs;
- explicit human gates for clarification, approval, and recovery;
- mutable Task files for content and role handoffs;
- a monotonic event ledger for audit and client invalidation.

Tasks is a general personal-task system.
Repository tools and delivery systems remain capability concerns.

## State and content authority

Each question has one authority:

| Question | Authority |
| --- | --- |
| Where is the Task in its workflow? | `tasks.stage_id` |
| What is an agent doing now? | Current run kind and status |
| What work must agents perform? | Current `TASK.md` |
| What result did the Executor submit? | Current `RESULT.md` |
| What did the Reviewer decide? | Current review decision metadata |
| What feedback must the Executor address? | Current `REVIEW.md` |
| Why must the human act? | The unresolved Task gate |
| What happened? | Monotonic `work_events` records |

Task content is not a database snapshot.
Noema does not version plans, results, reviews, or context content.
Derived labels and valid actions must not become another state machine.

## Task files and directories

Each Task has one working directory.

- A project Task uses `<project>/<task-slug>/`.
- A projectless Task uses `${NOEMA_HOME}/tasks/<task-slug>/`.
- An explicit Task directory base remains supported.

Noema allocates the slug once.
It adds an integer suffix when a path exists.
A title change does not move the directory.

`TASK.md` contains the current request, plan, working notes, progress, and open questions.
`RESULT.md` contains the current submitted result when one exists.
`REVIEW.md` contains the current Reviewer feedback when feedback exists.
Agents can create other support files when useful.

`RESULT.md` stores web sources as `[^noema-source-N]` markers and matching
Markdown footnote definitions. The Runtime resolves provider-private markers
before an Executor terminal commits. This format keeps citations with the
mutable result during continuation, review, correction, and reopening.

```markdown
Supported claim.[^noema-source-1]

[^noema-source-1]: [Source title](<https://example.com/source>)
```

Titles must contain text. Source URLs must use HTTP or HTTPS. The Runtime
numbers unique URLs by first use and writes definitions after one blank line.

Project Tasks can read shared files within the project boundary.
Relative parent paths can reach those files.
Paths cannot escape the project boundary.
Projectless Tasks cannot escape their Task directory.
Symbolic links cannot bypass either boundary.

The Planner and Executor can manage files in the Task directory.
Task file tools cannot delete `TASK.md` or `RESULT.md`.
The Planner and Reviewer can read files within the project boundary.
The Reviewer cannot directly change Task or project files.
`task.finish_review` owns each `REVIEW.md` replacement.
Governed capability tools own Executor writes outside the Task directory.

Task file tools accept relative paths and UTF-8 text.
Model-facing reads and writes have a 64 KiB limit.
Writes replace files atomically.
Task files have no database record, content hash, revision, or snapshot.

## Workflow

The Personal workflow maps fixed behaviors to user-facing stages:

| Stage | Behavior | Meaning |
| --- | --- | --- |
| Inbox | `Intake` | Captured but not authorized to run |
| Queue | `Dispatch` | Authorized and waiting for a role |
| Doing | `Active` | Planning, execution, or review is active |
| Waiting | `HumanGate` | A structured human response is required |
| Done | `TerminalSuccess` | The Reviewer approved the work |
| Cancelled | `TerminalCancelled` | The human cancelled the Task |

Stage behavior controls transitions and available operations.
Display text and free-form intent do not control them.

The normal path is:

1. Capture creates an Inbox Task.
2. Queue authorizes planning.
3. A complete delegated intent can queue execution directly.
4. The Planner updates `TASK.md` and calls `task.finish_planning`.
5. The Executor reads current Task files and performs the work.
6. The Executor writes `RESULT.md` and calls `task.finish_execution`.
7. The Reviewer checks `RESULT.md` against `TASK.md` and calls `task.finish_review`.
8. Requested changes queue another Executor, which replaces `RESULT.md`.
9. Approval completes the Task without copying its content.

The Executor can call `task.continue_execution` when more execution is useful.
That call queues another Executor without human action.
The Executor calls `task.report_blocked` when a specific human response can enable progress.
The Executor can submit a limitation report when the requested outcome is impossible for Noema.
Physical actions that require embodiment are obvious limitations.
A limitation report is also valid for a missing capability or enforced hard limit.
It is not valid when retry, continuation, human input, or another authorized approach can complete the Task.
The Reviewer approves an honest limitation report as a completed Task result.

Reopened Tasks reuse their current files.
Completed views read current `TASK.md` and optional `RESULT.md` separately.

## Runs, gates, and reconciliation

`RunKind` is Planner, Executor, or Reviewer.
`RunStatus` is queue and lease state for one run.
It is never Task workflow state.

Each run stores its provider selection, execution policy, generation, parentage, attempt, and review round.
Provider, agent, project, and filesystem settings resolve when each run starts.

Workers claim bounded leases and heartbeat while active.
Terminal tools verify the current generation, lease, and run kind.
Provider calls, tool calls, token counts, timing, and errors remain attributable to the run.

Human gates are typed records.
Only one gate can remain open for a Task.
Clarification and approval gates resume the recorded role.
Recovery gates record a closed reason and a safe continuation role.
Free-form text does not decide gate semantics.

Reconciliation derives one next action from current operational facts.
It can queue the required role, resume a gate, complete approval, or reject stale work.
Contradictory state fails closed or opens recovery.

An Active Task can wait for an action decision without runnable work.
The saved action continuation resumes that run.

Terminal and human-gated Tasks cannot retain runnable work.

## Context and compaction

The Planner reads current `TASK.md`.
The Executor reads current `TASK.md`, optional `RESULT.md`, and optional `REVIEW.md`.
The Reviewer reads current `TASK.md`, required `RESULT.md`, and optional `REVIEW.md`.
Role prompts must not prescribe batches, checklists, or document sections.

Each role receives a fresh current run clock as system context.
Scheduled Tasks use their schedule timezone.
Other Tasks use the source request timezone when one was captured.
The captured request date and time remain Task data for interpreting relative terms in the original request.
They never replace the current run clock.

Generic context compaction remains the only compaction system.
After compaction, the runtime reloads the same current files for the active role.
The runtime then applies normal context admission again.
Task content cannot override runtime policy or role permissions.

Large tool results are bounded before context admission.
Recoverable action-storage failures preserve provider continuation.

## Scheduling and recurrence

A one-time schedule extends the existing Task.
`scheduled_for` stores an exact UTC instant.
`schedule_time_zone` stores the authoring IANA zone.
The missed-run policy is `run_once` or `skip`.

A scheduled Task remains in Intake until its due transition.
Unschedule returns it to ordinary Inbox.
Reschedule replaces only future timing.

Repeat owns continuing authority after one occurrence starts.
The recurrence stores its current template, schedule, policies, lifecycle, revision, and next due time.
Each occurrence remains an ordinary Task with its own files, runs, gates, and transcript.
Template edits do not rewrite existing occurrences.

Occurrence rows resolve each local recurrence minute once.
Spring-forward gaps do not execute.
A repeated fall-back minute executes at most once.

`skip` records a missed slot.
`run_once` queues one representative occurrence before normal cadence resumes.
Overlap policies are `skip`, `queue_one`, and `allow`.

Run now is separate execution authority.
For a pending scheduled Task, it runs that same Task early.
For an established series, it creates one manual occurrence.

Due processing uses revision fences and one idempotent transaction.
The runtime maintains one dynamic deadline for the next Task or recurrence.

## Commands and persistence

Public mutations use semantic Task commands.
The API does not expose a generic `set_stage` operation.

Task commands carry the expected revision and generation.
Project commands carry the expected project revision.
Command metadata carries identity, correlation, causation, and idempotency data.

The store command service owns each mutation transaction.
A successful command commits applicable state, audit, notifications, and its idempotency receipt together.
An idempotent retry returns the original result.
Stale or invalid commands fail without partial effects.

Cancellation and reopen increment the generation.
Old work cannot change the new Task lifetime.
Answer and retry preserve the generation and resume the recorded role.

SQLite stores operational Task state only.
This includes workflow, gates, runs, run items, decisions, receipts, events, and notifications.
Task content remains in Task files.

`work_events` provides audit, subscription cursors, and invalidation.
It is not a replay authority.
Clients recover current state through bounded reads.

## Runtime and tools

The runtime owns worker supervision, provider dispatch, bounded context, role tools, leases, and reconciliation scheduling.
The store owns admission and transition validity.

Task agents receive only their role context and authorized tools.
They do not receive unrestricted primary-chat authority.
Agents decide how to organize long work and whether support files are useful.

Task file tools are:

- `task.files.list`;
- `task.files.read`;
- `task.files.write`;
- `task.files.delete`.

Role terminal tools are:

- `task.finish_planning`;
- `task.finish_execution`;
- `task.continue_execution`;
- `task.finish_review`;
- `task.report_blocked`.

External writes remain governed by capability and approval policy.
ACP and provider agents receive equivalent Task-file behavior.

Task updates appear in the primary conversation with a durable Task reference.
The reference stores only the Task identity.
Clients hydrate its current projection and use Task events for invalidation.

Detailed run transcripts remain attached to the Task.
They must not flood primary chat.
Completion notices read the current Task files.

## API and clients

GraphQL exposes bounded Task reads, operational runs, transcript activity, semantic mutations, and a cursor-based event subscription.
Task detail exposes current `TASK.md`, optional `RESULT.md`, and optional `REVIEW.md` content.
It does not expose removed content histories or snapshots.

Clients use generated GraphQL types and server-owned valid actions.
They do not mirror stage transitions.
After reconnect, clients refetch current state and use events for invalidation.

The Task surface remains task-first and dense.
Task detail shows `Result`, `Task`, and `Transcript` in that order when a result exists.
It omits `Result` when `RESULT.md` is absent or blank.
An initial Task opening defaults to `Result` when available and to `Task` otherwise.
Human decisions remain visible until resolution.
Internal operational data stays behind progressive disclosure.

UI hierarchy follows `docs/frontend/product-design.md`.
Backend field availability alone does not justify display.

## Code ownership

- `crates/noema-workspaces` owns workspace and project records.
- `crates/noema-tasks` owns Task workflow, commands, gates, runs, and events.
- Task modules in `crates/noema-store` own persistence, transitions, leases, and reconciliation.
- Task modules in `crates/noema-runtime` own supervised execution and Task tools.
- Task modules in `crates/noema-api` own GraphQL projections and resolvers.
- `apps/web` and `apps/ios` own client presentation from generated contracts.

These boundaries do not require horizontal feature work.
Implement new behavior as one small vertical slice.

## Validation and deferred scope

Tests concentrate on transition authority, atomicity, idempotency, fencing, lease races, boundaries, and demonstrated regressions.
Do not repeat pass-through behavior at every layer.

The current scope excludes:

- multiple configurable workflows;
- workspace administration;
- collaboration and assignment;
- dependencies and subtasks;
- a Task file browser;
- checklist-specific state;
- Markdown parsing into a second progress model.
