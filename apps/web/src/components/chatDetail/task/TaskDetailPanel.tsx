import * as React from "react";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { TaskDecisionCard } from "./TaskDecisionCard";
import { TaskExpandableContent, TaskStaticSection } from "./TaskSection";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskCriteria } from "./TaskCriteria";
import { TaskDetails, taskStageLabel } from "./TaskOverview";
import { TaskResult } from "./TaskResult";
import { TaskTranscript } from "./TaskTranscript";

type MarkdownXStyle = MarkdownProps["xstyle"];

export function TaskDetailPanel({
  taskId,
  detail,
  loading = false,
  error = null,
  liveRunItems,
  actions,
  governedActions
}: {
  taskId: string;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  actions?: React.ReactNode;
  governedActions?: React.ReactNode;
}) {
  const currentDetail = detail?.taskId === taskId ? detail : null;

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

  return (
    <div data-slot="task-detail-view-viewport" {...stylex.props(styles.viewport)}>
      <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
        <TaskTranscript detail={currentDetail} liveRunItems={liveRunItems} />
        <TaskContextDock
          key={`context:${taskId}:${currentDetail.attention ? "attention" : "info"}`}
          actions={actions}
          detail={currentDetail}
          governedActions={governedActions}
          taskId={taskId}
        />
      </div>
    </div>
  );
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

function TaskContextDock({
  detail,
  actions,
  governedActions,
  taskId
}: {
  detail: TaskDetail;
  actions?: React.ReactNode;
  governedActions?: React.ReactNode;
  taskId: string;
}) {
  const [expanded, setExpanded] = React.useState(Boolean(detail.attention));
  const result = detail.stageBehavior === "ACCEPTANCE" || detail.stageBehavior === "TERMINAL_SUCCESS"
    ? <TaskResult embedded artifacts={detail.artifacts} result={detail.finalResult} />
    : null;
  const evidence = detail.stageBehavior === "ACCEPTANCE" ? latestReviewSummary(detail) : null;

  return (
    <aside aria-label="Task context" {...stylex.props(styles.dock)}>
      {governedActions}
      {detail.attention ? (
        <TaskDecisionCard attention={detail.attention} question={detail.blockingQuestion}>
          {result}
          {evidence ? <ReviewEvidence summary={evidence} /> : null}
          {actions}
        </TaskDecisionCard>
      ) : actions ? (
        <div {...stylex.props(styles.dockActions)}>{actions}</div>
      ) : null}
      <button
        type="button"
        aria-controls={`task-context:${taskId}`}
        aria-expanded={expanded}
        onClick={() => setExpanded((current) => !current)}
        {...stylex.props(styles.contextToggle)}
      >
        <span {...stylex.props(styles.contextToggleTitle)}>{detail.attention ? "Task context" : "Task info"}</span>
        <span {...stylex.props(styles.contextToggleSummary)}>
          {taskStageLabel(detail)}
          {detail.currentRevision ? ` · Revision ${detail.currentRevision}` : ""}
        </span>
        <span aria-hidden="true" {...stylex.props(styles.contextToggleIcon, expanded && styles.contextToggleIconOpen)}>⌄</span>
      </button>
      {expanded ? (
        <div id={`task-context:${taskId}`} {...stylex.props(styles.contextBody)}>
          <TaskBrief key={`brief:${taskId}`} criteria={detail.criteria} request={detail.request} />
          <TaskDetails key={`details:${taskId}`} detail={detail} />
        </div>
      ) : null}
    </aside>
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
  viewport: { minWidth: 0, minHeight: 0, height: "100%", overflow: "hidden" },
  root: {
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr) auto",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    backgroundColor: "var(--noema-surface-card)"
  },
  dock: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-2)",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    backgroundColor: "var(--noema-surface-card)",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)",
    boxShadow: "0 -8px 22px color-mix(in srgb, var(--noema-text-primary) 7%, transparent)"
  },
  dockActions: { minWidth: 0 },
  contextToggle: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-2)",
    borderWidth: 0,
    borderRadius: 7,
    backgroundColor: "var(--noema-surface-sunken)",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)",
    color: "var(--noema-text-primary)",
    font: "inherit",
    textAlign: "left",
    cursor: "pointer",
    ":hover": { backgroundColor: "var(--noema-surface-hover)" },
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 }
  },
  contextToggleTitle: { color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 700, whiteSpace: "nowrap" },
  contextToggleSummary: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 11, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  contextToggleIcon: { color: "var(--noema-text-muted)", fontSize: 16, lineHeight: 1, transform: "translateY(-1px)", transition: "transform 140ms ease" },
  contextToggleIconOpen: { transform: "rotate(180deg) translateY(1px)" },
  contextBody: { minWidth: 0, maxHeight: "min(48vh, 420px)", overflowX: "hidden", overflowY: "auto", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 8, backgroundColor: "var(--noema-surface-sunken)" },
  status: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
  briefLabel: { margin: 0, color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650 },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.55 },
  reviewEvidence: { display: "grid", gap: "var(--spacing-1)", minWidth: 0 }
});
