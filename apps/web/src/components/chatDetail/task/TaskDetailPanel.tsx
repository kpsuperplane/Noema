import * as React from "react";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import { TaskExpandableContent } from "./TaskSection";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskCriteria } from "./TaskCriteria";
import { taskStageLabel } from "./TaskOverview";
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
        <div {...stylex.props(styles.transcriptRegion)}>
          <TaskTranscript detail={currentDetail} liveRunItems={liveRunItems} />
        </div>
        <TaskContextCard
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

function TaskContextCard({
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
  const [expanded, setExpanded] = React.useState(false);
  const stageLabel = taskStageLabel(detail);
  const criteriaSummary = detail.criteria.length > 0 ? `${detail.criteria.length} criteria` : "No criteria";

  return (
    <aside aria-label="Task context" {...stylex.props(styles.contextDock)}>
      <div {...stylex.props(styles.contextCard)}>
        <header {...stylex.props(styles.cardHeader)}>
          <div {...stylex.props(styles.cardIdentity)}>
            <span {...stylex.props(styles.cardState)}>{detail.attention ? "Needs you" : stageLabel}</span>
            <span {...stylex.props(styles.cardMeta)}>{criteriaSummary}</span>
            {detail.currentRevision ? <span {...stylex.props(styles.cardMeta)}>Revision {detail.currentRevision}</span> : null}
          </div>
          <button
            type="button"
            aria-controls={`task-context:${taskId}`}
            aria-expanded={expanded}
            aria-label={expanded ? "Collapse task info" : "Expand task info"}
            onClick={() => setExpanded((current) => !current)}
            {...stylex.props(styles.contextToggle)}
          >
            <ChevronDown aria-hidden="true" size={16} {...stylex.props(styles.contextToggleIcon, expanded && styles.contextToggleIconOpen)} />
          </button>
        </header>
        {governedActions ? <div {...stylex.props(styles.governedActions)}>{governedActions}</div> : null}
        {detail.attention ? <TaskAttention detail={detail} actions={actions} /> : actions ? <div {...stylex.props(styles.actionRow)}>{actions}</div> : null}
        {expanded ? <TaskInfoBody detail={detail} taskId={taskId} /> : null}
      </div>
    </aside>
  );
}

function TaskAttention({ detail, actions }: { detail: TaskDetail; actions?: React.ReactNode }) {
  const prompt = detail.blockingQuestion?.trim() || null;
  const context = detail.attention?.context?.trim() || null;
  const contextIsPrimary = Boolean(context) && (detail.attention?.kind === "RECOVERY_REQUIRED" || !prompt);
  const primaryText = contextIsPrimary ? null : prompt || (context ? null : detail.attention?.summary);
  const result = detail.stageBehavior === "ACCEPTANCE" || detail.stageBehavior === "TERMINAL_SUCCESS"
    ? <TaskResult embedded artifacts={detail.artifacts} result={detail.finalResult} />
    : null;
  const evidence = detail.stageBehavior === "ACCEPTANCE" ? latestReviewSummary(detail) : null;

  return (
    <section aria-labelledby="task-attention-title" {...stylex.props(styles.attention)}>
      <div {...stylex.props(styles.attentionEyebrow)}>Needs your input</div>
      <h3 id="task-attention-title" {...stylex.props(styles.attentionTitle)}>{detail.attention?.title}</h3>
      {primaryText ? <p {...stylex.props(styles.attentionText)}>{primaryText}</p> : null}
      {context ? (
        <Markdown
          autolink="gfm"
          contentWidth="100%"
          density="compact"
          headingLevelStart={4}
          xstyle={markdownXStyle(contextIsPrimary ? styles.attentionPrimaryMarkdown : styles.attentionMarkdown)}
        >
          {context}
        </Markdown>
      ) : null}
      {result}
      {evidence ? <div {...stylex.props(styles.attentionEvidence)}><span {...stylex.props(styles.infoLabel)}>Review evidence</span><Markdown autolink="gfm" contentWidth="100%" density="compact" headingLevelStart={4} xstyle={markdownXStyle(styles.attentionMarkdown)}>{evidence}</Markdown></div> : null}
      {actions ? <div {...stylex.props(styles.attentionActions)}>{actions}</div> : null}
    </section>
  );
}

function TaskInfoBody({ detail, taskId }: { detail: TaskDetail; taskId: string }) {
  const result = detail.stageBehavior === "ACCEPTANCE" || detail.stageBehavior === "TERMINAL_SUCCESS"
    ? <TaskResult embedded artifacts={detail.artifacts} result={detail.finalResult} />
    : null;

  return (
    <div id={`task-context:${taskId}`} {...stylex.props(styles.infoBody)}>
      {result ? <section {...stylex.props(styles.infoSection, styles.resultSection)}><span {...stylex.props(styles.infoLabel)}>Result</span>{result}</section> : null}
      <section {...stylex.props(styles.infoSection)}>
        <span {...stylex.props(styles.infoLabel)}>Request</span>
        <TaskExpandableContent id={`task-request-content:${taskId}`}>
          <Markdown autolink="gfm" contentWidth="100%" density="compact" headingLevelStart={4} xstyle={markdownXStyle(styles.markdown)}>{detail.request}</Markdown>
        </TaskExpandableContent>
      </section>
      {detail.criteria.length > 0 ? <section {...stylex.props(styles.infoSection)}><TaskCriteria embedded criteria={detail.criteria} /></section> : null}
      <TaskInfoMetadata detail={detail} />
    </div>
  );
}

function TaskInfoMetadata({ detail }: { detail: TaskDetail }) {
  const revision = detail.currentRevision ?? latestRevision(detail);
  const provenance = [detail.createdBy, detail.sourceLabel].filter(Boolean).join(" · ");
  return (
    <section {...stylex.props(styles.infoSection)}>
      <span {...stylex.props(styles.infoLabel)}>Task details</span>
      <dl {...stylex.props(styles.metadata)}>
        {detail.complexity ? <MetadataRow label="Complexity" value={capitalize(detail.complexity)} /> : null}
        <MetadataRow label="Stage" value={taskStageLabel(detail)} />
        {revision > 0 ? <MetadataRow label="Revision" value={`${revision}`} /> : null}
        {detail.maxReviewRounds ? <MetadataRow label="Review limit" value={`${detail.maxReviewRounds} rounds`} /> : null}
        {detail.createdAt ? <MetadataRow label="Created" value={formatDate(detail.createdAt)} /> : null}
        {provenance ? <MetadataRow label="From" value={provenance} /> : null}
      </dl>
    </section>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return <div {...stylex.props(styles.metadataRow)}><dt {...stylex.props(styles.metadataKey)}>{label}</dt><dd {...stylex.props(styles.metadataValue)}>{value}</dd></div>;
}

function latestRevision(detail: TaskDetail): number {
  return detail.revisions.reduce((latest, revision) => Math.max(latest, revision.revision), 0);
}

function formatDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp)
    ? value
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
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
  transcriptRegion: {
    minWidth: 0,
    minHeight: 0,
    paddingInline: "var(--spacing-4)",
    paddingBlockEnd: "var(--spacing-2)",
    boxSizing: "border-box"
  },
  contextDock: {
    position: "relative",
    zIndex: 2,
    minWidth: 0,
    marginInline: "var(--spacing-4)",
    marginBlockEnd: "var(--spacing-3)",
    marginBlockStart: "calc(-1 * var(--spacing-2))"
  },
  contextCard: {
    display: "grid",
    minWidth: 0,
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 12,
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "0 10px 28px color-mix(in srgb, var(--noema-text-primary) 13%, transparent)"
  },
  cardHeader: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  cardIdentity: { display: "flex", minWidth: 0, alignItems: "center", flexWrap: "wrap", gap: "var(--spacing-1-5)" },
  cardState: { color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 700, whiteSpace: "nowrap" },
  cardMeta: { color: "var(--noema-text-muted)", fontSize: 11, whiteSpace: "nowrap" },
  contextToggle: { display: "inline-flex", width: 28, height: 28, alignItems: "center", justifyContent: "center", borderWidth: 0, borderRadius: 6, backgroundColor: "transparent", color: "var(--noema-text-muted)", cursor: "pointer", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  contextToggleIcon: { transition: "transform 140ms ease" },
  contextToggleIconOpen: { transform: "rotate(180deg)" },
  governedActions: { minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  actionRow: { minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  attention: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "color-mix(in srgb, var(--noema-clay-600) 24%, transparent)", backgroundColor: "color-mix(in srgb, var(--noema-clay-50) 52%, var(--noema-surface-card))", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  attentionEyebrow: { color: "var(--noema-clay-700)", fontSize: 10, fontWeight: 700, letterSpacing: "0.04em", textTransform: "uppercase" },
  attentionTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 700, lineHeight: 1.35 },
  attentionText: { margin: 0, color: "var(--noema-text-primary)", fontSize: 12, lineHeight: 1.45 },
  attentionMarkdown: { color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.45 },
  attentionPrimaryMarkdown: { color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 600, lineHeight: 1.45 },
  attentionEvidence: { display: "grid", gap: "var(--spacing-1)", minWidth: 0 },
  attentionActions: { minWidth: 0, marginBlockStart: "var(--spacing-1)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "color-mix(in srgb, var(--noema-clay-600) 22%, transparent)", paddingBlockStart: "var(--spacing-2)" },
  infoBody: { minWidth: 0, maxHeight: "min(50vh, 420px)", overflowX: "hidden", overflowY: "auto", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", backgroundColor: "var(--noema-surface-sunken)" },
  infoSection: { display: "grid", gap: "var(--spacing-2)", minWidth: 0, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-3)", paddingInline: "var(--spacing-3)" },
  resultSection: { backgroundColor: "var(--noema-surface-card)" },
  infoLabel: { color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 700, letterSpacing: "0.03em", textTransform: "uppercase" },
  metadata: { display: "grid", gap: "var(--spacing-1-5)", margin: 0 },
  metadataRow: { display: "grid", gridTemplateColumns: "minmax(88px, 0.42fr) minmax(0, 1fr)", gap: "var(--spacing-3)", alignItems: "baseline" },
  metadataKey: { color: "var(--noema-text-muted)", fontSize: 11 },
  metadataValue: { minWidth: 0, margin: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  status: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.5 }
});
