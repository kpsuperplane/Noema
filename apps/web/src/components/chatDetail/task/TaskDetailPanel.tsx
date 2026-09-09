import { taskActionBarStyles } from "@/components/tasks/taskActionBarStyles";
import * as React from "react";
import { AvatarGroup } from "@astryxdesign/core/AvatarGroup";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import * as stylex from "@stylexjs/stylex";
import { Check, ExternalLink } from "lucide-react";
import { AnimatePresence, useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { IdentityAvatar } from "@/components/IdentityAvatar";
import { RollingSwap, RollingText } from "@/components/RollingText";
import type { RenderTranscriptEntry } from "@/components/transcript/renderModel";
import { ToolMarker } from "@/components/transcript/ToolMarker";
import { springs } from "@/motion/springs";
import type { TaskDetail, TaskRun, TaskRunItem, TaskRunStatus } from "./taskTypes";
import { TaskBody, TaskLoadingSkeleton } from "./TaskBody";
import { TaskTranscriptSourceProvider } from "./TaskTranscript";
import type { TaskInlineEditController } from "@/components/tasks/TaskActions";

export function TaskDetailPanel({
  taskId,
  header,
  detail,
  loading = false,
  error = null,
  liveRunItems,
  controls,
  edit,
  renderSecondarySurface,
  onOpenDetail,
  showTasksLink = false,
  onRetry,
  retrying = false
}: {
  taskId: string;
  header?: React.ReactNode;
  detail?: TaskDetail | null;
  loading?: boolean;
  error?: string | null;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  controls?: React.ReactNode;
  edit?: TaskInlineEditController;
  showTasksLink?: boolean;
  onRetry?: () => void;
  retrying?: boolean;
  renderSecondarySurface?: () => React.ReactNode;
  onOpenDetail: (target: ChatDetailTarget) => void;
}) {
  const currentDetail = detail?.taskId === taskId ? detail : null;
  const [latestRunEntries, setLatestRunEntries] = React.useState<
    ReadonlyMap<string, RenderTranscriptEntry>
  >(
    () => new Map()
  );
  const onLatestRunEntryChange = React.useCallback((
    runId: string,
    entry: RenderTranscriptEntry | null
  ) => {
    setLatestRunEntries((previous) => {
      if (!entry) {
        if (!previous.has(runId)) return previous;
        const next = new Map(previous);
        next.delete(runId);
        return next;
      }
      if (previous.get(runId) === entry) return previous;
      const next = new Map(previous);
      next.set(runId, entry);
      return next;
    });
  }, []);

  if (loading && !currentDetail) {
    return <TaskLoadingSkeleton header={header} />;
  }
  if (error && !currentDetail) {
    return <div {...stylex.props(styles.root, styles.unavailableRoot)}>{header}<TaskUnavailable message={error} onRetry={onRetry} retrying={retrying} /></div>;
  }
  if (!currentDetail) {
    return <div {...stylex.props(styles.root, styles.unavailableRoot)}>{header}<TaskUnavailable message="Task details are unavailable." onRetry={onRetry} retrying={retrying} /></div>;
  }

  const run = latestTaskRun(currentDetail);
  const renderContextCard = () => (
    <TaskContextCard
      key={`context:${taskId}:${currentDetail.attention ? "attention" : "info"}`}
      detail={currentDetail}
      latestRunEntries={latestRunEntries}
      renderSecondarySurface={renderSecondarySurface}
      run={run}
      controls={controls}
      showTasksLink={showTasksLink}
      taskId={taskId}
    />
  );
  return (
    <div data-slot="task-detail-view-viewport" {...stylex.props(styles.viewport)}>
      <div data-task-id={currentDetail.taskId} {...stylex.props(styles.root)}>
        <TaskTranscriptSourceProvider
          liveItems={run ? liveRunItems?.get(run.id) : undefined}
          onLatestRunEntryChange={onLatestRunEntryChange}
          run={run}
          taskId={taskId}
        >
          <TaskBody
            header={header}
            key={`task:${currentDetail.taskId}`}
            detail={currentDetail}
            edit={edit}
            liveRunItems={liveRunItems}
            onOpenDetail={onOpenDetail}
            onLatestRunEntryChange={onLatestRunEntryChange}
            renderContextCard={renderContextCard}
          />
        </TaskTranscriptSourceProvider>
      </div>
    </div>
  );
}

function TaskContextCard({
  detail,
  controls,
  latestRunEntries,
  renderSecondarySurface,
  run,
  showTasksLink,
  taskId
}: {
  taskId: string;
  detail: TaskDetail;
  controls?: React.ReactNode;
  latestRunEntries: ReadonlyMap<string, RenderTranscriptEntry>;
  renderSecondarySurface?: () => React.ReactNode;
  run: TaskRun | null;
  showTasksLink: boolean;
}) {
  const latestEntry = run ? latestRunEntries.get(run.id) ?? null : null;
  return (
    <aside aria-label="Task summary" {...stylex.props(taskActionBarStyles.contextDock)}>
      {renderSecondarySurface?.()}
      <div {...stylex.props(taskActionBarStyles.contextCard)}>
        <TaskSummaryHeader
          entry={latestEntry}
          detail={detail}
          run={run}
          controls={controls}
          showTasksLink={showTasksLink}
          taskId={taskId}
        />
      </div>
    </aside>
  );
}

function TaskSummaryHeader({
  entry,
  detail,
  run,
  controls,
  showTasksLink,
  taskId
}: {
  entry: RenderTranscriptEntry | null;
  detail: TaskDetail;
  run: TaskRun | null;
  controls?: React.ReactNode;
  showTasksLink: boolean;
  taskId: string;
}) {
  const completed = detail.status === "done";
  const active = detail.stageBehavior === "ACTIVE" && (run?.status === "running" || run?.status === "leased");
  return (
    <header {...stylex.props(styles.summaryHeader, !run && !completed && styles.summaryHeaderWithoutAvatar)}>
      {run || completed ? (
        <span {...stylex.props(styles.summaryAvatar)}>
          <AnimatePresence initial={false}>
            <TaskSummaryAvatar
              key={completed ? "task-completed" : run?.id}
              completed={completed}
              run={run}
            />
          </AnimatePresence>
        </span>
      ) : null}
      <div {...stylex.props(styles.summaryCopy)}>
        <strong {...stylex.props(styles.summaryTitle)}>
          {taskStateLabel(detail, run)}
        </strong>
        {completed && detail.contributorInstanceNames.length ? (
          <AvatarGroup aria-label="Agents that worked on this task" size="xsm">
            {detail.contributorInstanceNames.map((instanceName, index) => (
              <IdentityAvatar
                key={instanceName}
                actorId={`subagent:${instanceName}`}
                actorType="agent"
                className={stylex.props(styles.contributorAvatar, index > 0 && styles.contributorAvatarOverlap).className}
                focusable={false}
                label={instanceName}
                size="xs"
              />
            ))}
          </AvatarGroup>
        ) : !completed && detail.status !== "cancelled" && detail.attention?.summary.trim() ? (
          <span {...stylex.props(styles.summaryOutput)}>{detail.attention.summary}</span>
        ) : !detail.attention && active ? <TaskSummaryEntry entry={entry} /> : null}
      </div>
      <span {...stylex.props(styles.summaryActions)}>
        {controls ? <span {...stylex.props(styles.summaryControlsHost)}>{controls}</span> : null}
        {showTasksLink ? (
          <IconButton
            href={`/tasks/${encodeURIComponent(taskId)}`}
            icon={<ExternalLink aria-hidden="true" size={15} />}
            label="Open in Tasks"
            size="sm"
            tooltip="Open in Tasks"
            variant="ghost"
            xstyle={styles.summaryAction}
          />
        ) : null}
      </span>
    </header>
  );
}

function TaskSummaryAvatar({
  completed,
  run
}: {
  completed: boolean;
  run: TaskRun | null;
}) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  const avatarMotion = run ? taskRunAvatarMotion(run.status) : null;
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
      {completed ? (
        <span {...stylex.props(styles.completedAvatar)}>
          <Check aria-hidden="true" size={16} strokeWidth={2.5} />
        </span>
      ) : run && avatarMotion ? (
        <IdentityAvatar
          activity={avatarMotion.activity}
          actorId={`subagent:${run.instanceName}`}
          actorType="agent"
          label={run.instanceName}
          animated={avatarMotion.animated}
          size="sm"
        />
      ) : null}
    </m.span>
  );
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

