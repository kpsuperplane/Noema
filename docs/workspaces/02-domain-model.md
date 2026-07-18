# Work domain model

**Authority:** this document owns the public work-domain vocabulary, invariants, and task transition model. [Storage and event persistence](03-storage-and-events.md) owns SQLite layout; [command and reconciliation behavior](04-commands-and-reconciliation.md) owns transaction sequencing and recovery; the implementation packets turn these contracts into code.

## The single task state

`tasks.stage_id` is the only persisted task-level state. It identifies one row in `workflow_stages`; its `system_behavior` gives the runtime meaning. A task never stores a second `status`, `lifecycle`, `execution_phase`, or duplicated “is running” field.

`agent_runs.status` remains run-local queue and lease machinery. A running planner, executor, or reviewer is a projection of the task's current run, not a task state. Gates, reviews, submissions, and events are separate durable facts. This separation makes all of the following unambiguous:

| Question | Authoritative record |
| --- | --- |
| Where is this task in the user workflow? | `tasks.stage_id` |
| Is a worker currently leased or executing? | the current `agent_runs` row |
| What exact instructions and limits govern this work? | immutable `task_execution_contracts` row |
| Does a human need to do something? | an open `task_gates` row, or a reviewer-approved task at Review |
| What happened and in what order? | `work_events` |

The display name of a stage is never an authority. The runtime resolves the `WorkflowStageBehavior` associated with its ID, and only the seeded Personal workflow is executable in the first release.

## Module and identifier ownership

`noema-workspaces` owns workspace/project identities, records, and validation. `noema-tasks` owns workflow definitions, stages, task contracts, gates, messages, run vocabulary, submissions, reviews, event vocabulary, and pure transition planning. Neither crate owns SQLite transactions, GraphQL, prompts, or UI state.

There is deliberately no generic ID crate. Each semantic crate exposes a validated opaque newtype and owns its parser. New IDs must be nonblank, at most 255 UTF-8 bytes, contain no control characters, and use their semantic persisted prefix. Callers may compare and serialize them, but may not construct them from an unchecked `String`.

| Type | Owned by | Canonical prefix | Meaning |
| --- | --- | --- | --- |
| `WorkspaceId` | `noema-workspaces` | `workspace:` | A durable container for projects and tasks. |
| `ProjectId` | `noema-workspaces` | `project:` | An optional task container inside one workspace. |
| `WorkflowId` | `noema-tasks` | `workflow:` | A stage definition set for a workspace. |
| `WorkflowStageId` | `noema-tasks` | `stage:` | One stage in one workflow. |
| `TaskId` | `noema-tasks` | `task:` | The durable task shared by chat, Work, APIs, and runs. |
| `TaskContractId` | `noema-tasks` | `contract:` | One immutable executable contract version. |
| `TaskGateId` | `noema-tasks` | `gate:` | One human intervention request. |
| `TaskMessageId` | `noema-tasks` | `task_message:` | One durable human answer, change request, or retry note. |
| `WorkEventId` | `noema-tasks` | `event:` | An immutable global-ledger event. |

Existing `RunId`, `SubmissionId`, `ReviewId`, artifact IDs, conversation IDs, and actor IDs retain their semantic owners. New work APIs accept those validated types rather than generic strings at crate boundaries.

In the Rust-shaped contracts below, `Timestamp` denotes the existing canonical UTC ISO-8601 persistence string. This work does not introduce a generic time crate or a second timestamp representation.

## Workspace and project records

The first release exposes only `workspace:personal`, but persists membership so later sharing does not require changing task identity.

```rust
pub struct WorkspaceRecord {
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: String,
    pub is_personal: bool,
    pub archived_at: Option<Timestamp>,
    pub revision: u64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

pub struct WorkspaceMembership {
    pub workspace_id: WorkspaceId,
    pub human_id: HumanId,
    pub role: WorkspaceRole, // Owner | Member
    pub created_at: Timestamp,
}

pub struct ProjectRecord {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: String,
    pub revision: u64,
    pub archived_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
```

Project archive state controls whether a project can receive a newly captured or moved Inbox task. It does not silently cancel, move, hide from history, alter context snapshots for, or revoke authority from existing tasks. A workspace or project description is descriptive context only; it cannot select agents, retrieve memory, grant tools, or approve external effects.

## Workflow contracts

