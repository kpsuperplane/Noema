import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import * as React from "react";
import {
  CancelTaskDocument,
  ResumeTaskDocument,
  TaskDetailDocument,
  TaskEventsDocument,
  type TaskDetailQuery
} from "@/generated/graphql";
import { TaskDetailPanel } from "./TaskDetailPanel";
import { mapTaskRunItem, mergeTaskRunItems } from "./taskRunItemMapper";
import type {
  TaskComplexity,
  TaskCriterion,
  TaskCriterionVerdict,
  TaskDetail,
  TaskModelSnapshot,
  TaskReview,
  TaskRevision,
  TaskRun,
  TaskRunItem,
  TaskRunRole,
  TaskRunStatus,
  TaskSubmission,
  TaskStatus
} from "./taskTypes";

type GraphqlTaskDetail = NonNullable<TaskDetailQuery["task"]>;
const taskEventCursorByTask = new Map<string, string>();

export function TaskDetailQueryPanel({
  taskId,
  onTitleChange,
  onCancelTask,
  onResumeTask,
  onExpandRevision
}: {
  taskId: string;
  onTitleChange?: (title: string | null) => void;
  onCancelTask?: (taskId: string) => void | Promise<void>;
  onResumeTask?: (taskId: string, message?: string) => void | Promise<void>;
  onExpandRevision?: (taskId: string, revision: number) => void;
}) {
  const { data, error, loading, refetch } = useQuery(TaskDetailDocument, {
    fetchPolicy: "cache-and-network",
    variables: { taskId }
  });
  const [cancelTask] = useMutation(CancelTaskDocument);
  const [resumeTask] = useMutation(ResumeTaskDocument);
  const [liveState, setLiveState] = React.useState<{
    taskId: string;
    items: Map<string, readonly TaskRunItem[]>;
  }>(() => ({ taskId, items: new Map() }));
  const refreshTimer = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const detail = data?.task ? mapGraphqlTaskDetail(data.task) : null;
  useSubscription(TaskEventsDocument, {
    variables: { taskId, after: taskEventCursorByTask.get(taskId) ?? null },
    skip: Boolean(detail && isTerminalTaskStatus(detail.status)),
    onData: ({ data: result }) => {
      const event = result.data?.taskEvents;
      if (!event) {
        return;
      }
      taskEventCursorByTask.set(taskId, event.cursor);
      if (event.item && event.runId) {
        const run = data?.task?.runs.find((candidate) => candidate.runId === event.runId);
        const mapped = mapTaskRunItem(event.item, runRole(run?.runKind ?? "executor"));
        setLiveState((previous) => {
          const items = previous.taskId === taskId ? new Map(previous.items) : new Map();
          items.set(event.runId!, mergeTaskRunItems(items.get(event.runId!) ?? [], [mapped]));
          return { taskId, items };
        });
      }
      if (String(event.kind).toLowerCase() === "run_item_upserted") {
        return;
      }
      if (refreshTimer.current) {
        clearTimeout(refreshTimer.current);
      }
      refreshTimer.current = setTimeout(() => {
        refreshTimer.current = null;
        void refetch();
      }, 120);
    }
  });
  const liveRunItems = liveState.taskId === taskId ? liveState.items : new Map();

  const handleResumeTask = React.useCallback(
    async (resumeTaskId: string, message?: string) => {
      if (onResumeTask) {
        await onResumeTask(resumeTaskId, message);
      } else {
        await resumeTask({ variables: { taskId: resumeTaskId, message: message ?? null } });
      }
      await refetch();
    },
    [onResumeTask, refetch, resumeTask]
  );

  const handleCancelTask = React.useCallback(
    async (cancelTaskId: string) => {
      if (onCancelTask) {
        await onCancelTask(cancelTaskId);
      } else {
        await cancelTask({ variables: { taskId: cancelTaskId } });
      }
      await refetch();
    },
    [cancelTask, onCancelTask, refetch]
  );

  React.useEffect(() => {
    return () => {
      if (refreshTimer.current) {
        clearTimeout(refreshTimer.current);
        refreshTimer.current = null;
      }
    };
  }, []);

  React.useEffect(() => {
    onTitleChange?.(data?.task?.title ?? null);
  }, [data?.task?.title, onTitleChange]);

  return (
    <TaskDetailPanel
      detail={detail}
      error={error?.message ?? null}
      liveRunItems={liveRunItems}
      loading={loading}
      onCancelTask={handleCancelTask}
      onExpandRevision={onExpandRevision}
      onResumeTask={handleResumeTask}
      taskId={taskId}
    />
  );
}

function isTerminalTaskStatus(status: TaskStatus): boolean {
  return status === "completed" || status === "failed" || status === "cancelled";
}

