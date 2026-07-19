import * as React from "react";
import { useQuery, useSubscription } from "@apollo/client/react";
import {
  WorkTaskDetailDocument,
  WorkTaskEventsDocument,
  type WorkTaskDetailQuery
} from "@/generated/graphql";
import { TaskActions, TaskNavigationControls } from "@/components/work/TaskActions";
import { useAllWorkProjects } from "@/components/work/useAllWorkProjects";
import { useTaskEventCursor } from "./taskEventCursor";
import { TaskDetailPanel } from "./TaskDetailPanel";
import type {
  TaskCriterion,
  TaskDetail,
  TaskReview,
  TaskRevision,
  TaskRun,
  TaskRunRole,
  TaskRunStatus,
  TaskStatus,
  TaskSubmission
} from "./taskTypes";

type WorkDetail = NonNullable<WorkTaskDetailQuery["task"]>;

export function TaskDetailQueryPanel({
  taskId,
  onTitleChange,
  onClose,
  closeButtonRef,
  controlsHostRef,
  showWorkLink = true
}: {
  taskId: string;
  onTitleChange?: (title: string | null) => void;
  onClose: () => void;
  closeButtonRef?: React.RefObject<HTMLButtonElement | null>;
  controlsHostRef: React.RefObject<HTMLDivElement | null>;
  showWorkLink?: boolean;
}) {
  const result = useQuery(WorkTaskDetailDocument, {
    variables: { taskId },
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true
  });
  const projects = useAllWorkProjects();
  const task = result.data?.task ?? null;
  const [cursor, recordCursor] = useTaskEventCursor(taskId);

  useSubscription(WorkTaskEventsDocument, {
    variables: { taskId, after: cursor },
    skip: task ? terminalBehavior(task.stage.behavior) : false,
    onData: ({ data }) => {
      const event = data.data?.taskEvents;
      if (!event) return;
      recordCursor(event.cursor);
      void result.refetch();
    }
  });

  React.useEffect(() => {
    onTitleChange?.(task?.title ?? null);
  }, [onTitleChange, task?.title]);

  const detail = React.useMemo(() => task ? mapWorkTaskDetail(task) : null, [task]);
  const navigation = { taskId, showWorkLink, onClose };
  const decision = detail?.attention ? {
    attention: detail.attention,
    question: detail.blockingQuestion
  } : undefined;
  const actions = task ? (
    <TaskActions
      compact
      closeButtonRef={closeButtonRef}
      controlsHostRef={controlsHostRef}
      decision={decision}
      inlineAnswer={Boolean(decision && task.validActions.includes("ANSWER"))}
      navigation={navigation}
      task={task}
      validActions={task.validActions}
      projects={projects.projects}
      onUpdated={async () => { await result.refetch(); }}
    />
  ) : (
    <TaskNavigationControls
      navigation={navigation}
      closeButtonRef={closeButtonRef}
      controlsHostRef={controlsHostRef}
    />
  );
  const navigationActions = (
    <TaskNavigationControls navigation={navigation} controlsHostRef={controlsHostRef} />
  );

  return (
    <TaskDetailPanel
      actions={actions}
      detail={detail}
      error={result.error ? "Task details could not be loaded." : null}
      loading={result.loading}
      navigationActions={navigationActions}
      taskId={taskId}
    />
  );
}

function mapWorkTaskDetail(task: WorkDetail): TaskDetail {
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

  const reviewedCriteria = new Map<string, WorkDetail["reviews"][number]["criteria"][number]>();
  for (const review of task.reviews) {
    for (const criterion of review.criteria) {
      if (!reviewedCriteria.has(criterion.criterionId)) reviewedCriteria.set(criterion.criterionId, criterion);
    }
  }
  const submittedCriteria = new Map<string, WorkDetail["submissions"][number]["criteria"][number]>();
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
  const accepted = task.acceptedResult;
  const displayedSubmission = accepted ?? task.latestSubmission;
  const artifacts = task.artifacts.map((artifact) => ({
    id: artifact.artifactId,
    title: artifact.title,
    kind: artifact.artifactKind,
    mediaType: artifact.currentVersion.mediaType,
    href: artifact.currentVersion.downloadUrl ?? artifact.currentVersion.externalUrl
  }));

  return {
    taskId: task.taskId,
    title: task.title,
    status: taskStatus(task),
    complexity: (task.currentContract?.complexity.toLowerCase() ?? "medium") as TaskDetail["complexity"],
    request: task.currentContract?.requestMarkdown ?? task.description,
    criteria,
    createdAt: task.createdAt,
    updatedAt: task.updatedAt,
    sourceLabel: sourceLabel(task),
    currentRevision: task.generation,
    maxReviewRounds: task.currentContract?.executionPolicy.maxReviewRounds ?? null,
    revisions,
    finalResult: displayedSubmission ? {
      summary: displayedSubmission.summary,
      body: displayedSubmission.resultMarkdown,
      approvedAt: accepted?.createdAt ?? null
    } : null,
    artifacts,
    failureReason: currentFailure(task),
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

function mapRun(run: WorkDetail["runs"][number]): TaskRun {
  return {
    id: run.runId,
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
  submission: WorkDetail["submissions"][number]
): TaskSubmission {
  return {
    id: submission.submissionId,
    revision: submission.reviewRound,
    summary: submission.summary,
    result: submission.resultMarkdown,
    evidence: submission.criteria.map((criterion) => criterion.evidenceMarkdown).join("\n\n"),
    artifacts: submission.artifacts.map((artifact) => ({
      id: artifact.artifactId,
      title: artifact.title,
      kind: artifact.artifactKind,
      mediaType: artifact.mediaType,
      href: artifact.downloadUrl ?? artifact.externalUrl
    })),
    createdAt: submission.createdAt
  };
}

function mapReview(review: WorkDetail["reviews"][number]): TaskReview {
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

function taskStatus(task: WorkDetail): TaskStatus {
  switch (task.stage.behavior) {
    case "TERMINAL_SUCCESS": return "completed";
    case "TERMINAL_CANCELLED": return "cancelled";
    case "HUMAN_GATE":
    case "ACCEPTANCE": return "waiting_for_human";
    case "ACTIVE":
      if (task.currentRun?.kind === "REVIEWER") return "reviewing";
      if (task.latestReview?.verdict === "REQUEST_CHANGES") return "revision_requested";
      return "executing";
    case "DISPATCH":
    case "INTAKE": return "queued";
  }
}

function currentFailure(task: WorkDetail): string | null {
  const failed = task.runs.find((run) => run.status === "FAILED");
  return failed ? [failed.errorMessage, failed.errorCode].filter(Boolean).join(" · ") : null;
}

function sourceLabel(task: WorkDetail): string | null {
  if (task.project) return task.project.name;
  if (task.source.conversationId) return "Conversation";
  return null;
}

function criterionVerdict(value: string): "pass" | "fail" | "uncertain" {
  if (value === "PASS") return "pass";
  if (value === "FAIL") return "fail";
  return "uncertain";
}

function terminalBehavior(value: WorkDetail["stage"]["behavior"]): boolean {
  return value === "TERMINAL_SUCCESS" || value === "TERMINAL_CANCELLED";
}