```rust
pub struct WorkflowDefinition {
    pub workflow_id: WorkflowId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub revision: u64,
    pub is_default: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

pub struct WorkflowStage {
    pub stage_id: WorkflowStageId,
    pub workflow_id: WorkflowId,
    pub stable_key: String,
    pub display_name: String,
    pub ordinal: u32,
    pub system_behavior: WorkflowStageBehavior,
    pub board_visible: bool,
}

pub enum WorkflowStageBehavior {
    Intake,
    Dispatch,
    Active,
    HumanGate,
    Acceptance,
    TerminalSuccess,
    TerminalCancelled,
}
```

`stable_key` is a machine-owned identifier for a definition row, never a display label. It is unique within a workflow. `system_behavior` is closed and parsed fail-closed. The store validates that a task's `(workflow_id, stage_id)` pair exists together; code may not move a task by a display string or raw stage ID from another workflow.

The Personal workflow is seeded with this exact definition. The board renders the five `board_visible` rows, in `ordinal` order.

| Stage ID | Stable key | Behavior | Board | User meaning |
| --- | --- | --- | --- | --- |
| `stage:personal:inbox` | `inbox` | `Intake` | yes | Captured work that is not authorized to run. |
| `stage:personal:queue` | `queue` | `Dispatch` | yes | Authorized work waiting for the next worker. |
| `stage:personal:doing` | `doing` | `Active` | yes | Planning, execution, automated review, or automated revision is underway. |
| `stage:personal:waiting` | `waiting` | `HumanGate` | yes | A clarification, approval, or recovery decision is needed. |
| `stage:personal:review` | `review` | `Acceptance` | yes | A reviewer-approved result awaits the human’s decision. |
| `stage:personal:completed` | `completed` | `TerminalSuccess` | no | The human accepted the result. |
| `stage:personal:cancelled` | `cancelled` | `TerminalCancelled` | no | The human cancelled the work. |

The V1 definition is fixed. A later workflow editor may rename or reorder stages only through an explicit workflow-definition command. Adding a stage requires a declared, supported behavior and an updated transition contract; no display-text convention may create semantics.

## Task, provenance, and execution contracts

`TaskRecord` remains intentionally general. It holds the user-facing capture, placement, optimistic version, and pointers to evidence. It does not duplicate contract fields such as complexity or model selection.

```rust
pub struct TaskRecord {
    pub task_id: TaskId,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub workflow_id: WorkflowId,
    pub stage_id: WorkflowStageId,
    pub title: String,
    pub description_markdown: String,
    pub provenance: TaskProvenance,
    pub generation: u64,
    pub revision: u64,
    pub current_contract_id: Option<TaskContractId>,
    pub active_gate_id: Option<TaskGateId>,
    pub latest_run_id: Option<RunId>,
    pub latest_submission_id: Option<SubmissionId>,
    pub latest_review_id: Option<ReviewId>,
    pub accepted_submission_id: Option<SubmissionId>,
    pub queued_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub completed_at: Option<Timestamp>,
    pub cancelled_at: Option<Timestamp>,
}

pub struct TaskProvenance {
    pub source_kind: TaskSourceKind,
    pub conversation_id: Option<ConversationId>,
    pub turn_id: Option<TurnId>,
    pub item_id: Option<ConversationItemId>,
    pub source_tool_call_id: Option<String>,
    pub created_by_actor_id: ActorId,
}

pub enum TaskSourceKind {
    ChatCapture,
    ChatDelegate,
    WorkUi,
    System,
}
```

`generation` starts at `1` and fences work from a prior execution cycle. It increments on cancellation, human-requested contract revision, and reopen. `revision` starts at `1` and increments on every successful task mutation, including automated mutations, so clients can perform optimistic concurrency. A command must compare both before changing a task.

An execution contract exists only when it is complete and immutable. Capture does not create a draft contract. Queueing without a complete contract creates a Planner run; a planner’s accepted terminal output creates the next contract version (`1` for a newly created task). Chat delegation is atomic in both forms: an underspecified delegation creates a Queue task plus Planner, while a complete delegation creates the Queue task, first contract, and Executor. Every later human revision creates another contract version rather than mutating prior instructions.