export function mapGraphqlTaskDetail(detail: GraphqlTaskDetail): TaskDetail {
  const submissions = detail.submissions.map(mapSubmission);
  const reviews = detail.reviews.map(mapReview);
  const runs = detail.runs.map(mapRun);
  const submissionById = new Map(submissions.map((submission) => [submission.id, submission]));
  const reviewBySubmissionId = new Map(
    detail.reviews.map((review, index) => [review.reviewedSubmissionId, reviews[index]])
  );
  const reviewByReviewerRunId = new Map(
    detail.reviews.map((review, index) => [review.reviewerRunId, reviews[index]])
  );
  const runById = new Map(runs.map((run) => [run.id, run]));
  const revisionIndexes = new Set<number>([detail.revisionIndex]);
  for (const submission of submissions) {
    if (typeof submission.revision === "number") {
      revisionIndexes.add(submission.revision);
    }
  }
  for (const run of runs) {
    revisionIndexes.add(run.revision ?? 0);
  }
  for (const review of detail.reviews) {
    const reviewerRun = runById.get(review.reviewerRunId);
    if (reviewerRun) {
      revisionIndexes.add(reviewerRun.revision ?? 0);
    }
  }

  const latestReviewCriteria = latestReviewCriteriaById(detail.reviews);
  const latestSubmissionCriteria = latestSubmissionCriteriaById(detail.submissions);
  const criteria: TaskCriterion[] = detail.criteria.map((criterion) => {
    const reviewed = latestReviewCriteria.get(criterion.criterionId);
    const submitted = latestSubmissionCriteria.get(criterion.criterionId);
    return {
      id: criterion.criterionId,
      position: criterion.ordinal,
      text: criterion.description,
      expectedEvidence: criterion.expectedEvidence,
      verdict: reviewed ? criterionVerdict(reviewed.outcome) : "pending",
      evidence: reviewed?.evidenceMarkdown ?? submitted?.evidenceMarkdown ?? null
    };
  });

  return {
    taskId: detail.taskId,
    title: detail.title,
    status: taskStatus(detail.status),
    complexity: taskComplexity(detail.complexity),
    request: detail.requestMarkdown,
    criteria,
    createdAt: detail.createdAt,
    updatedAt: detail.updatedAt,
    createdBy: detail.createdByAgentId,
    sourceLabel: detail.source.conversationId ? `Conversation ${detail.source.conversationId}` : null,
    currentRevision: detail.revisionIndex,
    maxReviewRounds: detail.maxReviewRounds,
    executorModel: modelSnapshot(detail.executorModel),
    reviewerModel: modelSnapshot(detail.reviewerModel),
    reviewerModelInherited: inheritedModel(detail.reviewerModel),
    revisions: [...revisionIndexes]
      .sort((left, right) => left - right)
      .map((revision) =>
        revisionDetail(revision, submissions, runs, reviewBySubmissionId, reviewByReviewerRunId)
      ),
    finalResult: finalResult(detail.finalSubmissionId, submissionById, detail.reviews),
    artifacts: [],
    delivery: null,
    failureReason: failureReason(detail),
    blockingQuestion: detail.blockingQuestion,
    canCancel: detail.cancellable,
    canResume: detail.resumable
  };
}

function revisionDetail(
  revision: number,
  submissions: readonly TaskSubmission[],
  runs: readonly TaskRun[],
  reviewBySubmissionId: ReadonlyMap<string, TaskReview>,
  reviewByReviewerRunId: ReadonlyMap<string, TaskReview>
): TaskRevision {
  const executor = runs.find((run) => run.role === "executor" && run.revision === revision) ?? null;
  const reviewer = runs.find((run) => run.role === "reviewer" && run.revision === revision) ?? null;
  const submission = submissions.find((candidate) => candidate.revision === revision) ?? null;
  const review = submission
    ? reviewBySubmissionId.get(submission.id) ?? null
    : reviewer
      ? reviewByReviewerRunId.get(reviewer.id) ?? null
      : null;
  return {
    revision,
    executor,
    submission,
    reviewer,
    review
  };
}

function mapSubmission(
  submission: GraphqlTaskDetail["submissions"][number]
): TaskSubmission {
  return {
    id: submission.submissionId,
    revision: submission.revisionIndex,
    summary: submission.summary,
    result: submission.resultMarkdown,
    evidence: submission.criteria
      .map((criterion) => `${criterion.criterionId}: ${criterion.evidenceMarkdown}`)
      .join("\n\n"),
    createdAt: submission.createdAt
  };
}

function mapReview(review: GraphqlTaskDetail["reviews"][number]): TaskReview {
  return {
    id: review.reviewId,
    verdict: reviewVerdict(review.overallVerdict),
    summary: review.overallFeedback,
    criteria: review.criteria.map((criterion) => ({
      criterionId: criterion.criterionId,
      verdict: criterionVerdict(criterion.outcome),
      feedback: criterion.feedback,
      evidence: criterion.evidenceMarkdown
    })),
    createdAt: review.createdAt
  };
}

