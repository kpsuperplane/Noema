import * as React from "react";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import { Popover } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { ExternalLink, Info } from "lucide-react";
import { IdentityAvatar } from "@/components/IdentityAvatar";
import type { TaskDetail, TaskRun, TaskRunItem } from "./taskTypes";
import { TaskCriteria } from "./TaskCriteria";
import { taskStageLabel } from "./TaskOverview";
import { TaskTranscript } from "./TaskTranscript";

type MarkdownXStyle = MarkdownProps["xstyle"];

export function TaskDetailPanel({
  taskId,
  detail,
  loading = false,
  error = null,
  liveRunItems,
  controlsHostRef,
  inlineResponse = false,
  actions,
  governedActions,
  showWorkLink = false
}: {
  taskId: string;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  controlsHostRef?: React.RefObject<HTMLDivElement | null>;
  showWorkLink?: boolean;
  inlineResponse?: boolean;
  actions?: React.ReactNode;
  governedActions?: React.ReactNode;
}) {
  const currentDetail = detail?.taskId === taskId ? detail : null;
  const [latestRunItems, setLatestRunItems] = React.useState<ReadonlyMap<string, TaskRunItem>>(
    () => new Map()
  );
  const onLatestRunItemChange = React.useCallback((runId: string, item: TaskRunItem | null) => {
    setLatestRunItems((previous) => {
      if (!item) {
        if (!previous.has(runId)) return previous;
        const next = new Map(previous);
        next.delete(runId);
        return next;
      }
      if (previous.get(runId) === item) return previous;
      const next = new Map(previous);
      next.set(runId, item);
      return next;
    });
  }, []);

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
          <TaskTranscript
            detail={currentDetail}
            liveRunItems={liveRunItems}
            onLatestRunItemChange={onLatestRunItemChange}
          />
        </div>
        <TaskContextCard
          key={`context:${taskId}:${currentDetail.attention ? "attention" : "info"}`}
          actions={actions}
          detail={currentDetail}
          governedActions={governedActions}
          inlineResponse={inlineResponse}
          latestRunItems={latestRunItems}
          controlsHostRef={controlsHostRef}
          showWorkLink={showWorkLink}
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
  inlineResponse,
  controlsHostRef,
  latestRunItems,
  showWorkLink,
  taskId
}: {
  taskId: string;
  detail: TaskDetail;
  actions?: React.ReactNode;
  governedActions?: React.ReactNode;
  inlineResponse: boolean;
  controlsHostRef?: React.RefObject<HTMLDivElement | null>;
  latestRunItems: ReadonlyMap<string, TaskRunItem>;
  showWorkLink: boolean;
}) {
  return (
    <aside aria-label="Task summary" {...stylex.props(styles.contextDock)}>
      <div {...stylex.props(styles.contextCard)}>
        <TaskSummaryHeader
          detail={detail}
          latestRunItems={latestRunItems}
          controlsHostRef={controlsHostRef}
          showWorkLink={showWorkLink}
          taskId={taskId}
        />
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

function TaskSummaryHeader({
  detail,
  latestRunItems,
  controlsHostRef,
  showWorkLink,
  taskId
}: {
  detail: TaskDetail;
  latestRunItems: ReadonlyMap<string, TaskRunItem>;
  controlsHostRef?: React.RefObject<HTMLDivElement | null>;
  showWorkLink: boolean;
  taskId: string;
}) {
  const run = latestTaskRun(detail);
  const latestItem = run ? latestRunItems.get(run.id) ?? null : null;
  return (
    <header {...stylex.props(styles.summaryHeader)}>
      {run ? (
        <IdentityAvatar actorId={`subagent:${run.instanceName}`} actorType="agent" size="sm" />
      ) : null}
      <span {...stylex.props(styles.summaryCopy)}>
        <strong {...stylex.props(styles.summaryTitle)}>
          {run ? `${run.instanceName} · ${capitalize(run.role)}` : "No agent run yet"}
        </strong>
        <span {...stylex.props(styles.summaryOutput)}>{latestRunOutput(run, latestItem)}</span>
      </span>
      <span {...stylex.props(styles.summaryActions)}>
        {controlsHostRef ? <span ref={controlsHostRef} {...stylex.props(styles.summaryControlsHost)} /> : null}
        {showWorkLink ? (
          <IconButton
            href={`/work/tasks/${encodeURIComponent(taskId)}`}
            icon={<ExternalLink aria-hidden="true" size={15} />}
            label="Open in Work"
            size="sm"
            tooltip="Open in Work"
            variant="ghost"
            xstyle={iconButtonXStyle(styles.summaryAction)}
          />
        ) : null}
        <TaskInfoTrigger detail={detail} />
      </span>
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
  return (
    <div {...stylex.props(styles.infoContent)}>
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

function latestTaskRun(detail: TaskDetail): TaskRun | null {
  const runs = detail.revisions
    .flatMap((revision) => [...revision.executors, ...revision.reviewers])
    .sort((left, right) => parseTimestamp(right.updatedAt ?? right.createdAt) - parseTimestamp(left.updatedAt ?? left.createdAt));
  return runs.find((run) => run.status === "running" || run.status === "leased" || run.status === "queued") ?? runs[0] ?? null;
}

function latestRunOutput(run: TaskRun | null, item: TaskRunItem | null): string {
  if (item) {
    return item.summary?.trim() || item.title;
  }
  if (run?.error) return run.error;
  switch (run?.status) {
    case "running": return "Running";
    case "leased": return "Starting";
    case "queued": return "Queued";
    case "completed": return run.output?.trim() || "Completed";
    case "failed": return "Failed";
    case "cancelled": return "Cancelled";
    case "interrupted": return "Interrupted";
    case "waiting_for_approval": return "Waiting for approval";
    default: return "No output yet";
  }
}

function parseTimestamp(value?: string | null): number {
  const timestamp = value ? Date.parse(value) : Number.NaN;
  return Number.isNaN(timestamp) ? 0 : timestamp;
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

function TaskUnavailable({ message }: { message: string }) {
  return <div role="status" {...stylex.props(styles.unavailable)}>{message}</div>;
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

function popoverXStyle(...xstyle: unknown[]): React.ComponentProps<typeof Popover>["xstyle"] {
  return xstyle as React.ComponentProps<typeof Popover>["xstyle"];
}

function iconButtonXStyle(...xstyle: unknown[]): React.ComponentProps<typeof IconButton>["xstyle"] {
  return xstyle as React.ComponentProps<typeof IconButton>["xstyle"];
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
  },
  contextDock: {
    display: "flex",
    flexDirection: "column",
    position: "relative",
    zIndex: 2,
    minWidth: 0,
    minHeight: 0,
    maxHeight: "30%",
    marginInline: "var(--spacing-4)",
    marginBlockEnd: "var(--spacing-4)",
    marginBlockStart: "calc(-1 * var(--spacing-2))"
  },
  contextCard: {
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
    minHeight: 0,
    maxHeight: "100%",
    flex: "1 1 auto",
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 24,
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "0 10px 28px color-mix(in srgb, var(--noema-text-primary) 13%, transparent)"
  },
  summaryHeader: { display: "grid", gridTemplateColumns: "auto minmax(0, 1fr) auto", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)" },
  summaryCopy: { display: "grid", minWidth: 0, gap: "var(--spacing-0-5)" },
  summaryTitle: { minWidth: 0, color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 700, lineHeight: 1.35, overflow: "hidden", overflowWrap: "anywhere", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  summaryOutput: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  summaryActions: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)" },
  summaryControlsHost: { display: "inline-flex", alignItems: "center" },
  summaryAction: { width: 28, height: 28 },
  infoButton: { display: "inline-flex", width: 28, height: 28, alignItems: "center", justifyContent: "center", borderWidth: 0, borderRadius: 999, backgroundColor: "transparent", color: "var(--noema-text-muted)", cursor: "pointer", ":hover": { backgroundColor: "var(--noema-surface-hover)", color: "var(--noema-text-primary)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  infoPopover: { maxHeight: "min(70vh, 520px)", overflowX: "hidden", overflowY: "auto", padding: 0, borderRadius: "var(--radius-container)" },
  actionRow: { minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)", ":empty": { display: "none" } },
  attention: { display: "grid", gap: "var(--spacing-2)", minWidth: 0, maxHeight: "min(50vh, 360px)", overflowX: "hidden", overflowY: "auto", overscrollBehavior: "contain", borderBlockWidth: 1, borderBlockStyle: "solid", borderBlockColor: "var(--noema-red-700)", paddingBlock: "var(--spacing-1)", paddingInline: "var(--spacing-4)" },
  attentionComposer: { display: "grid", gap: "var(--spacing-2)", minWidth: 0, paddingBlock: "var(--spacing-2)" },
  attentionComposerCopy: { display: "grid", gap: "var(--spacing-1)", minWidth: 0 },
  attentionTitle: { margin: 0, color: "var(--noema-red-700)", fontSize: 13, fontWeight: 700, lineHeight: 1.35 },
  attentionText: { margin: 0, color: "var(--noema-text-primary)", fontSize: 12, lineHeight: 1.45 },
  attentionMarkdown: { color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.45 },
  attentionPrimaryMarkdown: { color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 600, lineHeight: 1.45 },
  attentionGovernedActions: { minWidth: 0, ":empty": { display: "none" } },
  attentionActions: { minWidth: 0 },
  validationRow: { display: "grid", gridTemplateRows: "auto minmax(0, 1fr)", minWidth: 0, minHeight: 0, flex: "1 1 auto", overflow: "hidden", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  validationHeader: { display: "flex", width: "100%", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)" },
  validationIdentity: { display: "flex", minWidth: 0, alignItems: "baseline", gap: "var(--spacing-2)" },
  validationLabel: { color: "var(--noema-text-primary)", fontSize: 11, fontWeight: 700 },
  validationSummary: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 11, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  infoContent: { display: "grid", minWidth: 0, overflow: "hidden", borderRadius: "inherit", backgroundColor: "var(--noema-surface-card)" },
  metadataSection: { paddingBlock: "var(--spacing-3)", paddingInline: "var(--spacing-3)" },
  metadata: { display: "grid", gap: "var(--spacing-1-5)", margin: 0 },
  metadataRow: { display: "grid", gridTemplateColumns: "minmax(88px, 0.42fr) minmax(0, 1fr)", gap: "var(--spacing-3)", alignItems: "baseline" },
  metadataKey: { color: "var(--noema-text-muted)", fontSize: 11 },
  metadataValue: { minWidth: 0, margin: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  status: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
});
