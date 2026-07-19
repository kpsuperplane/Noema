# Noema Work: GraphQL Contract

**Status:** implementation specification
**Owns:** browser/client-visible Work reads, semantic mutations, errors, and
subscriptions
**Read with:** [the domain model](02-domain-model.md),
[storage and events](03-storage-and-events.md), [commands and reconciliation](04-commands-and-reconciliation.md),
and [the Work UI contract](07-work-ui.md)

The GraphQL API presents one task stage and several projections. It never
reintroduces a task `status`, `lifecycle`, or `executionPhase` field. A client
reads the persisted `stage`, then separately reads `currentRun`, `activeGate`,
`latestReview`, and derived `attention`.

## API conventions

Work is owner-authorized before any projection or subscription is opened.
Personal is the only exposed workspace in the first release, but APIs retain a
workspace id so later membership checks do not require a breaking shape. An
unavailable task or project returns the same safe unavailable response whether
it is missing or unauthorized.

All IDs and cursors are opaque strings. Clients must store and return them, not
parse their contents. All timestamps are RFC 3339 UTC strings. `JSON` event
payloads are safe audit metadata and are not a UI state model.

Every existing-object mutation accepts:

| Field | Rule |
| --- | --- |
| `expectedRevision` | Required and compared with the target task or project revision. |
| `expectedGeneration` | Required for task commands that can race a cancellation, revision, or reopen. |
| `clientMutationId` | Required for every mutation; it is the caller idempotency key and is returned unchanged. |

Create mutations have no expected revision or generation because they do not
target a pre-existing object. The server supplies actor, workspace membership,
causation id, correlation id, source conversation, and trusted idempotency
namespace. No client can write those authority-bearing values.

Connection arguments use `first` in the range 1–100, defaulting to 50. A
connection returns stable ordering and an opaque `endCursor`; `after` means the
next page in that ordering. Active task lists use `updatedAt DESC, taskId DESC`,
project lists use `updatedAt DESC, projectId DESC`, and terminal tasks use their
terminal timestamp then task ID descending. Activity queries return newest
events first and page toward older sequence numbers; event subscriptions replay
ascending sequence order. Run transcript pages are chronological within the
returned page and page backward into older records.

## Object types

### Workspace, project, and workflow

| Type | Required fields | Notes |
| --- | --- | --- |
| `Workspace` | `workspaceId`, `name`, `description`, `isPersonal`, `membershipRole` | The first release returns only `workspace:personal`. |
| `Project` | `projectId`, `workspaceId`, `name`, `description`, `revision`, `archivedAt`, `createdAt`, `updatedAt` | An archived project is still readable and may contain historic tasks. |
| `Workflow` | `workflowId`, `name`, `stages` | Workflows are data, but the first release exposes one seeded workflow. |
| `WorkflowStage` | `stageId`, `key`, `name`, `displayOrder`, `behavior` | `behavior` is a stable enum; UI display text is never runtime authority. |

`WorkflowStageBehavior` has exactly `INTAKE`, `DISPATCH`, `ACTIVE`,
`HUMAN_GATE`, `ACCEPTANCE`, `TERMINAL_SUCCESS`, and
`TERMINAL_CANCELLED`. The seeded stage keys are `inbox`, `queue`, `doing`,
`waiting`, `review`, `completed`, and `cancelled`.

### Task summaries and details

`TaskSummary` is the board/list payload. It contains:

| Field | Meaning |
| --- | --- |
| `taskId`, `workspace`, `project` | Stable placement. `project` is nullable. |
| `title`, `descriptionPreview` | Human-readable work, with a bounded preview rather than an unbounded contract. |
| `stage` | The only task-level state. |
| `revision`, `generation` | Concurrency values required by semantic actions. |
| `createdAt`, `updatedAt`, `completedAt` | Ordering and age. |
| `currentRun` | Nullable derived `CurrentRunSummary`. |
| `activeGate` | Nullable unresolved `TaskGate`. |
| `latestReview` | Nullable `TaskReviewSummary`. |
| `attention` | Zero or more derived `TaskAttention` values. |
| `validActions` | Server-authorized `ValidTaskAction` values for this exact projection. |

`TaskDetail` includes every `TaskSummary` field plus:

- full title and description;
- source provenance safe to show to the owner;
- `currentContract` and paginated immutable contract versions;
- paginated gates and human task messages;
- paginated run, submission, review, artifact, and activity evidence;
- `latestSubmission`, `latestReview`, and accepted result when present.

The detail object must not contain a duplicate task state such as
`executionStatus`. `CurrentRunSummary` is nullable and contains `runId`,
`kind`, `status`, `attemptIndex`, nullable `contractId`, `queuedAt`,
`startedAt`, `updatedAt`, and a safe activity label. Planner has no contract
id; Executor and Reviewer have one. Run `kind` is `PLANNER`, `EXECUTOR`, or
`REVIEWER`; run `status` stays run-local.