function mapRun(run: GraphqlTaskDetail["runs"][number]): TaskRun {
  const role = runRole(run.runKind);
  return {
    id: run.runId,
    role,
    status: runStatus(run.status),
    revision: run.revisionIndex,
    model: modelSnapshot(run.model),
    error: [run.errorCode, run.errorMessage].filter(Boolean).join(" · ") || null,
    providerCallCount: run.providerCallCount,
    toolCallCount: run.toolCallCount,
    inputTokens: run.inputTokens,
    cachedInputTokens: run.cachedInputTokens,
    outputTokens: run.outputTokens,
    activeMilliseconds: run.activeMilliseconds,
    executionPolicy: run.executionPolicy,
    startedAt: run.startedAt,
    completedAt: run.endedAt
  };
}

function modelSnapshot(snapshot: GraphqlTaskDetail["executorModel"]): TaskModelSnapshot {
  const modelProfile = snapshot.modelProfile ?? `${snapshot.providerKind} default`;
  return {
    providerDisplayName: snapshot.providerKind,
    modelProfile,
    modelLabel: snapshot.modelProfile ?? modelProfile,
    reasoningEffort: snapshot.reasoningEffort ? String(snapshot.reasoningEffort) : null,
    inherited: inheritedModel(snapshot)
  };
}

function inheritedModel(snapshot: GraphqlTaskDetail["executorModel"]): boolean {
  return snapshot.selectionMode === "provider_default";
}

function finalResult(
  finalSubmissionId: string | null,
  submissionById: ReadonlyMap<string, TaskSubmission>,
  reviews: GraphqlTaskDetail["reviews"]
): TaskDetail["finalResult"] {
  if (!finalSubmissionId) {
    return null;
  }
  const submission = submissionById.get(finalSubmissionId);
  if (!submission) {
    return null;
  }
  const approval = reviews.find(
    (review) => review.reviewedSubmissionId === finalSubmissionId && review.overallVerdict === "approve"
  );
  return {
    summary: submission.summary,
    body: submission.result,
    approvedAt: approval?.createdAt ?? submission.createdAt
  };
}

function failureReason(detail: GraphqlTaskDetail): string | null {
  const parts = [detail.errorMessage, detail.terminalReason, detail.errorCode].filter(
    (part): part is string => Boolean(part?.trim())
  );
  return parts.length > 0 ? [...new Set(parts)].join(" · ") : null;
}

function latestReviewCriteriaById(
  reviews: GraphqlTaskDetail["reviews"]
): Map<string, GraphqlTaskDetail["reviews"][number]["criteria"][number]> {
  const values = new Map<string, GraphqlTaskDetail["reviews"][number]["criteria"][number]>();
  for (const review of reviews) {
    for (const criterion of review.criteria) {
      values.set(criterion.criterionId, criterion);
    }
  }
  return values;
}

function latestSubmissionCriteriaById(
  submissions: GraphqlTaskDetail["submissions"]
): Map<string, GraphqlTaskDetail["submissions"][number]["criteria"][number]> {
  const values = new Map<string, GraphqlTaskDetail["submissions"][number]["criteria"][number]>();
  for (const submission of submissions) {
    for (const criterion of submission.criteria) {
      values.set(criterion.criterionId, criterion);
    }
  }
  return values;
}

function taskStatus(value: string): TaskStatus {
  switch (value) {
    case "queued":
    case "executing":
    case "reviewing":
    case "revision_requested":
    case "waiting_for_human":
    case "completed":
    case "failed":
    case "cancelled":
      return value;
    default:
      return "failed";
  }
}

function taskComplexity(value: string): TaskComplexity {
  switch (value.toLowerCase()) {
    case "simple":
      return "simple";
    case "difficult":
      return "difficult";
    default:
      return "medium";
  }
}

function runRole(value: string): TaskRunRole {
  switch (value) {
    case "reviewer":
      return "reviewer";
    case "completion_delivery":
      return "completion_delivery";
    default:
      return "executor";
  }
}

function runStatus(value: string): TaskRunStatus {
  switch (value) {
    case "queued":
    case "leased":
    case "running":
    case "completed":
    case "waiting_for_approval":
    case "interrupted":
    case "failed":
    case "cancelled":
      return value;
    default:
      return "failed";
  }
}

function criterionVerdict(value: string): Exclude<TaskCriterionVerdict, "pending"> {
  switch (value) {
    case "pass":
    case "fail":
    case "uncertain":
      return value;
    default:
      return "uncertain";
  }
}

function reviewVerdict(value: string): TaskReview["verdict"] {
  switch (value) {
    case "approve":
    case "request_changes":
    case "needs_human":
      return value;
    default:
      return "needs_human";
  }
}