function TaskSummaryEntry({ entry }: { entry: RenderTranscriptEntry | null }) {
  if (entry?.kind === "tool_marker" || entry?.kind === "tool_marker_group") {
    return (
      <RollingSwap transitionKey="tool" {...stylex.props(styles.summaryEntry)}>
        <ToolMarker
          animateText
          data={entry.kind === "tool_marker"
            ? { kind: "tool", marker: entry.marker }
            : { kind: "tool_group", markers: entry.markers }}
          interactive={false}
          onToggle={() => {}}
          open={false}
          renderDetail={false}
        />
      </RollingSwap>
    );
  }
  const text = entry?.kind === "entry"
    ? entry.entry.type === "assistant"
      ? entry.entry.text
      : entry.entry.type === "activity"
        ? entry.entry.item.summary || entry.entry.item.title
        : null
    : null;
  return text?.trim() ? <RollingText {...stylex.props(styles.summaryOutput)} value={text.trim()} /> : null;
}

function taskStateLabel(detail: TaskDetail, run: TaskRun | null): string {
  if (detail.status === "done") return "Completed";
  if (detail.status === "cancelled") return "Cancelled";
  if (detail.attention) {
    switch (detail.attention.kind) {
      case "APPROVAL_REQUIRED": return "Needs your approval";
      case "CLARIFICATION_REQUIRED": return "Needs your answer";
      case "RECOVERY_REQUIRED": return "Needs your help";
    }
  }
  if (detail.status === "failed") return "Failed";
  if (detail.status === "waiting_for_human" || detail.stageBehavior === "HUMAN_GATE") return "Needs your help";
  if (detail.stageBehavior === "INTAKE") return detail.schedule ? "Scheduled" : "Ready when you are";
  if (detail.stageBehavior === "DISPATCH") return "Queued";
  if (detail.stageBehavior === "ACTIVE" && run?.status === "failed") return "Failed";
  if (detail.stageBehavior === "ACTIVE" && (run?.status === "running" || run?.status === "leased") && run.role === "planner") return "Planning";
  if (detail.status === "reviewing" || (detail.stageBehavior === "ACTIVE" && run?.role === "reviewer")) return "Reviewing";
  return "Working";
}

