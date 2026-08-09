import * as React from "react";
import { useQuery, useSubscription } from "@apollo/client/react";
import {
  TasksTaskDetailDocument,
  TasksTaskEventsDocument,
  type TasksTaskDetailQuery
} from "@/generated/graphql";
import { TaskActions } from "@/components/tasks/TaskActions";
import { PendingHumanInterventions } from "@/components/actions/PendingGovernedActions";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { useTaskProjects } from "@/components/tasks/useTaskProjects";
import { useTaskEventCursor } from "./taskEventCursor";
import { TaskDetailPanel } from "./TaskDetailPanel";
import { taskStatusFromProjection } from "./TaskStatusBadge";
import type {
  TaskCriterion,
  TaskDetail,
  TaskReview,
  TaskRevision,
  TaskRun,
  TaskRunRole,
  TaskRunStatus,
  TaskSubmission
} from "./taskTypes";

type TasksDetail = NonNullable<TasksTaskDetailQuery["task"]>;

export function TaskDetailQueryPanel({
  taskId,
  showTasksLink = true,
  onOpenDetail,
  onTaskTitleChange
}: {
  taskId: string;
  showTasksLink?: boolean;
  onOpenDetail: (target: ChatDetailTarget) => void;
  onTaskTitleChange?: (title: string) => void;
}) {
  const result = useQuery(TasksTaskDetailDocument, {
    variables: { taskId },
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true
  });
  const projects = useTaskProjects();
  const queriedTask = result.data?.task ?? null;
  const task = queriedTask?.taskId === taskId ? queriedTask : null;
  const [cursor, recordCursor] = useTaskEventCursor(taskId);
  React.useEffect(() => {
    if (task?.title) onTaskTitleChange?.(task.title);
  }, [onTaskTitleChange, task?.title]);

  useSubscription(TasksTaskEventsDocument, {
    variables: { taskId, after: cursor },
    skip: task ? terminalBehavior(task.stage.behavior) : false,
    onData: ({ data }) => {
      const event = data.data?.taskEvents;
      if (!event) return;
      recordCursor(event.cursor);
      void result.refetch();
    }
  });

  const detail = React.useMemo(() => task ? mapTaskDetail(task) : null, [task]);
  const taskControls = task?.validActions.filter((action) => action !== "ANSWER" && action !== "RETRY") ?? [];
  const renderPanel = (_actions?: React.ReactNode, controls?: React.ReactNode) => (
    <>
      <TaskDetailPanel
        detail={detail}
        error={result.error ? "Task details could not be loaded." : null}
        governedActions={<PendingHumanInterventions placement="dock" taskId={taskId} />}
        loading={result.loading}
        onOpenDetail={onOpenDetail}
        controls={controls}
        showTasksLink={showTasksLink}
        taskId={taskId}
      />
    </>
  );

  if (task) {
    return (
      <TaskActions
        task={task}
        validActions={taskControls}
        projects={projects.projects}
        onUpdated={async () => { await result.refetch(); }}
      >
        {renderPanel}
      </TaskActions>
    );
  }

  return renderPanel();
}

