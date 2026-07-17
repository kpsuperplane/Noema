# Work commands and reconciliation

**Authority:** this document owns semantic command behavior, runtime terminal writes, reconciliation decisions, and crash recovery. [The domain model](02-domain-model.md) owns types and legal stages; [storage and events](03-storage-and-events.md) owns tables, ledger shape, and transaction mechanics.

## Command service rules

`WorkCommandService` is the only path that mutates work projections. It accepts typed commands, reloads the authoritative rows inside a SQLite immediate transaction, validates stage behavior and version fences, writes every affected projection, appends ledger events, schedules deterministic next work when appropriate, persists an idempotency receipt, and commits once.

For any command with an idempotency key, receipt lookup precedes version checking. An exact replay returns the original result even if the task has since changed; a different request fingerprint returns `idempotency_conflict`; a new command must satisfy the current expected revision and generation. This makes provider-tool retries safe without making stale UI buttons silently succeed.

Task commands use the `TaskPrecondition` from [the domain model](02-domain-model.md): `task_id`, `expected_revision`, and `expected_generation`. Every successful task mutation increments `revision`. `generation` increments only for cancellation, human contract revision, and reopen. Project commands use their own revision precondition. A command never accepts a caller-selected raw stage or run status.

Every stage change appends `task.stage_changed` with `from_stage_id`, `to_stage_id`, and a closed semantic reason. The named events below are emitted as well when they carry user-meaningful facts. A command may enqueue a chat notification only for the five card types defined by the product contract: task created, Waiting, Review ready, recovery needed, and accepted completion.

## Human and primary-agent commands

### `CaptureTask`

| Item | Contract |
| --- | --- |
| Allowed start | No task exists. |
| Required input | Personal `workspace_id`; nonblank title; description Markdown, which may be empty; optional active `project_id`; provenance; actor/correlation metadata. |
| Version fence | Creation envelope has no task revision or generation. A source tool call and idempotency key, when present, identify replay. |
| Transaction writes | Insert task at `stage:personal:inbox`, `generation = 1`, `revision = 1`, no current contract, gate, run, submission, review, or accepted result. |
| Events | `task.captured`, `notification.queued`. |
| Runs | None. Capturing never authorizes worker execution. |
| Replay | Returns the original task and cursor when the receipt or source chat/tool-call match is exact. |
| Failures | `work_unavailable`, `project_archived`, `invalid_input`, or `idempotency_conflict`. |
| Notification | Enqueue one `task_created` card for the owning human's global primary conversation, regardless of whether creation came from chat or Work. |

### `UpdateInboxTask`

| Item | Contract |
| --- | --- |
| Allowed start | Inbox only. |
| Required input | Task precondition and at least one of title, description, or project association. A supplied title must be nonblank. `project_id: None` explicitly removes a project association. |
| Version fence | Exact task revision and generation. |
| Transaction writes | Update only `title`, `description_markdown`, `project_id`, `revision`, and `updated_at`. The workspace and workflow are immutable in V1. |
| Events | `task.updated`, with changed-field names but never unredacted large descriptions. |
| Runs | None. |
| Replay | Returns the stored resulting task/cursor. |
| Failures | `invalid_transition`, `stale_revision`, `stale_generation`, `project_archived`, `work_unavailable`, `invalid_input`, `idempotency_conflict`. |
| Notification | None. |

### `QueueTask`

| Item | Contract |
| --- | --- |
| Allowed start | Inbox only. |
| Required input | Task precondition and command metadata. The public UI/API does not submit model or policy snapshots. |
| Version fence | Exact task revision and generation. |
| Transaction writes | Change Inbox to Queue, set `queued_at`, increment revision. Because an Inbox capture has no contract, insert one queued Planner run using the executor identity and its resolved planner model snapshot. |
| Events | `task.queued`, `task.stage_changed`, `run.queued`. |
| Runs | Exactly one Planner run. The active-runnable-run index rejects a duplicate. |
| Replay | Returns the task and Planner run originally created. |
| Failures | `invalid_transition`, `stale_revision`, `stale_generation`, `configuration_unavailable`, `work_unavailable`, `idempotency_conflict`. |
| Notification | None; the creation card already covers the capture. |