function parseTimestamp(value?: string | null): number {
  const timestamp = value ? Date.parse(value) : Number.NaN;
  return Number.isNaN(timestamp) ? 0 : timestamp;
}

function TaskUnavailable({ message, onRetry, retrying }: { message: string; onRetry?: () => void; retrying: boolean }) {
  return (
    <div role={onRetry ? undefined : "status"} {...stylex.props(styles.unavailable)}>
      <p role={onRetry ? "alert" : undefined}>{message}</p>
      {onRetry ? <Button type="button" size="sm" variant="secondary" label="Retry" isLoading={retrying} isDisabled={retrying} onClick={onRetry} /> : null}
    </div>
  );
}

const styles = stylex.create({
  unavailableRoot: { gridTemplateRows: "auto minmax(0, 1fr)" },
  viewport: { minWidth: 0, minHeight: 0, height: "100%", overflow: "hidden" },
  root: {
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr)",
    position: "relative",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    backgroundColor: "var(--noema-surface-card)"
  },
  summaryHeader: { height: "calc(2 * var(--spacing-6) + var(--spacing-2))", boxSizing: "border-box", display: "grid", gridTemplateColumns: "auto minmax(0, 1fr) auto", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", paddingInlineStart: "calc(var(--spacing-3) + var(--spacing-0-5))", paddingInlineEnd: "var(--spacing-4)" },
  summaryHeaderWithoutAvatar: { gridTemplateColumns: "minmax(0, 1fr) auto", paddingInlineStart: "var(--spacing-4)" },
  summaryAvatar: { position: "relative", width: 28, height: 28 },
  summaryAvatarLayer: { position: "absolute", inset: 0, display: "flex" },
  completedAvatar: { display: "inline-flex", width: "100%", height: "100%", alignItems: "center", justifyContent: "center", borderRadius: 999, cornerShape: "var(--corner-shape-full)", backgroundColor: "var(--color-success)", color: "var(--color-on-accent)" },
  contributorAvatar: { boxSizing: "content-box", borderWidth: 2, borderStyle: "solid", borderColor: "var(--noema-surface-card)" },
  contributorAvatarOverlap: { marginInlineStart: "calc(-1 * var(--spacing-1))" },
  summaryCopy: { display: "grid", minWidth: 0, gap: "var(--spacing-0)" },
  summaryTitle: { minWidth: 0, color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 700, lineHeight: 1.35, overflow: "hidden", overflowWrap: "anywhere", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  summaryOutput: { marginBlockStart: "calc(-1 * var(--spacing-0-5))", minWidth: 0, overflow: "hidden", color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: "var(--spacing-6)", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  summaryEntry: { marginBlockStart: "calc(-1 * var(--spacing-0-5))", display: "block", minWidth: 0, height: "var(--spacing-6)", overflow: "hidden", whiteSpace: "nowrap" },
  summaryActions: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)" },
  summaryControlsHost: { display: "inline-flex", alignItems: "center" },
  summaryAction: { width: 28, height: 28 },
  status: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  unavailable: { padding: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.45 },
});
