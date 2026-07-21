export type TaskStatus =
  | "queued"
  | "executing"
  | "reviewing"
  | "revision_requested"
  | "waiting_for_human"
  | "done"
  | "failed"
  | "cancelled";

export type TaskComplexity = "simple" | "medium" | "difficult";

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

export type TaskCriterionVerdict = "pending" | "pass" | "fail" | "uncertain";

export type TaskReviewVerdict = "approve" | "request_changes" | "needs_human";

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

export type TaskCriterion = {
  id: string;
  position: number;
  text: string;
  expectedEvidence?: string | null;
  verdict?: TaskCriterionVerdict | null;
  evidence?: string | null;
};

export type TaskToolActivity = {
  id: string;
  title: string;
  summary?: string | null;
  status?: "running" | "completed" | "failed" | null;
  occurredAt?: string | null;
};

export type TaskSubmission = {
  id: string;
  executorRunId: string;
  revision?: number | null;
  summary?: string | null;
  result?: string | null;
  evidence?: string | null;
  artifacts?: readonly TaskArtifact[];
  createdAt?: string | null;
};

export type TaskCriterionReview = {
  criterionId: string;
  verdict: Exclude<TaskCriterionVerdict, "pending">;
  feedback?: string | null;
  evidence?: string | null;
};

export type TaskReview = {
  id: string;
  reviewerRunId?: string | null;
  verdict: TaskReviewVerdict;
  summary?: string | null;
  criteria: readonly TaskCriterionReview[];
  createdAt?: string | null;
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
  submission?: TaskSubmission | null;
  reviewers: readonly TaskRun[];
  review?: TaskReview | null;
  latestRunId?: string | null;
};

export type TaskArtifact = {
  id: string;
  versionId?: string | null;
  title: string;
  kind?: string | null;
  storageKind?: "external_url" | "local_file" | null;
  mediaType?: string | null;
  downloadUrl?: string | null;
  externalUrl?: string | null;
};

export type TaskDetail = {
  taskId: string;
  title: string;
  status: TaskStatus;
  stageBehavior: TaskStageBehavior;
  complexity: TaskComplexity | null;
  request: string;
  criteria: readonly TaskCriterion[];
  createdAt?: string | null;
  updatedAt?: string | null;
  createdBy?: string | null;
  sourceLabel?: string | null;
  currentRevision?: number | null;
  maxReviewRounds?: number | null;
  executorModel?: TaskModelSnapshot | null;
  reviewerModel?: TaskModelSnapshot | null;
  reviewerModelInherited?: boolean;
  revisions: readonly TaskRevision[];
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