The command does not create an incomplete contract. Other commands may move a
task through Queue as a scheduling boundary: Answer resumes the gate-owning
role, Retry resumes the failed role, and a human contract revision queues
Executor. Those commands persist the correct child run atomically; they never
use “contract exists” as a substitute for role lineage.

### `AnswerTask`

| Item | Contract |
| --- | --- |
| Allowed start | Waiting with the task’s one open Clarification, Approval, or Recovery gate. |
| Required input | Task precondition, current `gate_id`, and `TaskGateAnswer`: nonblank answer Markdown plus structured `Approved` or `Declined` when the gate kind is Approval. |
| Version fence | Exact task revision/generation plus an open gate in the same generation; Recovery additionally requires a non-null `retry_run_kind` and reason `infrastructure_retries_exhausted`, `review_rounds_exhausted`, or `unsafe_effect_uncertain`. |
| Transaction writes | Insert a `human_answer` message; resolve the gate; complete the gate-owning `waiting_for_approval` run if one exists; move task to Queue, clear `active_gate_id`, set `queued_at`, increment revision. |
| Events | `task.message_appended`, `gate.resolved`, `run.completed` when a gate-owning run is released, `task.stage_changed`, then `run.queued` for the reconciled next role. |
| Runs | For Clarification/Approval, queue a child of the role that opened the gate. For Recovery, require and queue its explicit `retry_run_kind`; a gate with no safe retry role does not expose Answer. Reviewer continuation retains the reviewed submission; the answer is consumed only by that run at its safe prompt boundary. |
| Replay | Returns the original resolved gate, task, and queued run. |
| Failures | `invalid_transition`, `gate_required`, `gate_unresolved`, `stale_revision`, `stale_generation`, `work_unavailable`, `configuration_unavailable`, `idempotency_conflict`. |
| Notification | None. |

### `RetryTask`

| Item | Contract |
| --- | --- |
| Allowed start | Waiting with an open Recovery gate whose reason is infrastructure retries exhausted, review rounds exhausted, or configuration unavailable. Unsafe-effect and invariant recovery cannot use blind Retry. |
| Required input | Task precondition, current `gate_id`, and optional nonblank retry note. |
| Version fence | Exact task revision/generation and open Recovery gate with a non-null `retry_run_kind`. |
| Transaction writes | Resolve the recovery gate, append a `retry_note` when supplied, move task to Queue, set `queued_at`, increment revision, and clear the active gate pointer. |
| Events | `gate.resolved`, optional `task.message_appended`, `task.stage_changed`, `run.queued`. |
| Runs | Queue a new child of the Recovery gate's explicit `retry_run_kind`. It retains the applicable contract, submission, and review-round lineage. Infrastructure recovery increments `attempt_index`; review-round recovery queues Executor at the next round and permits that one human-authorized round beyond the automatic limit. It never mutates the prior run. |
| Replay | Returns the same queued continuation. |
| Failures | `invalid_transition`, `gate_required`, `stale_revision`, `stale_generation`, `configuration_unavailable`, `work_unavailable`, `idempotency_conflict`. |
| Notification | None. |

### `AcceptTask`

| Item | Contract |
| --- | --- |
| Allowed start | Review only. |
| Required input | Task precondition. No result or stage text is accepted from the caller. |
| Version fence | Exact task revision/generation; the latest review must target the current contract/generation, have verdict `approve`, and contain exactly one `Pass` for each criterion. No gate may be open. |
| Transaction writes | Move task to Completed; set `accepted_submission_id`, `completed_at`; clear `queued_at`; increment revision. |
| Events | `task.accepted`, `task.stage_changed`, `notification.queued`. |
| Runs | None. Review is a human acceptance state, so execution is already idle. |
| Replay | Returns the accepted task and its original completion event cursor. |
| Failures | `invalid_transition`, `review_not_approved`, `gate_unresolved`, `stale_revision`, `stale_generation`, `work_unavailable`, `idempotency_conflict`. |
| Notification | Enqueue one `task_accepted` card for the owning human's global primary conversation. |

### `RequestTaskChanges`

