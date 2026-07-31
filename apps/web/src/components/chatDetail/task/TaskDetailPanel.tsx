import * as React from "react";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Popover } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { ExternalLink, Info } from "lucide-react";
import { AnimatePresence, useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { IdentityAvatar } from "@/components/IdentityAvatar";
import { springs } from "@/motion/springs";
import type { TaskDetail, TaskRun, TaskRunItem, TaskRunStatus } from "./taskTypes";
import { TaskCriterionStatusPopover } from "./TaskCriteria";
import { TaskCompletedBody } from "./TaskCompletedBody";
import { taskStageLabel } from "./TaskOverview";
import { TaskTranscript } from "./TaskTranscript";

export function TaskDetailPanel({
  taskId,
  detail,
  loading = false,
  error = null,
  liveRunItems,
  controls,
  governedActions,
  onOpenDetail,
  showWorkLink = false
}: {
  taskId: string;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  controls?: React.ReactNode;
  showWorkLink?: boolean;
  governedActions?: React.ReactNode;
  onOpenDetail: (target: ChatDetailTarget) => void;
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
        <div role="status" {...stylex.props(styles.status)}>
          Loading task details...
        </div>
      </div>
    );
  }
  if (error && !currentDetail) {
    return <div {...stylex.props(styles.root)}><TaskUnavailable message={error} /></div>;
  }
  if (!currentDetail) {
    return <div {...stylex.props(styles.root)}><TaskUnavailable message="Task details are unavailable." /></div>;
  }

  const contextCard = (
    <TaskContextCard
      key={`context:${taskId}:${currentDetail.attention ? "attention" : "info"}`}
      detail={currentDetail}
      governedActions={governedActions}
      latestRunItems={latestRunItems}
      controls={controls}
      showWorkLink={showWorkLink}
      taskId={taskId}
    />
  );
  const completedResult = currentDetail.stageBehavior === "TERMINAL_SUCCESS" ? currentDetail.completedResult : null;

  return (
    <div data-slot="task-detail-view-viewport" {...stylex.props(styles.viewport)}>
      <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
        {completedResult ? (
          <TaskCompletedBody
            key={`completed:${currentDetail.taskId}:${completedResult.id}`}
            contextCard={contextCard}
            detail={currentDetail}
            liveRunItems={liveRunItems}
            onOpenDetail={onOpenDetail}
            onLatestRunItemChange={onLatestRunItemChange}
          />
        ) : (
          <div {...stylex.props(styles.transcriptRegion)}>
            <TaskTranscript
              detail={currentDetail}
              liveRunItems={liveRunItems}
              onOpenDetail={onOpenDetail}
              onLatestRunItemChange={onLatestRunItemChange}
            />
          </div>
        )}
        {!completedResult ? contextCard : null}
      </div>
    </div>
  );
}

function TaskContextCard({
  detail,
  governedActions,
  controls,
  latestRunItems,
  showWorkLink,
  taskId
}: {
  taskId: string;
  detail: TaskDetail;
  governedActions?: React.ReactNode;
  controls?: React.ReactNode;
  latestRunItems: ReadonlyMap<string, TaskRunItem>;
  showWorkLink: boolean;
}) {
  const hasValidation = detail.criteria.length > 0;
  return (
    <aside aria-label="Task summary" {...stylex.props(styles.contextDock)}>
      {governedActions}
      <div {...stylex.props(styles.contextCard)}>
        <TaskSummaryHeader
          detail={detail}
          latestRunItems={latestRunItems}
          controls={controls}
          showWorkLink={showWorkLink}
          taskId={taskId}
        />
        <div {...stylex.props(styles.contextBody)}>
          {hasValidation ? <TaskValidationRow criteria={detail.criteria} /> : null}
        </div>
      </div>
    </aside>
  );
}

function TaskSummaryHeader({
  detail,
  latestRunItems,
  controls,
  showWorkLink,
  taskId
}: {
  detail: TaskDetail;
  latestRunItems: ReadonlyMap<string, TaskRunItem>;
  controls?: React.ReactNode;
  showWorkLink: boolean;
  taskId: string;
}) {
  const run = latestTaskRun(detail);
  const latestItem = run ? latestRunItems.get(run.id) ?? null : null;
  return (
    <header {...stylex.props(styles.summaryHeader, !run && styles.summaryHeaderWithoutAvatar)}>
      {run ? (
        <span {...stylex.props(styles.summaryAvatar)}>
          <AnimatePresence initial={false}>
            <TaskSummaryAvatar key={run.id} run={run} />
          </AnimatePresence>
        </span>
      ) : null}
      <span {...stylex.props(styles.summaryCopy)}>
        <strong {...stylex.props(styles.summaryTitle)}>
          {run ? `${run.instanceName} · ${capitalize(run.role)}` : "No agent run yet"}
        </strong>
        <span {...stylex.props(styles.summaryOutput)}>{latestRunOutput(run, latestItem)}</span>
      </span>
      <span {...stylex.props(styles.summaryActions)}>
        {controls ? <span {...stylex.props(styles.summaryControlsHost)}>{controls}</span> : null}
        {showWorkLink ? (
          <IconButton
            href={`/work/tasks/${encodeURIComponent(taskId)}`}
            icon={<ExternalLink aria-hidden="true" size={15} />}
            label="Open in Tasks"
            size="sm"
            tooltip="Open in Tasks"
            variant="ghost"
            xstyle={iconButtonXStyle(styles.summaryAction)}
          />
        ) : null}
        <TaskInfoTrigger detail={detail} />
      </span>
    </header>
  );
}

