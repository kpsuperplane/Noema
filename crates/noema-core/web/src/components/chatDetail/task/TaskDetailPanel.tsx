import * as React from "react";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { AlertCircle } from "lucide-react";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskCriteria } from "./TaskCriteria";
import { TaskModelSnapshots } from "./TaskModelSnapshots";
import { TaskOverview } from "./TaskOverview";
import { TaskResult } from "./TaskResult";
import { runTimelineLabel, TaskRevisionTimeline } from "./TaskRevisionTimeline";
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
          label={runTimelineLabel(selectedRun, selectedRunContext?.revision.review)}
          onBack={handleRunBack}
          run={selectedRun}
        />
      </div>
    );
  }

  return (
    <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
      <TaskOverview
        actionBusy={actionBusy}
        actionError={actionError}
        detail={currentDetail}
        onCancel={onCancelTask ? () => runAction("cancel") : undefined}
        onResume={onResumeTask ? (message) => runAction("resume", message) : undefined}
      />
      <Divider />
      <TaskTextSection title="Original request" text={currentDetail.request} />
      <Divider />
      <TaskCriteria criteria={currentDetail.criteria} />
      <TaskModelSnapshots
        executor={currentDetail.executorModel}
        reviewer={currentDetail.reviewerModel}
        reviewerInherited={currentDetail.reviewerModelInherited}
      />
      <Divider />
      <TaskRevisionTimeline
        onSelectRun={(run) => setSelectedRunKey({ taskId, runId: run.id })}
        revisions={currentDetail.revisions}
      />
      {currentDetail.finalResult || currentDetail.artifacts?.length ? <Divider /> : null}
      <TaskResult artifacts={currentDetail.artifacts} result={currentDetail.finalResult} />
      {currentDetail.failureReason ? (
        <>
          <Divider />
          <FailureNotice message={currentDetail.failureReason} />
        </>
      ) : null}
    </div>
  );
}

function TaskTextSection({ title, text }: { title: string; text: string }) {
  return (
    <section aria-labelledby="task-request-title" {...stylex.props(styles.textSection)}>
      <h3 id="task-request-title" {...stylex.props(styles.sectionTitle)}>{title}</h3>
      <Markdown
        autolink="gfm"
        contentWidth="100%"
        density="default"
        headingLevelStart={3}
        xstyle={markdownXStyle(styles.markdown)}
      >
        {text}
      </Markdown>
    </section>
  );
}

function FailureNotice({ message }: { message: string }) {
  return (
    <div role="alert" {...stylex.props(styles.failure)}>
      <AlertCircle aria-hidden="true" size={15} />
      <p>{message}</p>
    </div>
  );
}

function TaskUnavailable({ message }: { message: string }) {
  return <div role="status" {...stylex.props(styles.unavailable)}>{message}</div>;
}

function Divider() {
  return <div aria-hidden="true" {...stylex.props(styles.divider)} />;
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

const styles = stylex.create({
  root: { display: "grid", gap: 13, minWidth: 0 },
  status: { color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
  textSection: { display: "grid", gap: 8, minWidth: 0 },
  sectionTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 700, lineHeight: 1.35 },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.55 },
  divider: { height: 1, backgroundColor: "var(--noema-border-subtle)" },
  failure: { display: "flex", alignItems: "start", gap: 8, borderRadius: 8, backgroundColor: "color-mix(in srgb, var(--noema-red-100) 55%, transparent)", padding: 10, color: "var(--noema-red-700)", fontSize: 12, lineHeight: 1.4 },
  failureText: { margin: 0 }
});