| Item | Contract |
| --- | --- |
| Allowed start | Review only. |
| Required input | Task precondition and a nonblank `TaskContractAmendment.feedback_markdown`; optional replacement request, complete replacement criteria, and complexity. |
| Version fence | Exact task revision/generation and a latest approving review for the current contract. |
| Transaction writes | Create a new immutable contract with the next version, fresh eligible model/policy/context snapshots, a cleared execution plan, and the next task generation; append a `human_change_request` message linked to that contract; set it current; increment task generation and revision; move task to Queue; clear acceptance/gate/current result pointers; queue a new Executor run. |
| Events | `task.message_appended`, `contract.created`, `task.queued`, `task.stage_changed`, `run.queued`. |
| Runs | One Executor run for the new contract/generation. |
| Replay | Returns the exact new contract and queued run. |
| Failures | `invalid_transition`, `review_not_approved`, `stale_revision`, `stale_generation`, `invalid_input`, `configuration_unavailable`, `work_unavailable`, `idempotency_conflict`. |
| Notification | None. |

The command does not alter the capture title, description, project, workspace, or prior contract. It is the only post-Inbox path that changes the executable request or criteria.

### `CancelTask`

| Item | Contract |
| --- | --- |
| Allowed start | Inbox, Queue, Doing, Waiting, or Review. |
| Required input | Task precondition and optional safe cancellation reason. |
| Version fence | Exact task revision and generation. |
| Transaction writes | Increment generation and revision; move task to Cancelled; set `cancelled_at`; clear `queued_at`; supersede any open gate. Every queued, leased, or running run becomes `cancelled`; in-flight holders also receive `cancellation_requested = 1` and a runtime cancellation signal. |
| Events | `run.cancel_requested` and `run.cancelled` for in-flight holders, `run.cancelled` for queued runs, `gate.superseded` when relevant, `task.cancelled`, and `task.stage_changed`. |
| Runs | No new run. A worker that races with cancellation fails its generation/lease predicate and cannot create evidence or change stage. |
| Replay | Returns the terminal task produced by the original cancellation. |
| Failures | `invalid_transition` for Completed/Cancelled tasks, `stale_revision`, `stale_generation`, `work_unavailable`, `idempotency_conflict`. |
| Notification | None in V1; cancellation remains visible in Work and the task rail. |

### `ReopenTask`

| Item | Contract |
| --- | --- |
| Allowed start | Completed or Cancelled only. |
| Required input | Task precondition and command metadata. |
| Version fence | Exact task revision/generation. |
| Transaction writes | Increment generation/revision; move task to Inbox; clear `current_contract_id`, `active_gate_id`, latest run/submission/review pointers, accepted submission, queue/completion/cancellation timestamps. Existing contracts, evidence, transcripts, artifacts, gates, and events remain historical rows. |
| Events | `task.reopened`, `task.stage_changed`. |
| Runs | None. A reopened task requires explicit Queue or a new delegation. |
| Replay | Returns the same Inbox task. |
| Failures | `invalid_transition`, `stale_revision`, `stale_generation`, `work_unavailable`, `idempotency_conflict`. |
| Notification | None. |

### Tool-only `DelegateTask` composition

`task.delegate` is not a second state machine. It is the primary-agent-only
atomic composition for authorized asynchronous work, whether or not the
primary agent can already supply a complete execution intent.

| Item | Contract |
| --- | --- |
| Allowed start | No task exists. |
| Required input | Capture fields plus either no `execution_intent` and optional `complexity_hint`, or one complete intent containing request, criteria, complexity, and optional execution plan. Supplying both a separate hint and an intent, or a partial intent, is `invalid_input`. The store, not the model, resolves exact selections and policy. |
| Version fence | Creation has no task revision/generation; source conversation/tool-call identity and required provider-tool idempotency key fence replay. |
| Transaction writes | Always create the task directly in Queue. Without an intent, create one queued Planner using the hint or medium fallback and no contract. With an intent, create contract version 1 with `origin = delegated`, copy criteria/context/model/policy snapshots, and create one queued Executor. |
| Events | Always `task.captured`, `task.queued`, `run.queued`, `notification.queued`; complete intent also emits `contract.created`. |
| Runs | Exactly one Planner or Executor, selected only by complete-contract presence. |
| Replay | Source conversation/tool-call uniqueness and idempotency receipt return the existing task/run and contract, if any. |
| Failures | `work_unavailable`, `project_archived`, `configuration_unavailable`, `invalid_input`, or `idempotency_conflict`; an atomic failure creates nothing. |
| Notification | One `task_created` card in the owning human's global primary conversation. |