### Contract, gate, message, and evidence types

| Type | Required fields |
| --- | --- |
| `TaskExecutionContract` | `contractId`, `version`, `taskGeneration`, `supersedesContractId`, `origin`, `requestMarkdown`, nullable `executionPlanMarkdown`, `criteria`, `complexity`, executor/reviewer/policy snapshots, workspace/project context snapshots, `createdAt` |
| `TaskGate` | `gateId`, `taskGeneration`, `kind`, `state`, nullable `recoveryReason`, nullable `retryRunKind`, `prompt`, `contextMarkdown`, `openedBy`, `openedAt`, `resolvedBy`, `resolvedAt`, `resolution`; unresolved gates have null resolution fields |
| `TaskMessage` | `messageId`, `taskGeneration`, `kind`, `bodyMarkdown`, `author`, `gateId`, `contractId`, `approvalDecision`, `consumedByRunId`, `consumedAt`, `createdAt` |
| `TaskSubmission` | `submissionId`, `contractId`, `executorRunId`, `summary`, `resultMarkdown`, criterion evidence, linked artifact versions, `createdAt` |
| `TaskReview` | `reviewId`, `contractId`, `reviewerRunId`, `reviewedSubmissionId`, `reviewAttemptIndex`, nullable `supersedesReviewId`, `verdict`, `feedback`, criterion outcomes, `createdAt` |
| `TaskRun` | `runId`, `kind`, `status`, `agentId`, `contractId`, `taskGeneration`, lineage, model/policy snapshots, safe usage/error fields, timestamps |

`TaskGateKind` has `CLARIFICATION`, `APPROVAL`, and `RECOVERY`. `TaskMessageKind`
has `HUMAN_ANSWER`, `HUMAN_CHANGE_REQUEST`, and `RETRY_NOTE`. The API keeps
submission, review, criterion evidence, artifact link, and transcript records
immutable.

`TaskRecoveryReason` has `INFRASTRUCTURE_RETRIES_EXHAUSTED`,
`REVIEW_ROUNDS_EXHAUSTED`, `UNSAFE_EFFECT_UNCERTAIN`,
`CONFIGURATION_UNAVAILABLE`, and `INVARIANT_FAULT`. Recovery `validActions`
follow the closed reason/action table in the domain model; clients never infer
them from the prompt.

`TaskAttention` is derived rather than persisted. It contains `kind`,
`title`, `summary`, `gate`, `review`, `task`, and `validActions`. `kind` is
one of `CLARIFICATION_REQUIRED`, `APPROVAL_REQUIRED`, `RECOVERY_REQUIRED`,
or `REVIEW_READY`.

`ValidTaskAction` is the closed enum:

```text
EDIT | QUEUE | ANSWER | RETRY | ACCEPT | REQUEST_CHANGES | CANCEL | REOPEN
```

It is legal for a task to expose no actions. A client must render only actions
returned by the server; it must not infer that a stage permits an operation.

### Events and connections

`WorkEvent` is the public projection of the append-only `work_events` ledger:

| Field | Meaning |
| --- | --- |
| `cursor`, `eventId`, `kind`, `occurredAt` | Stable replay identity and ordering. |
| `workspaceId`, `projectId`, `taskId`, `runId` | Optional object linkage. |
| `actor`, `causationId`, `correlationId` | Safe audit attribution. |
| `payload` | Bounded safe JSON payload; never the source of client stage semantics. |