function TaskSummaryAvatar({ run }: { run: TaskRun }) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  const avatarMotion = taskRunAvatarMotion(run.status);
  return (
    <m.span
      aria-hidden={!isPresent}
      inert={!isPresent}
      initial={reduceMotion ? false : { opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={reduceMotion ? { duration: 0 } : springs.standard}
      {...stylex.props(styles.summaryAvatarLayer)}
    >
      <IdentityAvatar
        activity={avatarMotion.activity}
        actorId={`subagent:${run.instanceName}`}
        actorType="agent"
        animated={avatarMotion.animated}
        size="sm"
      />
    </m.span>
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

function TaskValidationRow({ criteria }: { criteria: TaskDetail["criteria"] }) {
  return (
    <section aria-label="Validation" {...stylex.props(styles.validationRow)}>
      <div {...stylex.props(styles.validationHeader)}>
        <span {...stylex.props(styles.validationLabel)}>Validation</span>
        <span {...stylex.props(styles.validationStatuses)}>
          {criteria.map((criterion) => (
            <TaskCriterionStatusPopover key={criterion.id} criterion={criterion} />
          ))}
        </span>
      </div>
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

function taskRunAvatarMotion(status: TaskRunStatus) {
  switch (status) {
    case "queued":
    case "leased":
      return { activity: "listening", animated: true } as const;
    case "running":
      return { activity: "thinking", animated: true } as const;
    case "completed":
    case "waiting_for_approval":
    case "interrupted":
    case "failed":
    case "cancelled":
      return { activity: "idle", animated: false } as const;
  }
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
    position: "relative",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    backgroundColor: "var(--noema-surface-card)"
  },
  transcriptRegion: {
    minWidth: 0,
    minHeight: 0,
    "--task-transcript-bottom-inset": "var(--spacing-3)"
  },
  contextDock: {
    display: "flex",
    flexDirection: "column",
    position: "relative",
    zIndex: 2,
    minWidth: 0,
    minHeight: 0,
    marginInline: "var(--spacing-4)",
    marginBlockEnd: "var(--spacing-4)",
    marginBlockStart: "calc(-1 * var(--spacing-3))",
    "--human-intervention-card-radius": "24px",
    "--human-intervention-card-bottom-radius": "0px",
    "--human-intervention-card-overlap": "var(--human-intervention-card-radius)"
  },
  contextCard: {
    display: "flex",
    flexDirection: "column",
    position: "relative",
    zIndex: 1,
    minWidth: 0,
    minHeight: 0,
    flex: "0 0 auto",
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: "var(--human-intervention-card-radius)",
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "0 10px 28px color-mix(in srgb, var(--noema-text-primary) 13%, transparent)"
  },
  contextBody: { minWidth: 0, minHeight: 0 },
  summaryHeader: { display: "grid", gridTemplateColumns: "auto minmax(0, 1fr) auto", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)" },
  summaryHeaderWithoutAvatar: { gridTemplateColumns: "minmax(0, 1fr) auto" },
  summaryAvatar: { position: "relative", width: 28, height: 28 },
  summaryAvatarLayer: { position: "absolute", inset: 0, display: "flex" },
  summaryCopy: { display: "grid", minWidth: 0, gap: "var(--spacing-0-5)" },
  summaryTitle: { minWidth: 0, color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 700, lineHeight: 1.35, overflow: "hidden", overflowWrap: "anywhere", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  summaryOutput: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  summaryActions: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)" },
  summaryControlsHost: { display: "inline-flex", alignItems: "center" },
  summaryAction: { width: 28, height: 28 },
  infoButton: { display: "inline-flex", width: 28, height: 28, alignItems: "center", justifyContent: "center", borderWidth: 0, borderRadius: 999, cornerShape: "var(--corner-shape-full)", backgroundColor: "transparent", color: "var(--noema-text-muted)", cursor: "pointer", ":hover": { backgroundColor: "var(--noema-surface-hover)", color: "var(--noema-text-primary)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  infoPopover: { maxHeight: "min(70vh, 520px)", overflowX: "hidden", overflowY: "auto", padding: "var(--spacing-0)", borderRadius: "var(--radius-container)" },
  validationRow: { display: "grid", minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  validationHeader: { display: "flex", width: "100%", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)" },
  validationLabel: { color: "var(--noema-text-primary)", fontSize: 11, fontWeight: 700 },
  validationStatuses: { display: "inline-flex", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)" },
  infoContent: { display: "grid", minWidth: 0, overflow: "hidden", borderRadius: "inherit", backgroundColor: "var(--noema-surface-card)" },
  metadataSection: { paddingBlock: "var(--spacing-3)", paddingInline: "var(--spacing-3)" },
  metadata: { display: "grid", gap: "var(--spacing-1-5)", margin: "var(--spacing-0)" },
  metadataRow: { display: "grid", gridTemplateColumns: "minmax(88px, 0.42fr) minmax(0, 1fr)", gap: "var(--spacing-3)", alignItems: "baseline" },
  metadataKey: { color: "var(--noema-text-muted)", fontSize: 11 },
  metadataValue: { minWidth: 0, margin: "var(--spacing-0)", color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  status: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
});