The primary runtime may decide foreground versus capture versus delegate through structured tool choice and policy. No phrase match, duration-string parser, or literal tool-call count is a semantic authority.

## Project commands

Projects are containers, so their commands have no task stage or run effects.

| Command | Allowed start and required input | Transaction writes/events | Replay and failures |
| --- | --- | --- | --- |
| `CreateProject` | Active Personal workspace; nonblank name; description may be empty; creation metadata. | Insert project at revision 1 with `archived_at = NULL`; emit `project.created`. | Exact receipt returns it; failures are `work_unavailable`, `invalid_input`, `idempotency_conflict`. |
| `UpdateProject` | Existing unarchived project; project ID/revision; at least one of name/description. | Update supplied fields, revision, timestamp; emit `project.updated`. Existing contract snapshots remain unchanged. | Exact receipt returns it; failures are `work_unavailable`, `stale_revision`, `invalid_input`, `idempotency_conflict`. |
| `ArchiveProject` | Existing unarchived project; ID/revision. | Set `archived_at`, increment revision; emit `project.archived`. Do not move/cancel existing tasks. | Exact receipt returns it; failures are `work_unavailable`, `stale_revision`, `invalid_transition`, `idempotency_conflict`. |
| `ReopenProject` | Existing archived project; ID/revision. | Clear `archived_at`, increment revision; emit `project.reopened`. | Exact receipt returns it; failures are `work_unavailable`, `stale_revision`, `invalid_transition`, `idempotency_conflict`. |

No project command changes agent identity, model pools, memory retrieval, task contract snapshots, capabilities, or external-action approvals.
No project command enqueues a chat notification in V1; project activity remains
available through the Work event stream.

## Runtime terminal commands

Background roles never write rows directly. They submit typed terminal contracts to the command service with run ID, lease token, task generation, and all required evidence. These commands are fenced exactly like user mutations and are idempotent on the immutable terminal record they create.

### Claim and start

`ClaimNextRun` selects the next queued run globally by `queued_at, run_id`, subject to the existing eight-run supervisor limit. It accepts a worker ID and produces a lease token/expiry.

- If the task is Queue, it atomically leases the run and changes the task to Doing.
- If the task is already Doing, it leases the deterministic child run and leaves stage unchanged; this covers automated retries and reviewer-requested executor revisions.
- Any other task stage, generation mismatch, cancellation flag, or active different run makes the candidate ineligible. The reconciler skips it rather than forcing it through a state change.

Claim emits `run.claimed` and, only for Queue-to-Doing, `task.stage_changed`. The same worker then uses `StartRun`, heartbeats, and transcript/usage recording. These are run-local actions; `StartRun` emits `run.started` but does not add a task stage axis.

### Planner terminal contract

`SubmitPlan` is available only to a leased Planner run. It has two mutually exclusive variants:

| Variant | Required terminal content | Atomic result |
| --- | --- | --- |
| Complete plan | A normalized request, bounded execution plan, exact nonempty criteria, and complexity. | Create the next immutable contract version, complete Planner, queue Executor, retain Doing, and emit `contract.created`, `run.completed`, `run.queued`. |
| Blocking question | Nonblank safe prompt/context and gate kind Clarification or Approval. | Put Planner in `waiting_for_approval`, open a gate, change task to Waiting, and emit `run.waiting_for_approval`/`gate.opened`/`task.stage_changed`/`notification.queued`. |

Planner model selection follows the executor-pool policy, but Planner has no general action tools. It cannot create an arbitrary draft, mutate task capture fields, execute the task, or choose capabilities. A plan that fails domain validation is not persisted and leaves the run active for a valid retry or eventual failure handling.

### Executor terminal contract

`SubmitTaskResult` is available only to a leased Executor run for the current task generation and contract. It requires a concise summary, complete result Markdown, exact nonempty criterion evidence, and a bounded deduplicated list of task-owned artifact version snapshots.