```rust
pub struct TaskExecutionContract {
    pub contract_id: TaskContractId,
    pub task_id: TaskId,
    pub version: u32,
    pub task_generation: u64,
    pub supersedes_contract_id: Option<TaskContractId>,
    pub origin: ContractOrigin,
    pub request_markdown: String,
    pub execution_plan_markdown: Option<String>,
    pub criteria: Vec<TaskValidationCriterion>,
    pub complexity: TaskComplexity,
    pub executor_model: ProviderSelectionSnapshot,
    pub reviewer_model: ProviderSelectionSnapshot,
    pub execution_policy: TaskExecutionPolicy,
    pub workspace_context: WorkspaceContextSnapshot,
    pub project_context: Option<ProjectContextSnapshot>,
    pub created_by_actor_id: ActorId,
    pub created_at: Timestamp,
}

pub enum ContractOrigin {
    Delegated,
    Planned,
    HumanRevision,
}

pub struct TaskValidationCriterion {
    pub criterion_id: String,
    pub ordinal: u32,
    pub description: String,
    pub expected_evidence: Option<String>,
}

pub struct TaskExecutionPolicy {
    pub max_provider_continuations: u32,
    pub max_tool_calls: u32,
    pub max_active_minutes: u32,
    pub progress_audit_interval: u32,
    pub max_automatic_retries: u32,
    pub max_review_rounds: u32,
}

pub struct WorkspaceContextSnapshot {
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: String,
}

pub struct ProjectContextSnapshot {
    pub project_id: ProjectId,
    pub name: String,
    pub description: String,
}
```

`TaskValidationCriterion` keeps the current exact-set contract: a nonempty description, positive unique ordinal, and optional expected evidence. Submission and review evidence must reference every criterion in the contract exactly once. `execution_plan_markdown` is required for a Planned contract and optional for Delegated or HumanRevision contracts; it is immutable guidance, not another task state. A human revision clears the prior plan unless a later planner produces a new one. `TaskExecutionPolicy` includes the bounded provider continuations, tool calls, active minutes, audit interval, automatic infrastructure retry limit, and maximum reviewed submission count. The existing executor/reviewer model snapshots remain full provider snapshots, including instance identity and reasoning effort.

The workspace/project snapshot is a bounded prompt section. It is not live retrieval, agent identity, capability policy, or external-action authority. A changed project description affects only newly created contracts.

### Human amendments

`RequestTaskChanges` carries a `TaskContractAmendment`:

```rust
pub struct TaskContractAmendment {
    pub feedback_markdown: String,
    pub request_markdown: Option<String>,
    pub replacement_criteria: Option<Vec<NewTaskValidationCriterion>>,
    pub complexity: Option<TaskComplexity>,
}
```

When a field is omitted, the new contract copies the corresponding current-contract value. When criteria are supplied, they are a complete replacement set, never a patch. The prior execution plan is cleared because it may no longer describe the amended request. The command resolves fresh eligible model and policy snapshots for the new contract, links the immutable human-change message to it, creates a new workspace/project context snapshot, increments task generation, and queues a new executor. It may not change the task workspace or project; those are Inbox-only task fields.

## Gates, human messages, runs, and evidence

```rust
pub enum TaskGateKind {
    Clarification,
    Approval,
    Recovery,
}

pub enum TaskRecoveryReason {
    InfrastructureRetriesExhausted,
    ReviewRoundsExhausted,
    UnsafeEffectUncertain,
    ConfigurationUnavailable,
    InvariantFault,
}

pub enum TaskGateState {
    Open,
    Resolved,
    Superseded,
}

pub enum ApprovalDecision {
    Approved,
    Declined,
}

pub struct TaskGateAnswer {
    pub message_markdown: String,
    pub approval_decision: Option<ApprovalDecision>,
}

pub struct TaskGateRecord {
    pub gate_id: TaskGateId,
    pub task_id: TaskId,
    pub task_generation: u64,
    pub contract_id: Option<TaskContractId>,
    pub kind: TaskGateKind,
    pub state: TaskGateState,
    pub recovery_reason: Option<TaskRecoveryReason>,
    pub retry_run_kind: Option<RunKind>,
    pub prompt_markdown: String,
    pub context_markdown: String,
    pub opened_by_actor_id: ActorId,
    pub originating_run_id: Option<RunId>,
    pub resolved_by_actor_id: Option<ActorId>,
    pub resolution_message_id: Option<TaskMessageId>,
    pub opened_at: Timestamp,
    pub resolved_at: Option<Timestamp>,
}

pub enum TaskMessageKind {
    HumanAnswer,
    HumanChangeRequest,
    RetryNote,
}

pub struct TaskMessageRecord {
    pub message_id: TaskMessageId,
    pub task_id: TaskId,
    pub task_generation: u64,
    pub contract_id: Option<TaskContractId>,
    pub gate_id: Option<TaskGateId>,
    pub review_id: Option<ReviewId>,
    pub kind: TaskMessageKind,
    pub body_markdown: String,
    pub approval_decision: Option<ApprovalDecision>,
    pub author_actor_id: ActorId,
    pub consumed_by_run_id: Option<RunId>,
    pub consumed_at: Option<Timestamp>,
    pub created_at: Timestamp,
}
```

