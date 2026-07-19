import * as React from "react";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { TaskDecisionCard } from "./TaskDecisionCard";
import { TaskExpandableContent, TaskStaticSection } from "./TaskSection";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskCriteria } from "./TaskCriteria";
import { TaskDetails, taskStateHeading } from "./TaskOverview";
import { TaskResult } from "./TaskResult";
import { TaskCurrentRun, TaskRevisionTimeline, taskTimelineEntryCount } from "./TaskRevisionTimeline";
import { TaskRunConversationView } from "./TaskRunConversationView";

type MarkdownXStyle = MarkdownProps["xstyle"];
type TaskDetailView = { kind: "overview" } | { kind: "run"; runId: string };

const taskDetailTransitionMs = 300;

export function TaskDetailPanel({
  taskId,
  detail,
  loading = false,
  error = null,
  liveRunItems,
  actions
}: {
  taskId: string;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  actions?: React.ReactNode;
}) {
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
          (document.getElementById("task-timeline-title") ?? document.getElementById("task-current-state-title"))?.focus();
        });
      }
    }, taskDetailTransitionMs);

    return () => window.clearTimeout(timeoutId);
  }, [selectedRunId, taskId, transitioning]);

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

    const selectRun = (run: Parameters<NonNullable<React.ComponentProps<typeof TaskRevisionTimeline>["onSelectRun"]>>[0]) => {
      if (!transitioning) {
        setSelectedRunKey({ taskId, runId: run.id });
      }
    };
    const currentRunId = currentDetail.stageBehavior === "ACTIVE"
      ? currentDetail.revisions.find((revision) => revision.latestRunId)?.latestRunId
      : null;
    const activityCount = taskTimelineEntryCount(currentDetail.revisions, currentRunId) + (currentDetail.messages?.length ?? 0);

    return (
      <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
        <TaskCurrentState
          actions={actions}
          detail={currentDetail}
          onSelectRun={selectRun}
        />
        <TaskBrief
          key={`brief:${taskId}`}
          criteria={currentDetail.criteria}
          request={currentDetail.request}
        />
        {activityCount > 0 ? (
          <TaskStaticSection count={activityCount} id="task-timeline-title" tabIndex={-1} title="Activity">
            <TaskRevisionTimeline embedded excludeRunId={currentRunId} onSelectRun={selectRun} revisions={currentDetail.revisions} />
            {currentDetail.messages?.length ? <TaskMessages messages={currentDetail.messages} /> : null}
          </TaskStaticSection>
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

function TaskCurrentState({
  detail,
  actions,
  onSelectRun
}: {
  detail: TaskDetail;
  actions?: React.ReactNode;
  onSelectRun: React.ComponentProps<typeof TaskRevisionTimeline>["onSelectRun"];
}) {
  const result = detail.stageBehavior === "ACCEPTANCE" || detail.stageBehavior === "TERMINAL_SUCCESS"
    ? <TaskResult embedded artifacts={detail.artifacts} result={detail.finalResult} />
    : null;
  const evidence = detail.stageBehavior === "ACCEPTANCE" ? latestReviewSummary(detail) : null;
  const description = taskStateDescription(detail);

  if (detail.attention) {
    return (
      <TaskDecisionCard attention={detail.attention} question={detail.blockingQuestion}>
        {result}
        {evidence ? <ReviewEvidence summary={evidence} /> : null}
        {actions}
      </TaskDecisionCard>
    );
  }

  return (
    <TaskStaticSection id="task-current-state-title" tabIndex={-1} title={taskStateHeading(detail)}>
      {description ? <p {...stylex.props(styles.stateDescription)}>{description}</p> : null}
      {detail.stageBehavior === "ACTIVE" ? (
        <TaskCurrentRun onSelectRun={onSelectRun} revisions={detail.revisions} />
      ) : null}
      {result}
      {evidence ? <ReviewEvidence summary={evidence} /> : null}
      {actions}
    </TaskStaticSection>
  );
}

function taskStateDescription(detail: TaskDetail): string | null {
  switch (detail.stageBehavior) {
    case "INTAKE": return "This task has not started.";
    case "DISPATCH": return "Waiting for an available executor.";
    case "TERMINAL_CANCELLED": return "Work was cancelled.";
    case "TERMINAL_SUCCESS": return detail.finalResult ? null : "The accepted work is complete.";
    default: return null;
  }
}

function TaskBrief({
  request,
  criteria
}: {
  request: string;
  criteria: TaskDetail["criteria"];
}) {
  return (
    <TaskStaticSection id="task-brief-title" title="Task brief">
      <div {...stylex.props(styles.briefLabel)}>Original request</div>
      <TaskExpandableContent id="task-request-content">
        <Markdown
          autolink="gfm"
          contentWidth="100%"
          density="default"
          headingLevelStart={4}
          xstyle={markdownXStyle(styles.markdown)}
        >
          {request}
        </Markdown>
      </TaskExpandableContent>
      <TaskCriteria embedded criteria={criteria} />
    </TaskStaticSection>
  );
}

function TaskMessages({ messages }: { messages: NonNullable<TaskDetail["messages"]> }) {
  return (
    <section aria-labelledby="task-updates-title" {...stylex.props(styles.activityGroup)}>
      <h4 id="task-updates-title" {...stylex.props(styles.activityTitle)}>Updates</h4>
      <ol {...stylex.props(styles.messages)}>
        {messages.map((message) => (
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
    </section>
  );
}

function ReviewEvidence({ summary }: { summary: string }) {
  return (
    <div {...stylex.props(styles.reviewEvidence)}>
      <h4 {...stylex.props(styles.briefLabel)}>Review evidence</h4>
      <Markdown autolink="gfm" contentWidth="100%" density="default" headingLevelStart={4} xstyle={markdownXStyle(styles.markdown)}>{summary}</Markdown>
    </div>
  );
}

function latestReviewSummary(detail: TaskDetail): string | null {
  return [...detail.revisions]
    .sort((left, right) => right.revision - left.revision)
    .find((revision) => revision.review?.summary?.trim())
    ?.review?.summary?.trim() ?? null;
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
    paddingBlock: "var(--spacing-1)",
    paddingInline: 0,
    backgroundColor: "var(--noema-surface-card)",
    willChange: "transform, opacity"
  },
  runFrame: { overflow: "hidden" },
  scrollFrame: { overflowX: "hidden", overflowY: "auto" },
  exitingFrame: { position: "absolute", inset: 0, pointerEvents: "none" },
  root: { display: "grid", minWidth: 0, gap: "var(--spacing-1)" },
  runRoot: { minHeight: 0, height: "100%" },
  status: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
  briefLabel: { margin: 0, color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650 },
  stateDescription: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.55 },
  activityGroup: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0 },
  activityTitle: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 11, fontWeight: 650 },
  messages: { display: "grid", gap: "var(--spacing-1-5)", margin: 0, padding: 0, listStyle: "none" },
  message: { display: "grid", gap: "var(--spacing-1)", minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlockStart: "var(--spacing-1-5)" },
  messageMeta: { display: "flex", justifyContent: "space-between", gap: "var(--spacing-2)", color: "var(--noema-text-muted)", fontSize: 10 },
  reviewEvidence: { display: "grid", gap: "var(--spacing-1)", minWidth: 0 }
});