The service atomically validates the evidence set, inserts `task_submissions` and related rows, completes Executor, queues one Reviewer run for the same contract/generation, updates latest submission/run pointers, and leaves the task Doing. It emits `submission.created`, `run.completed`, and `run.queued`. Duplicate terminal calls with byte-for-byte normalized evidence return the existing submission/reviewer run; divergent evidence is `idempotency_conflict`/`invalid_input` and does not overwrite history.

An Executor that cannot continue safely may use `ReportTaskBlocked` instead of
submitting a result. The leased current-generation Executor supplies a
Clarification or Approval gate kind plus a nonblank prompt/context; the command
puts that run in `waiting_for_approval`, opens the gate for the same contract,
moves Doing to Waiting, and emits `run.waiting_for_approval`, `gate.opened`,
`task.stage_changed`, and `notification.queued`. It creates no submission. A
later `AnswerTask` completes the waiting run and queues an Executor child with
the same contract/review-round lineage, consuming the answer only at that
child's safe prompt boundary.

### Reviewer terminal contract

`SubmitTaskReview` is available only to a leased Reviewer run tied to the current submission and contract. It requires exactly one criterion outcome for each contract criterion, a verdict consistent with those outcomes, and safe overall feedback. The first review of a submission uses `review_attempt_index = 1`; a Reviewer resumed after `needs_human` writes the next immutable attempt and links the prior review rather than updating it.

| Verdict | Required evidence | Atomic result |
| --- | --- | --- |
| `approve` | Every criterion is `Pass`. | Persist review, complete Reviewer, move task to Review, set latest review pointer, emit `review.created`, `run.completed`, `task.stage_changed`, `notification.queued`, and enqueue `task_review_ready`. |
| `request_changes` | At least one `Fail`, no `Uncertain`. | Persist review and complete Reviewer, emitting `review.created` and `run.completed`. If another review round remains, emit `run.queued` for a new Executor on the same contract/generation and leave task Doing. If the bound is exhausted, emit `gate.opened`, `task.stage_changed`, and `notification.queued` for Recovery/Waiting. |
| `needs_human` | At least one `Uncertain` and explicit Clarification or Approval gate kind. | Persist this review attempt, put Reviewer in `waiting_for_approval`, open that gate, move task to Waiting, emit `review.created`, `run.waiting_for_approval`, `gate.opened`, `task.stage_changed`, and `notification.queued`, then enqueue `task_waiting`. A later answer queues a Reviewer for the same submission and next review attempt. |

A reviewer cannot complete a task. It cannot create artifacts, alter the submission, delegate, use write/export tools, or suppress a failed/uncertain criterion.

### Run interruption, failure, and recovery

`ReportRunFailure` and lease expiry both use the same pure recovery planner. A retryable failure for the current generation creates exactly one queued child run with the same role, contract, review round, and model/policy snapshot, increments that lineage's `attempt_index`, and leaves the task Doing. It emits `run.failed` or `run.interrupted` followed by `run.queued`.

When automatic retries are exhausted, a failure is nonretryable, or a terminal contract cannot safely be reconstructed, the service completes/fails the run, opens a Recovery gate, moves the task to Waiting, and emits the applicable `run.failed` or `run.interrupted` plus `gate.opened`, `task.stage_changed`, and `notification.queued`. It enqueues `task_recovery` for the owning human's global primary conversation. It never marks a task Completed or invents a human answer because a worker process exited.

## Reconciler decision table

The reconciler runs after bootstrap, after each worker wakeup, after every command commit, and periodically while workers exist. It is idempotent: it derives its action solely from task stage, task generation, current contract, current gate, submissions, reviews, and runs; its insert predicates and unique indexes make an already-applied action a no-op.

