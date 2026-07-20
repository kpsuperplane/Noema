import * as React from "react";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import { Popover } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { Info } from "lucide-react";
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
  inlineResponse = false,
  actions,
  governedActions
}: {
  taskId: string;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  inlineResponse?: boolean;
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
          inlineResponse={inlineResponse}
        />
      </div>
    </div>
  );
}

function TaskContextCard({
  detail,
  actions,
  governedActions,
  inlineResponse
}: {
  detail: TaskDetail;
  actions?: React.ReactNode;
  governedActions?: React.ReactNode;
  inlineResponse: boolean;
}) {
  return (
    <aside aria-label="Task summary" {...stylex.props(styles.contextDock)}>
      <div {...stylex.props(styles.contextCard)}>
        <TaskSummaryHeader detail={detail} />
        {detail.attention ? (
          <TaskAttention detail={detail} actions={actions} governedActions={governedActions} inlineResponse={inlineResponse} />
        ) : governedActions ? (
          <div {...stylex.props(styles.actionRow)}>{governedActions}</div>
        ) : actions ? (
          <div {...stylex.props(styles.actionRow)}>{actions}</div>
        ) : null}
        {detail.criteria.length > 0 ? <TaskValidationRow criteria={detail.criteria} /> : null}
      </div>
    </aside>
  );
}

function TaskSummaryHeader({ detail }: { detail: TaskDetail }) {
  return (
    <header {...stylex.props(styles.summaryHeader)}>
      <h2 {...stylex.props(styles.summaryTitle)}>{detail.title}</h2>
      <TaskInfoTrigger detail={detail} />
    </header>
  );
}

function TaskInfoTrigger({ detail }: { detail: TaskDetail }) {
  const [open, setOpen] = React.useState(false);
  const pinnedRef = React.useRef(false);
  const hoveringRef = React.useRef(false);
  const closeTimeoutRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const clearCloseTimeout = React.useCallback(() => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
      closeTimeoutRef.current = null;
    }
  }, []);
  const scheduleClose = React.useCallback(() => {
    clearCloseTimeout();
    if (pinnedRef.current || hoveringRef.current) return;
    closeTimeoutRef.current = setTimeout(() => {
      closeTimeoutRef.current = null;
      setOpen(false);
    }, 180);
  }, [clearCloseTimeout]);
  React.useEffect(() => () => clearCloseTimeout(), [clearCloseTimeout]);

  return (
    <Popover
      alignment="end"
      content={(
        <div onMouseEnter={() => { hoveringRef.current = true; clearCloseTimeout(); }} onMouseLeave={() => { hoveringRef.current = false; scheduleClose(); }}>
          <TaskInfoPopoverContent detail={detail} />
        </div>
      )}
      hasAutoFocus={false}
      isOpen={open}
      label="Task information"
      onOpenChange={(next) => {
        if (!next) {
          pinnedRef.current = false;
          setOpen(false);
        }
      }}
      placement="above"
      width="min(320px, calc(100vw - var(--spacing-6)))"
      xstyle={popoverXStyle(styles.infoPopover)}
    >
      {(trigger) => (
        <button
          ref={(element) => trigger.ref(element)}
          type="button"
          aria-controls={trigger["aria-controls"]}
          aria-expanded={trigger["aria-expanded"]}
          aria-haspopup={trigger["aria-haspopup"]}
          aria-label="Show task information"
          onClick={() => {
            clearCloseTimeout();
            pinnedRef.current = !pinnedRef.current;
            setOpen(pinnedRef.current);
          }}
          onFocus={() => { clearCloseTimeout(); setOpen(true); }}
          onMouseEnter={() => { hoveringRef.current = true; clearCloseTimeout(); setOpen(true); }}
          onMouseLeave={() => { hoveringRef.current = false; scheduleClose(); }}
          {...stylex.props(styles.infoButton)}
        >
          <Info aria-hidden="true" size={15} strokeWidth={2} />
        </button>
      )}
    </Popover>
  );
}

function TaskAttention({
  detail,
  actions,
  governedActions,
  inlineResponse
}: {
  detail: TaskDetail;
  actions?: React.ReactNode;
  governedActions?: React.ReactNode;
  inlineResponse: boolean;
}) {
  const prompt = detail.blockingQuestion?.trim() || null;
  const context = detail.attention?.context?.trim() || null;
  const contextIsPrimary = Boolean(context) && (detail.attention?.kind === "RECOVERY_REQUIRED" || !prompt);
  const primaryText = contextIsPrimary ? null : prompt || (context ? null : detail.attention?.summary);
  const result = detail.stageBehavior === "ACCEPTANCE" || detail.stageBehavior === "TERMINAL_SUCCESS"
    ? <TaskResult embedded artifacts={detail.artifacts} result={detail.finalResult} />
    : null;
  const evidence = detail.stageBehavior === "ACCEPTANCE" ? latestReviewSummary(detail) : null;

  const attentionCopy = (
    <>
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
    </>
  );

  return (
    <section aria-labelledby="task-attention-title" {...stylex.props(styles.attention)}>
      {inlineResponse && actions ? (
        <div {...stylex.props(styles.attentionComposer)}>
          <div {...stylex.props(styles.attentionComposerCopy)}>{attentionCopy}</div>
          <div {...stylex.props(styles.attentionActions)}>{actions}</div>
        </div>
      ) : (
        attentionCopy
      )}
      {result}
      {evidence ? <div {...stylex.props(styles.attentionEvidence)}><Markdown autolink="gfm" contentWidth="100%" density="compact" headingLevelStart={4} xstyle={markdownXStyle(styles.attentionMarkdown)}>{evidence}</Markdown></div> : null}
      {governedActions ? <div {...stylex.props(styles.attentionGovernedActions)}>{governedActions}</div> : null}
      {!inlineResponse && actions ? <div {...stylex.props(styles.attentionActions)}>{actions}</div> : null}
    </section>
  );
}

