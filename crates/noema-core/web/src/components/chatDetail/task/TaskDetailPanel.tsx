import * as React from "react";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { AlertCircle } from "lucide-react";
import { TaskExpandableContent, TaskSection } from "./TaskSection";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskCriteria } from "./TaskCriteria";
import { TaskDetails, TaskStatusSummary } from "./TaskOverview";
import { TaskResult } from "./TaskResult";
import { TaskRevisionTimeline } from "./TaskRevisionTimeline";
import { TaskRunConversationView } from "./TaskRunConversationView";

type MarkdownXStyle = MarkdownProps["xstyle"];

export function TaskDetailPanel({
  taskId,
  detail,
  loading = false,
  error = null,
  onCancelTask,
  onResumeTask,
  liveRunItems
}: {
  taskId: string;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  onCancelTask?: (taskId: string) => void | Promise<void>;
  onResumeTask?: (taskId: string, message?: string) => void | Promise<void>;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
}) {
  const [actionBusy, setActionBusy] = React.useState<"cancel" | "resume" | null>(null);
  const [actionError, setActionError] = React.useState<string | null>(null);
  const [selectedRunKey, setSelectedRunKey] = React.useState<{
    taskId: string;
    runId: string;
  } | null>(null);
  const currentDetail = detail?.taskId === taskId ? detail : null;
  const selectedRunId = selectedRunKey?.taskId === taskId ? selectedRunKey.runId : null;
  const selectedRunContext = currentDetail
    ? currentDetail.revisions
        .flatMap((revision) => [
          ...revision.executors.map((run) => ({ revision, run })),
          ...revision.reviewers.map((run) => ({ revision, run }))
        ])
        .find(({ run }) => run.id === selectedRunId) ?? null
    : null;
  const selectedRun = selectedRunContext?.run ?? null;
  const handleRunBack = React.useCallback(() => {
    setSelectedRunKey(null);
    window.requestAnimationFrame(() => {
      document.getElementById("task-timeline-title")?.focus();
    });
  }, []);

  const runAction = React.useCallback(
    async (kind: "cancel" | "resume", message?: string) => {
      const action = kind === "cancel"
        ? onCancelTask
        : onResumeTask
          ? (currentTaskId: string) => onResumeTask(currentTaskId, message)
          : undefined;
      if (!action || actionBusy) {
        return;
      }
      setActionBusy(kind);
      setActionError(null);
      try {
        await action(taskId);
      } catch (caught) {
        setActionError(caught instanceof Error ? caught.message : "Noema could not update this task.");
      } finally {
        setActionBusy(null);
      }
    },
    [actionBusy, onCancelTask, onResumeTask, taskId]
  );

  if (loading && !currentDetail) {
    return (
      <div role="status" {...stylex.props(styles.status)}>
        Loading task details...
      </div>
    );
  }
  if (error && !currentDetail) {
    return <TaskUnavailable message={error} />;
  }
  if (!currentDetail) {
    return <TaskUnavailable message="Task details are unavailable." />;
  }

  if (selectedRun) {
    return (
      <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
        <TaskRunConversationView
          liveItems={liveRunItems?.get(selectedRun.id)}
          onBack={handleRunBack}
          review={selectedRunContext?.revision.review}
          run={selectedRun}
        />
      </div>
    );
  }

  return (
    <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
      <TaskStatusSummary
        key={`status:${taskId}`}
        actionBusy={actionBusy}
        actionError={actionError}
        detail={currentDetail}
        onCancel={onCancelTask ? () => runAction("cancel") : undefined}
        onResume={onResumeTask ? (message) => runAction("resume", message) : undefined}
      />
      <TaskTextSection key={`request:${taskId}`} text={currentDetail.request} />
      <TaskRevisionTimeline
        onSelectRun={(run) => setSelectedRunKey({ taskId, runId: run.id })}
        revisions={currentDetail.revisions}
      />
      <TaskCriteria key={`criteria:${taskId}`} criteria={currentDetail.criteria} />
      <TaskResult key={`result:${taskId}`} artifacts={currentDetail.artifacts} result={currentDetail.finalResult} />
      {currentDetail.failureReason ? (
        <FailureNotice message={currentDetail.failureReason} />
      ) : null}
      <TaskDetails key={`details:${taskId}`} detail={currentDetail} />
    </div>
  );
}

function TaskTextSection({ text }: { text: string }) {
  return (
    <TaskSection label="Original request">
      <TaskExpandableContent id="task-request-content">
        <Markdown
          autolink="gfm"
          contentWidth="100%"
          density="default"
          headingLevelStart={3}
          xstyle={markdownXStyle(styles.markdown)}
        >
          {text}
        </Markdown>
      </TaskExpandableContent>
    </TaskSection>
  );
}

function FailureNotice({ message }: { message: string }) {
  return (
    <div role="alert" {...stylex.props(styles.failure)}>
      <AlertCircle aria-hidden="true" size={15} />
      <p {...stylex.props(styles.failureText)}>{message}</p>
    </div>
  );
}

function TaskUnavailable({ message }: { message: string }) {
  return <div role="status" {...stylex.props(styles.unavailable)}>{message}</div>;
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

const styles = stylex.create({
  root: { display: "grid", minWidth: 0 },
  status: { color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.55 },
  failure: { display: "flex", alignItems: "start", gap: 8, marginTop: 10, borderRadius: 8, backgroundColor: "color-mix(in srgb, var(--noema-red-100) 55%, transparent)", padding: 10, color: "var(--noema-red-700)", fontSize: 12, lineHeight: 1.4 },
  failureText: { margin: 0 }
});