Gate state belongs to a gate, not the task’s workflow state. At most one gate may be `Open` for a task. An answer or retry atomically resolves it, persists a message, moves the task to Queue, and queues a child continuation. Clarification/Approval use the role identified by `originating_run_id`: Planner resumes Planner, Executor resumes Executor, and Reviewer resumes Reviewer against the same submission. Recovery uses its explicit nullable `retry_run_kind`; Review-round exhaustion selects Executor, infrastructure exhaustion selects the failed role, and an invariant with no safe continuation leaves it null so Retry/Answer are not offered. The runtime marks the message consumed only after it has incorporated it at that safe run boundary; it never injects a human answer into an in-flight provider call.

Recovery actions are closed by reason: infrastructure or review exhaustion
offers Answer and Retry; unsafe-effect uncertainty offers Answer only;
configuration unavailability offers Retry only; invariant fault offers neither.
Cancel remains available for every open Recovery gate. The server derives
`ValidTaskAction` from this table rather than persisting a second attention
state or trusting UI wording.

An Approval gate requires `TaskGateAnswer.approval_decision`; the persisted answer stores `Approved` or `Declined` as structured data. The model may explain that decision in its message but can never infer approval authority from its wording.

`RunKind` becomes `Planner | Executor | Reviewer`. `RunStatus` remains the current lease-local vocabulary (`Queued`, `Leased`, `Running`, `Completed`, `WaitingForApproval`, `Interrupted`, `Failed`, `Cancelled`) and may not be projected back into a task field. Each run gains `task_generation` and optional `contract_id`:

- Planner runs have no contract ID, use the executor identity and exact model/policy snapshots, and may only submit a complete plan or a blocking question.
- Executor and reviewer runs require a contract ID and copy its model/policy snapshots into the immutable run record.
- Every run terminal write compares its lease token, run status, `task_generation`, and the task’s current generation. A stale run can add no submission, review, gate, stage transition, or notification.

Submissions, submission criterion evidence, artifact-version links, reviews, review criterion evidence, run transcripts, and usage stay immutable. A submission and each of its review attempts explicitly link to the contract ID they evaluated. Reviewer `needs_human` persists attempt 1; the post-answer Reviewer writes attempt 2 with `supersedes_review_id`, so uncertainty evidence is never updated in place. Existing artifacts remain task-owned and reviewers remain read-only.

## Event contract

`WorkEventKind` is a closed, fail-closed enum. Its persisted strings are:

```text
project.created             project.updated              project.archived
project.reopened            task.captured                task.updated
task.queued                 task.stage_changed           task.cancelled
task.reopened               task.accepted                contract.created
gate.opened                 gate.resolved                gate.superseded
task.message_appended       task.message_consumed         run.queued
run.claimed                 run.started                  run.heartbeat
run.completed               run.waiting_for_approval      run.interrupted
run.failed                  run.cancel_requested          run.cancelled
submission.created          review.created                notification.queued
notification.delivered      notification.failed
```

```rust
pub struct WorkEventRecord {
    pub event_id: WorkEventId,
    pub event_sequence: u64,
    pub kind: WorkEventKind,
    pub workspace_id: WorkspaceId,
    pub project_id: Option<ProjectId>,
    pub task_id: Option<TaskId>,
    pub run_id: Option<RunId>,
    pub actor_id: ActorId,
    pub causation_id: Option<String>,
    pub correlation_id: String,
    pub safe_payload: serde_json::Value,
    pub created_at: Timestamp,
}
```

The event payload begins with `{"v": 1}` and contains only redacted, UI-safe metadata. `event_sequence` is allocated only by SQLite’s global ledger and is the one cursor source for subscriptions and activity views. Event persistence, payload storage, and cursor encoding are specified in [storage and events](03-storage-and-events.md).

