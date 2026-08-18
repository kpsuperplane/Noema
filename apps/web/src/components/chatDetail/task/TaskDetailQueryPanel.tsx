import * as React from "react";
import { useQuery, useSubscription } from "@apollo/client/react";
import {
  TasksTaskDetailDocument,
  TasksTaskEventsDocument,
  TasksTaskRuntimeEventsDocument,
  type TasksTaskDetailQuery
} from "@/generated/graphql";
import { TaskActions } from "@/components/tasks/TaskActions";
import {
  PendingHumanInterventionsResult,
  pendingHumanInterventionsAreFresh,
  usePendingHumanInterventions
} from "@/components/actions/PendingGovernedActions";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { useTaskProjects } from "@/components/tasks/useTaskProjects";
import { useTaskEventCursor } from "./taskEventCursor";
import { TaskDetailPanel } from "./TaskDetailPanel";
import { taskStatusFromProjection } from "./TaskStatusBadge";
import type {
  TaskDetail,
  TaskRevision,
  TaskRun,
  TaskRunRole,
  TaskRunStatus
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
  const interventionResult = usePendingHumanInterventions({ taskId });
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
  useSubscription(TasksTaskRuntimeEventsDocument, {
    variables: { taskId },
    onData: () => { void result.refetch(); }
  });

  const detail = React.useMemo(() => task ? mapTaskDetail(task) : null, [task]);
  const interventionFresh = pendingHumanInterventionsAreFresh(interventionResult);
  const taskControls = result.error
    ? []
    : task?.validActions.filter((action) => (
        !interventionFresh || (action !== "ANSWER" && action !== "RETRY")
      )) ?? [];
  const renderPanel = (_actions?: React.ReactNode, controls?: React.ReactNode) => (
    <>
      <TaskDetailPanel
        detail={detail}
        error={result.error ? "Task details could not be loaded." : null}
        loading={result.loading}
        onOpenDetail={onOpenDetail}
        renderSecondarySurface={(status) => (
          <PendingHumanInterventionsResult
            emptyContent={status}
            placement="dock"
            result={interventionResult}
          />
        )}
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
  const revisionNumbers = new Set<number>([0, ...runs.map((run) => run.revision ?? 0)]);
  const revisions: TaskRevision[] = [...revisionNumbers]
    .sort((left, right) => left - right)
    .map((revision) => {
      const revisionRuns = runs.filter((run) => (run.revision ?? 0) === revision);
      return {
        revision,
        executors: revisionRuns.filter((run) => run.role !== "reviewer"),
        reviewers: revisionRuns.filter((run) => run.role === "reviewer"),
        latestRunId: task.currentRun?.runId ?? null
      };
    });
  return {
    taskId: task.taskId,
    title: task.title,
    schedule: task.schedule,
    status: taskStatusFromProjection(task),
    stageBehavior: task.stage.behavior,
    capturedRequest: task.description.trim() || task.title,
    taskDocument: task.taskDocument,
    resultDocument: task.resultDocument,
    reviewDocument: task.reviewDocument,
    createdAt: task.createdAt,
    updatedAt: task.updatedAt,
    sourceLabel: sourceLabel(task),
    currentRevision: task.generation,
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

function sourceLabel(task: TasksDetail): string | null {
  if (task.project) return task.project.name;
  if (task.source.conversationId) return "Conversation";
  return null;
}

function terminalBehavior(value: TasksDetail["stage"]["behavior"]): boolean {
  return value === "TERMINAL_SUCCESS" || value === "TERMINAL_CANCELLED";
}