| Durable facts | Decision | Result |
| --- | --- | --- |
| Inbox, no runnable run | Intentionally idle. | No action; only Queue or delegate authorizes work. |
| Queue, resolved gate with an unconsumed message and no runnable run | Resume the opening role for Clarification/Approval or the explicit Recovery `retry_run_kind`. | One Planner, Executor, or Reviewer child with the original lineage. |
| Queue, no current contract, no runnable run | Queue Planner. | One Planner run ordered by FIFO. |
| Queue, complete current contract, no runnable run | Queue Executor. | One Executor run ordered by FIFO. |
| Queue or Doing, one queued/leased/running current-generation run | Intentionally busy. | No duplicate run. |
| Queue, stale/terminal run and no runnable replacement | Evaluate the normal failure/recovery rule. | Retry child or Recovery gate. |
| Doing, completed Planner, no contract, no open gate | Reconcile terminal planner evidence. | Create contract/Executor only if a valid persisted complete plan exists; otherwise Recovery gate. |
| Doing, current contract, completed Executor submission with no review/run | Queue Reviewer. | One Reviewer run. |
| Doing, review `request_changes`, remaining review rounds, no runnable run | Queue Executor. | One same-contract executor revision. |
| Doing, review `request_changes`, review limit reached | Open Recovery gate. | Waiting, no runnable run. |
| Doing, retryable interrupted/failed current run under retry limit | Queue same-role child run. | Doing remains active. |
| Doing, exhausted/nonretryable current run | Open Recovery gate. | Waiting, no runnable run. |
| Doing, approved review with no stage change due to a crash | Move to Review if all approval predicates still hold. | Review, notification outbox row if absent. |
| Doing, review `needs_human` or planner blocking question with no gate due to a crash | Open its persisted gate. | Waiting, notification outbox row if absent. |
| Waiting, exactly one open current-generation gate and no runnable run | Intentionally idle. | Wait for Answer or Retry. |
| Waiting, gate resolved but Queue transition missing due to an invariant-recovery path | Move to Queue and resume the role recorded by the gate's origin or explicit Recovery retry role. | Planner, Executor, or Reviewer. |
| Review, complete current-generation approving review and no gate/run | Intentionally idle. | Wait for Accept or Request Changes. |
| Completed or Cancelled | Fence any accidentally runnable old-generation run and leave history intact. | No new run. |

Any combination not represented above is an invariant fault, not an invitation to guess from a transcript, artifact, process table, Git state, terminal text, or English model output. The reconciler emits a safe diagnostic event and opens a Recovery gate when a human decision is necessary; it does not silently repair evidence.

## Crash and race recovery

| Boundary | Durable state after a crash | Recovery outcome |
| --- | --- | --- |
| Before a command transaction commits | No projection, ledger, outbox, or receipt exists. | Caller may retry normally. |
| After a task/contract/run command commits but before worker wakeup | Task and queued run/event are durable. | Reconciler finds the queued row and dispatches it once. |
| After a worker lease commits but before provider execution starts | Run is Leased and task is Doing. | Lease expiry changes the run to Interrupted; recovery queues one fenced child or opens Recovery. |
| After provider output but before a terminal command commits | No submission/review/gate was accepted. | Worker can retry the same terminal contract while it owns a valid lease; otherwise normal lease recovery applies. |
| After a resolved message is checkpointed/consumed but before the child provider call | The child run transcript contains the exact bounded context and the message names that run as consumer. | Lease recovery resumes from the durable checkpoint; it never drops the answer or injects it into another role. |
| After executor submission commits but before reviewer starts | Immutable submission and queued Reviewer are durable. | Reconciler sees and claims the Reviewer. |
| After reviewer decision commits but before notification delivery | Stage/review/gate is durable and the outbox row is pending. | Outbox redelivers by deterministic notification ID. |
| After a conversation item is inserted but before outbox acknowledgement | The deterministic item exists and the outbox row remains leased/pending. | Redelivery observes the item ID and marks the row delivered without inserting another card. |
| After a human answer/change/accept commits but response is lost | Receipt, task revision, and event cursor are durable. | Exact client retry returns the original result without another transition. |
| After cancellation commits but before the runtime receives its signal | Task generation and run cancellation are already durable. | The old worker's next heartbeat/terminal write is fenced; best-effort signaling only shortens cleanup. |
| Cancellation races a worker terminal write | Cancellation increments generation and marks the run; one transaction wins. | The loser fails the generation/lease predicate and writes no task evidence or stage. |
| Reopen races an old run | Reopen increments generation and clears current pointers. | The old run remains historical and is fence-rejected. |
| Subscription disconnects after an event commit | Event has a global sequence in SQLite. | Client resumes by durable `workEvents(after)` scan before relying on broadcast. |

The reconciler and notification worker may run more than once after any of these boundaries. Their only externally visible effects are protected by unique rows, generation fencing, deterministic IDs, and idempotency receipts.