function mapTaskDetail(task: TasksDetail): TaskDetail {
  const runs = task.runs.map(mapRun);
  const submissions = task.submissions.map(mapSubmission);
  const reviews = task.reviews.map(mapReview);
  const reviewBySubmission = new Map<string, TaskReview>();
  task.reviews.forEach((review, index) => {
    if (!reviewBySubmission.has(review.reviewedSubmissionId)) {
      reviewBySubmission.set(review.reviewedSubmissionId, reviews[index]);
    }
  });
  const revisionNumbers = new Set<number>([0, ...runs.map((run) => run.revision ?? 0)]);
  for (const submission of submissions) revisionNumbers.add(submission.revision ?? 0);
  const revisions: TaskRevision[] = [...revisionNumbers]
    .sort((left, right) => left - right)
    .map((revision) => {
      const revisionRuns = runs.filter((run) => (run.revision ?? 0) === revision);
      const submission = submissions.find((item) => item.revision === revision) ?? null;
      return {
        revision,
        executors: revisionRuns.filter((run) => run.role !== "reviewer"),
        submission,
        reviewers: revisionRuns.filter((run) => run.role === "reviewer"),
        review: submission ? reviewBySubmission.get(submission.id) ?? null : null,
        latestRunId: task.currentRun?.runId ?? null
      };
    });

  const reviewedCriteria = new Map<string, TasksDetail["reviews"][number]["criteria"][number]>();
  for (const review of task.reviews) {
    for (const criterion of review.criteria) {
      if (!reviewedCriteria.has(criterion.criterionId)) reviewedCriteria.set(criterion.criterionId, criterion);
    }
  }
  const submittedCriteria = new Map<string, TasksDetail["submissions"][number]["criteria"][number]>();
  for (const submission of task.submissions) {
    for (const criterion of submission.criteria) {
      if (!submittedCriteria.has(criterion.criterionId)) submittedCriteria.set(criterion.criterionId, criterion);
    }
  }
  const criteria: TaskCriterion[] = (task.currentContract?.criteria ?? []).map((criterion) => {
    const reviewed = reviewedCriteria.get(criterion.criterionId);
    const submitted = submittedCriteria.get(criterion.criterionId);
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
    taskId: task.taskId,
    title: task.title,
    schedule: task.schedule,
    status: taskStatusFromProjection(task),
    stageBehavior: task.stage.behavior,
    complexity: task.currentContract
      ? task.currentContract.complexity.toLowerCase() as TaskDetail["complexity"]
      : null,
    capturedRequest: task.description.trim() || task.title,
    criteria,
    createdAt: task.createdAt,
    updatedAt: task.updatedAt,
    sourceLabel: sourceLabel(task),
    currentRevision: task.generation,
    maxReviewRounds: task.currentContract?.executionPolicy.maxReviewRounds ?? null,
    completedResult: task.completedResult ? mapSubmission(task.completedResult) : null,
    revisions,
    canCancel: task.validActions.includes("CANCEL"),
    canResume: false,
    blockingQuestion: task.activeGate?.prompt ?? null,
    attention: task.attention ? {
      kind: task.attention.kind,
      title: task.attention.title,
      summary: task.attention.summary,
      context: task.activeGate?.contextMarkdown ?? null
    } : null,
    messages: task.messages.map((message) => ({
      id: message.messageId,
      author: message.author,
      body: message.bodyMarkdown,
      createdAt: message.createdAt
    }))
  };
}

function mapRun(run: TasksDetail["runs"][number]): TaskRun {
  return {
    id: run.runId,
    instanceName: run.instanceName,
    role: run.kind.toLowerCase() as TaskRunRole,
    status: run.status.toLowerCase() as TaskRunStatus,
    revision: run.reviewRound,
    attemptIndex: run.attemptIndex,
    model: {
      providerDisplayName: run.actualProviderKind ?? run.model.providerKind,
      modelProfile: run.actualModelProfile ?? run.model.modelProfile ?? "Provider default",
      reasoningEffort: run.model.reasoningEffort
    },
    error: [run.errorCode, run.errorMessage].filter(Boolean).join(" · ") || null,
    providerCallCount: run.providerCallCount,
    toolCallCount: run.toolCallCount,
    inputTokens: run.inputTokens,
    cachedInputTokens: run.cachedInputTokens,
    outputTokens: run.outputTokens,
    activeMilliseconds: run.activeMilliseconds,
    executionPolicy: run.executionPolicy,
    startedAt: run.startedAt,
    completedAt: run.endedAt,
    createdAt: run.createdAt,
    updatedAt: run.updatedAt
  };
}

function mapSubmission(
  submission: TasksDetail["submissions"][number]
): TaskSubmission {
  return {
    id: submission.submissionId,
    executorRunId: submission.executorRunId,
    revision: submission.reviewRound,
    summary: submission.summary,
    result: submission.resultMarkdown,
    evidence: submission.criteria.map((criterion) => criterion.evidenceMarkdown).join("\n\n"),
    artifacts: submission.artifacts.map((artifact) => ({
      id: artifact.artifactId,
      versionId: artifact.artifactVersionId,
      title: artifact.title,
      kind: artifact.artifactKind,
      storageKind: artifact.storageKind === "EXTERNAL_URL" ? "external_url" : "local_file",
      mediaType: artifact.mediaType,
      downloadUrl: artifact.downloadUrl,
      externalUrl: artifact.externalUrl
    })),
    createdAt: submission.createdAt
  };
}

function mapReview(review: TasksDetail["reviews"][number]): TaskReview {
  return {
    id: review.reviewId,
    reviewerRunId: review.reviewerRunId,
    verdict: review.verdict.toLowerCase() as TaskReview["verdict"],
    summary: review.feedback,
    criteria: review.criteria.map((criterion) => ({
      criterionId: criterion.criterionId,
      verdict: criterionVerdict(criterion.outcome),
      feedback: criterion.feedback,
      evidence: criterion.evidenceMarkdown
    })),
    createdAt: review.createdAt
  };
}

function sourceLabel(task: TasksDetail): string | null {
  if (task.project) return task.project.name;
  if (task.source.conversationId) return "Conversation";
  return null;
}

function criterionVerdict(value: string): "pass" | "fail" | "uncertain" {
  if (value === "PASS") return "pass";
  if (value === "FAIL") return "fail";
  return "uncertain";
}

function terminalBehavior(value: TasksDetail["stage"]["behavior"]): boolean {
  return value === "TERMINAL_SUCCESS" || value === "TERMINAL_CANCELLED";
}