`WorkEvent.kind` is the closed persisted `WorkEventKind` string vocabulary,
rather than a GraphQL enum that rewrites the dotted names. It preserves the
exact V1 values defined in [the domain event contract](02-domain-model.md#event-contract),
including `task.stage_changed`, `contract.created`, `run.waiting_for_approval`,
and `notification.delivered`. The API may add payload fields but does not
change an existing event kind's meaning. UI activity labels are generated from
`kind` and safe structured fields, never free-form model output.

Every list uses a conventional `Connection`:

```text
Connection<T> { edges: [Edge<T>!]!, pageInfo: PageInfo! }
Edge<T>       { cursor: String!, node: T! }
PageInfo      { endCursor: String, hasNextPage: Boolean! }
```

The exposed Work connections are `ProjectConnection`, `TaskConnection`,
`TaskAttentionConnection`, and `WorkEventConnection`.
`TaskRunItemConnection` remains the bounded transcript read for a selected run.
Task detail is one owner-authorized snapshot rather than a collection of
independently paginated audit APIs.

## Queries

| Query | Arguments | Result and behavior |
| --- | --- | --- |
| `workOverview` | `workspaceId`, optional `projectId` | `WorkOverview` with workspace, workflow stages, per-stage active counts, bounded recent task summaries, and Needs You count. It is the board bootstrap query. |
| `projects` | `workspaceId`, `includeArchived`, `first`, `after` | `ProjectConnection`. Archived rows are excluded unless requested. |
| `workTasks` | `input: WorkTasksInput!`, `first`, `after` | `TaskConnection` for Board or List. |
| `task` | `taskId` | `TaskDetail`, or a safe `work_unavailable` error for a missing or unauthorized task. |
| `needsYou` | `workspaceId`, optional `projectId`, `first`, `after` | `TaskAttentionConnection` derived from unresolved gates and current approved reviews. |
| `workActivity` | `workspaceId`, optional `projectId`, `taskId`, `first`, `after` | `WorkEventConnection` ordered newest-first; `after` pages toward older sequence numbers. |
| `completedTasks` | `workspaceId`, optional `projectId`, `kind`, `text`, `first`, `after` | `TaskConnection` limited to completed and cancelled stages. |
| `taskRunItems` | `runId`, `first`, `after` | Owner-authorized `TaskRunItemConnection` for a detail transcript. |

`WorkTasksInput` is:

```text
{
  workspaceId: ID!
  projectId: ID
  stageIds: [ID!]
  stageBehaviors: [WorkflowStageBehavior!]
  text: String
  attentionOnly: Boolean = false
  scope: ACTIVE | TERMINAL | ALL = ACTIVE
}
```

`ACTIVE` includes Inbox, Queue, Doing, Waiting, and Review by their stage
behavior/configuration, not by hard-coded client labels. `TERMINAL` includes
Terminal Success and Terminal Cancelled behavior. The server rejects an
inconsistent filter such as an active scope with only terminal stage ids.
`completedTasks.kind` uses `TerminalTaskKind = ACCEPTED | CANCELLED | ALL` and
defaults to `ALL`; Accepted maps only to `TERMINAL_SUCCESS`, while Cancelled
maps only to `TERMINAL_CANCELLED`.

`WorkOverview` exposes workflow stage metadata and an `activeColumns` list in
display order. Board clients render this data rather than hard-coding English
stage names, though the first release ships the five active seeded stages.

The `task` detail contains the current contract, current run and gate, latest
and accepted submissions, latest review, attention and valid actions, plus
bounded lists of messages, runs, submissions, reviews, and artifacts. These
lists are assembled in one store transaction and returned as plain GraphQL
lists. Contract revisions, resolved gates, and per-task activity remain in the
durable ledger but are not separate first-release UI query surfaces. A selected
run's potentially long transcript stays independently cursor-paginated through
`taskRunItems`.

Resolvers batch current run, gate, review, project, and action derivation for a
page of task summaries. They must not issue one run/review/gate query per card.
The detail resolver loads its bounded evidence/history snapshot in one
transaction and maps it directly, without re-entering public list resolvers.

## Semantic mutations

There is no `setTaskStage` mutation, no generic patch of arbitrary task fields,
and no drag-and-drop mutation path. Every mutation is a command with one
documented effect.

| Mutation | Input | Payload |
| --- | --- | --- |
| `createProject` | `CreateProjectInput { workspaceId, name, description, clientMutationId }` | `ProjectCommandPayload` |
| `updateProject` | `UpdateProjectInput { projectId, expectedRevision, name?, description?, clientMutationId }` | `ProjectCommandPayload` |
| `archiveProject` | `ArchiveProjectInput { projectId, expectedRevision, clientMutationId }` | `ProjectCommandPayload` |
| `reopenProject` | `ReopenProjectInput { projectId, expectedRevision, clientMutationId }` | `ProjectCommandPayload` |
| `captureTask` | `CaptureTaskInput { workspaceId, projectId?, title, description, clientMutationId }` | `TaskCommandPayload` |
| `updateInboxTask` | `UpdateInboxTaskInput { taskId, expectedRevision, expectedGeneration, title?, description?, projectId?, clearProject?, clientMutationId }` | `TaskCommandPayload` |
| `queueTask` | `QueueTaskInput { taskId, expectedRevision, expectedGeneration, clientMutationId }` | `TaskCommandPayload` |
| `answerTask` | `AnswerTaskInput { taskId, gateId, expectedRevision, expectedGeneration, answerMarkdown, approvalDecision?, clientMutationId }` | `TaskCommandPayload` |
| `retryTask` | `RetryTaskInput { taskId, gateId, expectedRevision, expectedGeneration, retryNote?, clientMutationId }` | `TaskCommandPayload` |
| `acceptTask` | `AcceptTaskInput { taskId, expectedRevision, expectedGeneration, clientMutationId }` | `TaskCommandPayload` |
| `requestTaskChanges` | `RequestTaskChangesInput { taskId, expectedRevision, expectedGeneration, feedbackMarkdown, requestMarkdown?, replacementCriteria?, complexity?, clientMutationId }` | `TaskCommandPayload` |
| `cancelTask` | `CancelTaskInput { taskId, expectedRevision, expectedGeneration, reason?, clientMutationId }` | `TaskCommandPayload` |
| `reopenTask` | `ReopenTaskInput { taskId, expectedRevision, expectedGeneration, clientMutationId }` | `TaskCommandPayload` |

`updateInboxTask` requires at least one editable field. When it sets
`clearProject`, `projectId` must be null; when it supplies `projectId`,
`clearProject` must be false or omitted. `queueTask` carries no caller-supplied
contract, model, or policy input: from Inbox it queues a Planner. A Queue
reached through a resolved gate, retry, or server continuation queues an
Executor only when a complete current contract already exists. `answerTask`
requires nonblank `answerMarkdown` and requires `approvalDecision` to be
`APPROVED` or `DECLINED` for an Approval gate.

`TaskCommandPayload` is:

```text
{
  task: TaskDetail!
  eventCursor: String!
  clientMutationId: String!
}
```

`ProjectCommandPayload` has the analogous `project`, `eventCursor`, and
`clientMutationId` fields. The returned cursor identifies the event committed
with the mutation. Replaying the same client mutation returns the originally
committed payload; replaying it with different normalized input returns
`idempotency_conflict`.

## Errors and authorization

Command validation errors use normal GraphQL errors with a stable
`extensions.code` and a safe human-readable message. The first-release codes
are:

| Code | Meaning |
| --- | --- |
| `work_unavailable` | Missing or unauthorized workspace, project, or task; no existence leak. |
| `stale_revision`, `stale_generation` | The caller must reload the authoritative object before retrying. |
| `invalid_transition`, `workflow_mismatch` | The command is not allowed from this task stage or workflow. |
| `project_archived` | A mutation tries to associate a task with an archived project. |
| `contract_required`, `contract_immutable` | Contract preconditions or immutable-version rules failed. |
| `gate_required`, `gate_unresolved` | The named/current gate is absent, invalid, or must be resolved first. |
| `review_not_approved`, `review_limit_reached` | Current review predicates do not permit the requested action. |
| `configuration_unavailable` | No ready planner/executor/reviewer route or policy can be frozen. |
| `run_fenced` | A stale or cancelled run attempted a terminal write. |
| `idempotency_conflict` | A client mutation id was reused for different normalized input. |
| `invalid_input`, `invalid_cursor` | Field, criterion, page-size, or cursor validation failed. |

Provider failures, authorization implementation details, lease tokens,
credentials, redacted task content, and raw tool arguments never enter an
error extension. A stale mutation is an expected product conflict, not a
silent retry: the UI reloads the relevant task and preserves the user's
unsent text.

## Subscriptions and replay

`workEvents(workspaceId: ID!, after: String)` is the one durable Work event
stream. `after` is an exclusive global sequence cursor. If it is absent, the
subscription starts after the current ledger head and receives only later
events; a client that needs history calls `workActivity` first.

`taskEvents(taskId: ID!, after: String)` is a filtered projection of
`workEvents`. It does not read a task-specific event ledger and has the same
cursor semantics. It exists for the chat detail rail and narrow task views.

The subscription implementation must:

1. Authorize the workspace or task before allocating a receiver.
2. Establish live invalidation, then capture a durable high-water cursor and
   replay the ledger after the supplied cursor through that high water, so a
   commit cannot fall between receiver setup and replay.
3. Replay durable events in batches of at most 256 and continue from the
   high-water cursor before waiting for new invalidations.
4. On a broadcast lag signal, re-read the ledger from the last delivered
   cursor rather than dropping events.
5. Emit only committed events and advance the cursor only after the event has
   been projected successfully.

Event delivery is at-least-once across reconnects. Clients deduplicate by
cursor/event id, keep the newest cursor in route-local state, and refetch
active bounded queries after a subscription-ready reconnect. They must not
reconstruct task stage by replaying event payloads.

## GraphQL acceptance checks

The API implementation is complete when:

- Board, List, Needs You, Activity, Completed, chat cards, and task detail all
  read the same `stage` plus derived projections;
- every mutation uses expected revision, task generation where applicable, and
  `clientMutationId`, and no arbitrary stage mutation exists;
- pagination, filters, nullable project linkage, and terminal history are
  bounded and stable;
- Task attention is derived from stage/gate/review rather than a second
  persisted phase;
- unavailable objects, stale commands, gate conflicts, and missing model
  configuration return stable safe errors;
- `workEvents` replays durable history without a lost-event race, and
  `taskEvents` is demonstrably a filtered projection of that same stream.
