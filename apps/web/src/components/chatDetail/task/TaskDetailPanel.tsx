import * as React from "react";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { AlertCircle } from "lucide-react";
import { TaskExpandableContent, TaskSection, TaskStaticSection } from "./TaskSection";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskCriteria } from "./TaskCriteria";
import { TaskDetails, TaskStatusSummary } from "./TaskOverview";
import { TaskResult } from "./TaskResult";
import { TaskRevisionTimeline } from "./TaskRevisionTimeline";
import { TaskRunConversationView } from "./TaskRunConversationView";

type MarkdownXStyle = MarkdownProps["xstyle"];
type TaskDetailView = { kind: "overview" } | { kind: "run"; runId: string };

const taskDetailTransitionMs = 300;

export function TaskDetailPanel({
  taskId,
  detail,
  loading = false,
  error = null,
  onCancelTask,
  onResumeTask,
  liveRunItems,
  actions,
  navigationActions
}: {
  taskId: string;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  onCancelTask?: (taskId: string) => void | Promise<void>;
  onResumeTask?: (taskId: string, message?: string) => void | Promise<void>;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  actions?: React.ReactNode;
  navigationActions?: React.ReactNode;
}) {
  const [actionBusy, setActionBusy] = React.useState<"cancel" | "resume" | null>(null);
  const [actionError, setActionError] = React.useState<string | null>(null);
  const [selectedRunKey, setSelectedRunKey] = React.useState<{
    taskId: string;
    runId: string;
  } | null>(null);
  const [settledRunKey, setSettledRunKey] = React.useState<{
    taskId: string;
    runId: string;
  } | null>(null);
  const focusTimelineAfterTransitionRef = React.useRef(false);
  const currentDetail = detail?.taskId === taskId ? detail : null;
  const selectedRunId = selectedRunKey?.taskId === taskId ? selectedRunKey.runId : null;
  const settledRunId = settledRunKey?.taskId === taskId ? settledRunKey.runId : null;
  const transitioning = selectedRunId !== settledRunId;
  const transitionDirection = selectedRunId ? "forward" : "backward";
  const handleRunBack = React.useCallback(() => {
    if (transitioning) {
      return;
    }
    focusTimelineAfterTransitionRef.current = true;
    setSelectedRunKey(null);
  }, [transitioning]);

  React.useEffect(() => {
    if (!transitioning) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setSettledRunKey(selectedRunId ? { taskId, runId: selectedRunId } : null);
      if (focusTimelineAfterTransitionRef.current && !selectedRunId) {
        focusTimelineAfterTransitionRef.current = false;
        window.requestAnimationFrame(() => {
          document.getElementById("task-timeline-title")?.focus();
        });
      }
    }, taskDetailTransitionMs);

    return () => window.clearTimeout(timeoutId);
  }, [selectedRunId, taskId, transitioning]);

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
      <div {...stylex.props(styles.root)}>
        {actions}
        <div role="status" {...stylex.props(styles.status)}>
          Loading task details...
        </div>
      </div>
    );
  }
  if (error && !currentDetail) {
    return <div {...stylex.props(styles.root)}>{actions}<TaskUnavailable message={error} /></div>;
  }
  if (!currentDetail) {
    return <div {...stylex.props(styles.root)}>{actions}<TaskUnavailable message="Task details are unavailable." /></div>;
  }

  const selectedView: TaskDetailView = selectedRunId
    ? { kind: "run", runId: selectedRunId }
    : { kind: "overview" };
  const settledView: TaskDetailView = settledRunId
    ? { kind: "run", runId: settledRunId }
    : { kind: "overview" };
  const renderView = (view: TaskDetailView) => {
    if (view.kind === "run") {
      const runContext = currentDetail.revisions
        .flatMap((revision) => [
          ...revision.executors.map((run) => ({ revision, run })),
          ...revision.reviewers.map((run) => ({ revision, run }))
        ])
        .find(({ run }) => run.id === view.runId);
      if (runContext) {
        return (
          <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root, styles.runRoot)}>
            {navigationActions}
            <TaskRunConversationView
              liveItems={liveRunItems?.get(runContext.run.id)}
              onBack={handleRunBack}
              review={runContext.revision.review}
              run={runContext.run}
            />
          </div>
        );
      }
    }

    return (
      <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
        {actions}
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
          onSelectRun={(run) => {
            if (!transitioning) {
              setSelectedRunKey({ taskId, runId: run.id });
            }
          }}
          revisions={currentDetail.revisions}
        />
        <TaskCriteria key={`criteria:${taskId}`} criteria={currentDetail.criteria} />
        <TaskResult key={`result:${taskId}`} artifacts={currentDetail.artifacts} result={currentDetail.finalResult} />
        {currentDetail.messages?.length ? (
          <TaskStaticSection count={currentDetail.messages.length} id="task-messages-title" title="Updates">
            <ol {...stylex.props(styles.messages)}>
              {currentDetail.messages.map((message) => (
                <li key={message.id} {...stylex.props(styles.message)}>
                  <div {...stylex.props(styles.messageMeta)}>
                    <span>{message.author}</span><time dateTime={message.createdAt}>{formatDate(message.createdAt)}</time>
                  </div>
                  <TaskExpandableContent id={`task-message-${message.id}`}>
                    <Markdown autolink="gfm" contentWidth="100%" density="default" headingLevelStart={4} xstyle={markdownXStyle(styles.markdown)}>{message.body}</Markdown>
                  </TaskExpandableContent>
                </li>
              ))}
            </ol>
          </TaskStaticSection>
        ) : null}
        {currentDetail.failureReason ? (
          <FailureNotice message={currentDetail.failureReason} />
        ) : null}
        <TaskDetails key={`details:${taskId}`} detail={currentDetail} />
      </div>
    );
  };

  return (
    <div data-slot="task-detail-view-viewport" {...stylex.props(styles.viewport)}>
      {transitioning ? (
        <div
          key={`exiting:${taskViewKey(settledView)}`}
          data-slot="task-detail-view-frame"
          data-task-detail-frame-state="exiting"
          data-task-detail-transition-direction={transitionDirection}
          aria-hidden="true"
          inert
          {...stylex.props(
            styles.frame,
            settledView.kind === "run" ? styles.runFrame : styles.scrollFrame,
            styles.exitingFrame
          )}
        >
          {renderView(settledView)}
        </div>
      ) : null}
      <div
        key={taskViewKey(selectedView)}
        data-slot="task-detail-view-frame"
        data-task-detail-frame-state={transitioning ? "entering" : "current"}
        data-task-detail-transition-direction={transitionDirection}
        {...stylex.props(
          styles.frame,
          selectedView.kind === "run" ? styles.runFrame : styles.scrollFrame
        )}
      >
        {renderView(selectedView)}
      </div>
    </div>
  );
}

function formatDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? value : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

function taskViewKey(view: TaskDetailView): string {
  return view.kind === "run" ? `run:${view.runId}` : "overview";
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
  viewport: { position: "relative", minWidth: 0, minHeight: 0, height: "100%", overflow: "hidden" },
  frame: {
    position: "relative",
    boxSizing: "border-box",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    width: "100%",
    padding: 8,
    backgroundColor: "var(--noema-surface-card)",
    willChange: "transform, opacity"
  },
  runFrame: { overflow: "hidden" },
  scrollFrame: { overflowX: "hidden", overflowY: "auto" },
  exitingFrame: { position: "absolute", inset: 0, pointerEvents: "none" },
  root: { display: "grid", minWidth: 0 },
  runRoot: { gridTemplateRows: "auto minmax(0, 1fr)", minHeight: 0, height: "100%" },
  status: { padding: 8, color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: 8, color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.55 },
  messages: { display: "grid", gap: 6, margin: 0, padding: 0, listStyle: "none" },
  message: { display: "grid", gap: 4, minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlockStart: 7 },
  messageMeta: { display: "flex", justifyContent: "space-between", gap: 8, color: "var(--noema-text-muted)", fontSize: 10 },
  failure: { display: "flex", alignItems: "start", gap: 8, marginTop: 10, borderRadius: 8, backgroundColor: "color-mix(in srgb, var(--noema-red-100) 55%, transparent)", padding: 10, color: "var(--noema-red-700)", fontSize: 12, lineHeight: 1.4 },
  failureText: { margin: 0 }
});