function TaskValidationRow({ criteria }: { criteria: TaskDetail["criteria"] }) {
  const passed = criteria.filter((criterion) => criterion.verdict === "pass").length;
  const failed = criteria.filter((criterion) => criterion.verdict === "fail").length;
  const summary = failed > 0
    ? `${failed} failed · ${criteria.length} criteria`
    : passed === criteria.length
      ? `${passed} passed · ${criteria.length} criteria`
      : `${passed} passed · ${criteria.length - passed} pending`;

  return (
    <section {...stylex.props(styles.validationRow)}>
      <div {...stylex.props(styles.validationHeader)}>
        <span {...stylex.props(styles.validationIdentity)}>
          <span {...stylex.props(styles.validationLabel)}>Validation</span>
          <span {...stylex.props(styles.validationSummary)}>{summary}</span>
        </span>
      </div>
      <TaskCriteria embedded criteria={criteria} showTitle={false} />
    </section>
  );
}

function TaskInfoPopoverContent({ detail }: { detail: TaskDetail }) {
  const result = detail.stageBehavior === "ACCEPTANCE" || detail.stageBehavior === "TERMINAL_SUCCESS"
    ? <TaskResult embedded artifacts={detail.artifacts} result={detail.finalResult} />
    : null;

  return (
    <div {...stylex.props(styles.infoContent)}>
      {result ? <section {...stylex.props(styles.infoSection, styles.resultSection)}>{result}</section> : null}
      <TaskInfoMetadata detail={detail} />
    </div>
  );
}

function TaskInfoMetadata({ detail }: { detail: TaskDetail }) {
  const revision = detail.currentRevision ?? latestRevision(detail);
  const provenance = [detail.createdBy, detail.sourceLabel].filter(Boolean).join(" · ");
  return (
    <dl {...stylex.props(styles.metadataSection, styles.metadata)}>
      {detail.complexity ? <MetadataRow label="Complexity" value={capitalize(detail.complexity)} /> : null}
      <MetadataRow label="Stage" value={taskStageLabel(detail)} />
      {revision > 0 ? <MetadataRow label="Revision" value={`${revision}`} /> : null}
      {detail.maxReviewRounds ? <MetadataRow label="Review limit" value={`${detail.maxReviewRounds} rounds`} /> : null}
      {detail.createdAt ? <MetadataRow label="Created" value={formatDate(detail.createdAt)} /> : null}
      {provenance ? <MetadataRow label="From" value={provenance} /> : null}
    </dl>
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

function popoverXStyle(...xstyle: unknown[]): React.ComponentProps<typeof Popover>["xstyle"] {
  return xstyle as React.ComponentProps<typeof Popover>["xstyle"];
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
    paddingBlockStart: "var(--spacing-8)",
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
  summaryHeader: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  summaryTitle: { minWidth: 0, margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 700, lineHeight: 1.35, overflowWrap: "anywhere" },
  infoButton: { display: "inline-flex", width: 28, height: 28, alignItems: "center", justifyContent: "center", borderWidth: 0, borderRadius: 999, backgroundColor: "transparent", color: "var(--noema-text-muted)", cursor: "pointer", ":hover": { backgroundColor: "var(--noema-surface-hover)", color: "var(--noema-text-primary)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  infoPopover: { maxHeight: "min(70vh, 520px)", overflowX: "hidden", overflowY: "auto" },
  actionRow: { minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  attention: { display: "grid", gap: "var(--spacing-2)", minWidth: 0, backgroundColor: "var(--noema-surface-sunken)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  attentionComposer: { display: "grid", gap: "var(--spacing-2)", minWidth: 0, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, backgroundColor: "var(--noema-surface-card)", padding: "var(--spacing-2)" },
  attentionComposerCopy: { display: "grid", gap: "var(--spacing-1)", minWidth: 0 },
  attentionTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 700, lineHeight: 1.35 },
  attentionText: { margin: 0, color: "var(--noema-text-primary)", fontSize: 12, lineHeight: 1.45 },
  attentionMarkdown: { color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.45 },
  attentionPrimaryMarkdown: { color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 600, lineHeight: 1.45 },
  attentionEvidence: { display: "grid", gap: "var(--spacing-1)", minWidth: 0 },
  attentionGovernedActions: { minWidth: 0 },
  attentionActions: { minWidth: 0 },
  validationRow: { minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  validationHeader: { display: "flex", width: "100%", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)" },
  validationIdentity: { display: "flex", minWidth: 0, alignItems: "baseline", gap: "var(--spacing-2)" },
  validationLabel: { color: "var(--noema-text-primary)", fontSize: 11, fontWeight: 700 },
  validationSummary: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 11, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  infoContent: { display: "grid", minWidth: 0, backgroundColor: "var(--noema-surface-card)" },
  infoSection: { display: "grid", gap: "var(--spacing-2)", minWidth: 0, paddingBlock: "var(--spacing-3)", paddingInline: "var(--spacing-3)" },
  resultSection: { backgroundColor: "var(--noema-surface-card)" },
  metadataSection: { paddingBlock: "var(--spacing-3)", paddingInline: "var(--spacing-3)" },
  metadata: { display: "grid", gap: "var(--spacing-1-5)", margin: 0 },
  metadataRow: { display: "grid", gridTemplateColumns: "minmax(88px, 0.42fr) minmax(0, 1fr)", gap: "var(--spacing-3)", alignItems: "baseline" },
  metadataKey: { color: "var(--noema-text-muted)", fontSize: 11 },
  metadataValue: { minWidth: 0, margin: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  status: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
});