## Commands and version preconditions

All work mutations carry common causality metadata:

```rust
pub struct CommandMeta {
    pub actor_id: ActorId,
    pub causation_id: Option<String>,
    pub correlation_id: String,
    pub idempotency_key: Option<String>,
}

pub struct TaskPrecondition {
    pub task_id: TaskId,
    pub expected_revision: u64,
    pub expected_generation: u64,
}

pub struct ProjectPrecondition {
    pub project_id: ProjectId,
    pub expected_revision: u64,
}
```

The typed command enum and result are intentionally small at the public boundary:

```rust
pub enum WorkCommand {
    CaptureTask(CaptureTask),
    UpdateInboxTask(UpdateInboxTask),
    QueueTask(QueueTask),
    AnswerTask(AnswerTask),
    RetryTask(RetryTask),
    AcceptTask(AcceptTask),
    RequestTaskChanges(RequestTaskChanges),
    CancelTask(CancelTask),
    ReopenTask(ReopenTask),
    CreateProject(CreateProject),
    UpdateProject(UpdateProject),
    ArchiveProject(ArchiveProject),
    ReopenProject(ReopenProject),
    DelegateTask(DelegateTask), // primary-agent-only composition
}

pub struct WorkCommandResult {
    pub task: Option<TaskRecord>,
    pub project: Option<ProjectRecord>,
    pub contract_id: Option<TaskContractId>,
    pub gate_id: Option<TaskGateId>,
    pub run_id: Option<RunId>,
    pub event_id: WorkEventId,
    pub event_sequence: u64,
}
```

| Command input | Fields beyond `CommandMeta` and its precondition |
| --- | --- |
| `CaptureTask` | Workspace, title, description, optional project, provenance. |
| `UpdateInboxTask` | At least one replacement capture field: title, description, or optional project association. |
| `QueueTask` / `AcceptTask` / `ReopenTask` | No caller-controlled execution fields. |
| `AnswerTask` | Current gate ID and `TaskGateAnswer`. |
| `RetryTask` | Current Recovery gate ID and optional retry note. |
| `RequestTaskChanges` | `TaskContractAmendment`. |
| `CancelTask` | Optional safe cancellation reason. |
| `CreateProject` | Workspace, nonblank name, description. |
| `UpdateProject` | At least one replacement name/description. |
| `ArchiveProject` / `ReopenProject` | Project precondition only. |
| `DelegateTask` | Capture fields plus either no execution intent and an optional complexity hint, or one complete normalized request/criteria/complexity intent. Partial intent is invalid. Model/policy snapshots are resolved by the service. |

Every mutation has `CommandMeta`. Task-targeting commands require `TaskPrecondition`; task creation carries an explicit `expected_revision: None` and `expected_generation: None` in its serialized command envelope because no task exists yet. Project commands use their project precondition and no generation because projects do not own runs. `correlation_id` is required even for system work so a complete causal trail exists. An idempotency key is optional for human UI commands and required for provider tool execution and notification delivery.

The public command set is:

```text
CaptureTask               UpdateInboxTask          QueueTask
AnswerTask                RetryTask                AcceptTask
RequestTaskChanges        CancelTask               ReopenTask
CreateProject             UpdateProject            ArchiveProject
ReopenProject
```

`task.delegate` is a tool-level atomic composition of capture and queueing, with complete-contract creation when a complete intent is supplied. Without one, the same transaction queues Planner and creates no draft contract. It is represented internally as `DelegateTask`, but it does not introduce another workflow transition or task state. The command service returns the resulting task, its new revision/generation, affected run/gate IDs, and the last committed `WorkEventId`.

The exact allowed stages, effects, notifications, and idempotent replay behavior are specified in [command and reconciliation behavior](04-commands-and-reconciliation.md).

## Transition matrix

Only semantic commands and runtime terminal contracts may change `stage_id`. A generic `setStage`, raw SQL status write, drag-and-drop transition, or English-text inference is invalid.

