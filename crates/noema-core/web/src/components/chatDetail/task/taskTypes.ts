export type TaskStatus =
  | "queued"
  | "executing"
  | "reviewing"
  | "revision_requested"
  | "waiting_for_human"
  | "completed"
  | "failed"
  | "cancelled";

export type TaskComplexity = "simple" | "medium" | "difficult";

export type TaskRunRole = "executor" | "reviewer" | "completion_delivery";

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

export type TaskDeliveryStatus = "pending" | "delivered" | "failed" | "not_required";

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
  revision?: number | null;
  summary?: string | null;
  result?: string | null;
  evidence?: string | null;
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
  verdict: TaskReviewVerdict;
  summary?: string | null;
  criteria: readonly TaskCriterionReview[];
  createdAt?: string | null;
};

export type TaskRunItem = {
  id: string;
  kind: "input" | "tool" | "message" | "result" | "artifact" | "status";
  title: string;
  summary?: string | null;
  details?: string | null;
  role?: TaskRunRole;
  status?: "running" | "completed" | "failed" | null;
  occurredAt?: string | null;
};

export type TaskRun = {
  id: string;
  role: TaskRunRole;
  status: TaskRunStatus;
  revision?: number | null;
  model?: TaskModelSnapshot | null;
  output?: string | null;
  error?: string | null;
  toolActivities?: readonly TaskToolActivity[];
  items?: readonly TaskRunItem[];
  startedAt?: string | null;
  completedAt?: string | null;
};

export type TaskRevision = {
  revision: number;
  executor?: TaskRun | null;
  submission?: TaskSubmission | null;
  reviewer?: TaskRun | null;
  review?: TaskReview | null;
  items?: readonly TaskRunItem[];
  itemsLoading?: boolean;
  itemsError?: string | null;
  hasMoreItems?: boolean;
};

export type TaskArtifact = {
  id: string;
  title: string;
  kind?: string | null;
  mediaType?: string | null;
  href?: string | null;
};

export type TaskFinalResult = {
  summary?: string | null;
  body?: string | null;
  approvedAt?: string | null;
};

export type TaskDelivery = {
  status: TaskDeliveryStatus;
  deliveredAt?: string | null;
  destination?: string | null;
  error?: string | null;
};

export type TaskDetail = {
  taskId: string;
  title: string;
  status: TaskStatus;
  complexity: TaskComplexity;
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
  finalResult?: TaskFinalResult | null;
  artifacts?: readonly TaskArtifact[];
  delivery?: TaskDelivery | null;
  failureReason?: string | null;
  canCancel?: boolean;
  canRetry?: boolean;
};
