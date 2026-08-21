export type TaskStatus =
  | "queued"
  | "executing"
  | "reviewing"
  | "revision_requested"
  | "waiting_for_human"
  | "done"
  | "failed"
  | "cancelled";

export type TaskStageBehavior =
  | "ACTIVE"
  | "DISPATCH"
  | "HUMAN_GATE"
  | "INTAKE"
  | "TERMINAL_CANCELLED"
  | "TERMINAL_SUCCESS";

export type TaskRunRole = "planner" | "executor" | "reviewer";

export type TaskRunStatus =
  | "queued"
  | "leased"
  | "running"
  | "completed"
  | "waiting_for_approval"
  | "interrupted"
  | "failed"
  | "cancelled";

export type TaskAttentionKind =
  | "CLARIFICATION_REQUIRED"
  | "APPROVAL_REQUIRED"
  | "RECOVERY_REQUIRED";

export type TaskModelSnapshot = {
  providerDisplayName?: string | null;
  modelProfile: string;
  modelLabel?: string | null;
  reasoningEffort?: string | null;
  inherited?: boolean;
};

export type TaskToolActivity = {
  id: string;
  title: string;
  summary?: string | null;
  status?: "running" | "completed" | "failed" | null;
  occurredAt?: string | null;
};

export type TaskRunItem = {
  id: string;
  runId?: string | null;
  sequenceIndex?: number | null;
  roundIndex?: number | null;
  sourceKind?: string;
  kind: "tool" | "message" | "result" | "artifact" | "status";
  title: string;
  summary?: string | null;
  details?: string | null;
  payload?: unknown;
  role?: TaskRunRole;
  status?: "queued" | "running" | "completed" | "failed" | "cancelled" | "skipped" | null;
  correlationId?: string | null;
  parentItemId?: string | null;
  responseIndex?: number | null;
  occurredAt?: string | null;
  updatedAt?: string | null;
};

export type TaskRun = {
  id: string;
  instanceName: string;
  role: TaskRunRole;
  status: TaskRunStatus;
  revision?: number | null;
  attemptIndex: number;
  isLatest?: boolean;
  model?: TaskModelSnapshot | null;
  output?: string | null;
  error?: string | null;
  toolActivities?: readonly TaskToolActivity[];
  items?: readonly TaskRunItem[];
  providerCallCount?: number | null;
  toolCallCount?: number | null;
  inputTokens?: number | null;
  cachedInputTokens?: number | null;
  outputTokens?: number | null;
  activeMilliseconds?: number | null;
  executionPolicy?: {
    maxProviderContinuations: number;
    maxToolCalls: number;
    maxActiveMinutes: number;
    progressAuditInterval: number;
  } | null;
  startedAt?: string | null;
  completedAt?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
};

export type TaskRevision = {
  revision: number;
  executors: readonly TaskRun[];
  reviewers: readonly TaskRun[];
  latestRunId?: string | null;
};

export type TaskWorkspaceFile = {
  path: string;
  isDirectory: boolean;
  sizeBytes?: number | null;
};

export type TaskDetail = {
  taskId: string;
  title: string;
  schedule?: {
    scheduledFor: string;
    timeZone: string;
    missedRunPolicy: "RUN_ONCE" | "SKIP";
    recurrenceId?: string | null;
    recurrenceRevision?: number | null;
    recurrenceScheduledFor?: string | null;
  } | null;
  status: TaskStatus;
  stageBehavior: TaskStageBehavior;
  capturedRequest: string;
  taskDocument: string;
  resultDocument?: string | null;
  resultMetadata?: unknown;
  reviewDocument?: string | null;
  workspaceFiles: readonly TaskWorkspaceFile[];
  workspaceFilesTruncated: boolean;
  createdAt?: string | null;
  updatedAt?: string | null;
  createdBy?: string | null;
  sourceLabel?: string | null;
  currentRevision?: number | null;
  executorModel?: TaskModelSnapshot | null;
  reviewerModel?: TaskModelSnapshot | null;
  reviewerModelInherited?: boolean;
  revisions: readonly TaskRevision[];
  contributorInstanceNames: readonly string[];
  canCancel?: boolean;
  canResume?: boolean;
  blockingQuestion?: string | null;
  attention?: {
    kind: TaskAttentionKind;
    title: string;
    summary: string;
    context?: string | null;
  } | null;
  messages?: readonly { id: string; author: string; body: string; createdAt: string }[];
};