| From behavior | Authorized actor/action | To behavior | Notes |
| --- | --- | --- | --- |
| none | `CaptureTask` | `Intake` | Creates an Inbox task with no run. |
| none | `DelegateTask` | `Dispatch` | Atomically queues Planner without a contract, or creates a complete contract and queues Executor. |
| `Intake` | `QueueTask` | `Dispatch` | Queues Planner; an Inbox task never has a current execution contract. |
| `Intake` | `UpdateInboxTask` | `Intake` | Updates capture fields only. |
| `Dispatch` | worker claims its queued run | `Active` | Claim and stage move are one fenced transaction. |
| `Active` | planner accepts plan | `Active` | Creates contract and queues Executor without changing stage. |
| `Active` | executor submits result | `Active` | Creates immutable submission and queues Reviewer. |
| `Active` | reviewer requests changes | `Active` | Queues another Executor on the same contract while review rounds remain. |
| `Active` | reviewer approves | `Acceptance` | Reviewer evidence is complete; human acceptance remains required. |
| `Active` | planner/executor/reviewer/failure opens gate | `HumanGate` | No runnable run remains. |
| `HumanGate` | `AnswerTask` or `RetryTask` | `Dispatch` | Resolves the gate and returns work to the reconciler. |
| `Acceptance` | `AcceptTask` | `TerminalSuccess` | Requires a valid approving review. |
| `Acceptance` | `RequestTaskChanges` | `Dispatch` | Creates a new immutable contract and increments generation. |
| `Intake`, `Dispatch`, `Active`, `HumanGate`, `Acceptance` | `CancelTask` | `TerminalCancelled` | Fences active/runnable runs. |
| `TerminalSuccess`, `TerminalCancelled` | `ReopenTask` | `Intake` | Preserves history, increments generation, and requires a new contract. |

There are no other task transitions. `Completed` is deliberately human acceptance, never a synonym for “executor finished” or “reviewer approved.”

## Editing and evidence rules

- `title`, `description_markdown`, and `project_id` are directly editable only in Inbox. `workspace_id` and `workflow_id` never change in V1.
- A Queue, Doing, Waiting, Review, Completed, or Cancelled task cannot be directly edited. Clarification and retry input become a `TaskMessageRecord`; a request for a changed result becomes a new contract through `RequestTaskChanges`.
- Execution contracts, criteria, model snapshots, policy snapshots, submission evidence, reviews, run transcripts, artifact snapshots, and work events are append-only. No command updates them in place.
- An approved review must cover each contract criterion exactly once and every outcome must be `Pass`. It cannot be accepted if a gate is open or if it refers to an old generation or contract.
- A task may have at most one queued, leased, or running run. A task in Waiting, Review, Completed, or Cancelled has no runnable run.
- Cancellation, human contract revision, and reopening increment generation. Cancellation marks every queued, leased, or running run Cancelled and signals any in-flight holder; old runs remain readable evidence but are fence-rejected if they later report a terminal result.
- A project is a container, not an agent, scope, memory store, tool grant, or external-authority grant.

## Stable validation failures

`WorkDomainError` is the domain-safe error vocabulary. Store, runtime, GraphQL, and tools may add contextual diagnostics but must preserve these stable codes.

| Code | Meaning |
| --- | --- |
| `work_unavailable` | The task, project, workflow, or workspace does not exist or is outside the actor’s visible scope. |
| `stale_revision` | The supplied optimistic revision does not match the current row. |
| `stale_generation` | The supplied task generation does not match the current execution cycle. |
| `invalid_transition` | The command is not authorized from the task’s current stage behavior. |
| `workflow_mismatch` | The requested stage is not part of the task’s workflow. |
| `project_archived` | A new or moved Inbox task targets an archived project. |
| `contract_required` | An operation requires a complete current contract. |
| `contract_immutable` | A caller attempted to mutate an immutable contract or criterion set. |
| `gate_required` | Waiting resolution was attempted without the current open gate. |
| `gate_unresolved` | An operation cannot proceed while a gate is open. |
| `review_not_approved` | Acceptance was attempted without a complete approving review. |
| `review_limit_reached` | Another automated revision would exceed the contract’s bound. |
| `configuration_unavailable` | Required executor/reviewer selection or policy is unavailable for a new contract/run. |
| `run_fenced` | A lease, run generation, cancellation, or task generation check failed. |
| `idempotency_conflict` | An idempotency key was replayed with different command content. |
| `invalid_input` | Required text, identifiers, criteria, bounds, or enum values were malformed. |

Errors never reveal provider credentials, raw tool payloads, hidden reasoning, or internal SQLite details.
