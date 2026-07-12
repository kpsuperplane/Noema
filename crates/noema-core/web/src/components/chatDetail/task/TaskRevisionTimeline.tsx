import * as stylex from "@stylexjs/stylex";
import { ChevronRight } from "lucide-react";
import * as React from "react";
import { TaskStaticSection } from "./TaskDisclosureSection";
import type { TaskRevision, TaskReview, TaskRun, TaskRunStatus } from "./taskTypes";

type TimelineEntry = {
  revision: TaskRevision;
  run: TaskRun;
};

export function TaskRevisionTimeline({
  revisions,
  onSelectRun
}: {
  revisions: readonly TaskRevision[];
  onSelectRun?: (run: TaskRun) => void;
}) {
  const entries = timelineEntries(revisions);
  const now = useTaskRunClock(entries.some(({ run }) => run.status === "running"));

  return (
    <TaskStaticSection count={entries.length} id="task-timeline-title" tabIndex={-1} title="Timeline">
      {entries.length === 0 ? (
        <p {...stylex.props(styles.empty)}>Executor and review activity will appear here.</p>
      ) : (
        <ol {...stylex.props(styles.timeline)}>
          {entries.map(({ revision, run }) => (
            <li key={run.id} {...stylex.props(styles.item)}>
              <button
                type="button"
                aria-label={`Open ${runTimelineLabel(run, revision.review)} conversation, Revision ${revision.revision}, Attempt ${run.attemptIndex + 1}`}
                onClick={() => onSelectRun?.(run)}
                {...stylex.props(styles.itemButton)}
              >
                <span aria-hidden="true" {...stylex.props(styles.dot, runDotStyle(run.status))} />
                <span {...stylex.props(styles.copy)}>
                  <span {...stylex.props(styles.label)}>{runTimelineLabel(run, revision.review)}</span>
                  <span {...stylex.props(styles.meta)}>
                    Revision {revision.revision} · Attempt {run.attemptIndex + 1}
                  </span>
                </span>
                <span {...stylex.props(styles.duration)}>{runDurationLabel(run, now)}</span>
                <ChevronRight aria-hidden="true" size={15} {...stylex.props(styles.chevron)} />
              </button>
            </li>
          ))}
        </ol>
      )}
    </TaskStaticSection>
  );
}

export function runTimelineLabel(run: TaskRun, review?: TaskReview | null): string {
  if (run.role === "executor") {
    return terminalRoleLabel("Executor", run.status);
  }
  if (run.status === "failed" || run.status === "cancelled" || run.status === "interrupted") {
    return terminalRoleLabel("Review", run.status);
  }
  if (review?.reviewerRunId === run.id) {
    switch (review.verdict) {
      case "approve":
        return "Review, Passed";
      case "request_changes":
        return "Review, Failed";
      case "needs_human":
        return "Review, Needs input";
    }
  }
  return terminalRoleLabel("Review", run.status);
}

export function runDurationLabel(run: TaskRun, now = Date.now()): string {
  const activeMilliseconds = Math.max(0, run.activeMilliseconds ?? 0);
  const startedAt = parseTimestamp(run.startedAt ?? run.createdAt);
  const endedAt = parseTimestamp(run.completedAt ?? run.updatedAt);
  const measuredMilliseconds = activeMilliseconds > 0
    ? activeMilliseconds
    : startedAt === null
      ? 0
      : Math.max(0, (run.status === "running" ? now : endedAt ?? now) - startedAt);
  const seconds = Math.max(0, Math.round(measuredMilliseconds / 1_000));
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes > 0 ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
}

export function useTaskRunClock(active: boolean): number {
  const [now, setNow] = React.useState(() => Date.now());

  React.useEffect(() => {
    if (!active) {
      return;
    }
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, [active]);

  return now;
}

function timelineEntries(revisions: readonly TaskRevision[]): TimelineEntry[] {
  return revisions
    .flatMap((revision) => [
      ...revision.executors.map((run) => ({ revision, run })),
      ...revision.reviewers.map((run) => ({ revision, run }))
    ])
    .sort((left, right) => {
      const leftTimestamp = parseTimestamp(left.run.createdAt) ?? Number.MAX_SAFE_INTEGER;
      const rightTimestamp = parseTimestamp(right.run.createdAt) ?? Number.MAX_SAFE_INTEGER;
      if (leftTimestamp !== rightTimestamp) {
        return leftTimestamp - rightTimestamp;
      }
      if (left.revision.revision !== right.revision.revision) {
        return left.revision.revision - right.revision.revision;
      }
      return left.run.attemptIndex - right.run.attemptIndex;
    });
}

function terminalRoleLabel(label: string, status: TaskRunStatus): string {
  switch (status) {
    case "failed":
      return `${label}, Failed`;
    case "cancelled":
      return `${label}, Cancelled`;
    case "interrupted":
      return `${label}, Interrupted`;
    default:
      return label;
  }
}

function runDotStyle(status: TaskRunStatus) {
  if (status === "completed") {
    return styles.dotSuccess;
  }
  if (status === "failed" || status === "cancelled" || status === "interrupted") {
    return styles.dotError;
  }
  if (status === "running") {
    return styles.dotRunning;
  }
  return styles.dotPending;
}

function parseTimestamp(value?: string | null): number | null {
  if (!value) {
    return null;
  }
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? null : timestamp;
}

const styles = stylex.create({
  timeline: {
    display: "grid",
    gap: 4,
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  item: { minWidth: 0 },
  itemButton: {
    display: "grid",
    gridTemplateColumns: "8px minmax(0, 1fr) auto 15px",
    alignItems: "center",
    width: "100%",
    gap: 10,
    border: 0,
    borderRadius: 8,
    backgroundColor: "transparent",
    padding: 10,
    color: "var(--noema-text-primary)",
    font: "inherit",
    textAlign: "left",
    cursor: "pointer",
    ":hover": { backgroundColor: "var(--noema-surface-hover)" },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)",
      outlineOffset: -2
    }
  },
  dot: { width: 8, height: 8, borderRadius: 999 },
  dotSuccess: { backgroundColor: "var(--noema-green-600)" },
  dotError: { backgroundColor: "var(--noema-red-600)" },
  dotRunning: { backgroundColor: "var(--noema-pine-600)" },
  dotPending: { backgroundColor: "var(--noema-text-muted)" },
  copy: { display: "grid", minWidth: 0, gap: 2 },
  label: { minWidth: 0, fontSize: 12, fontWeight: 700, overflowWrap: "anywhere" },
  meta: { minWidth: 0, color: "var(--noema-text-muted)", fontSize: 10, overflowWrap: "anywhere" },
  duration: { color: "var(--noema-text-secondary)", fontFamily: "var(--noema-font-mono)", fontSize: 11, fontVariantNumeric: "tabular-nums", whiteSpace: "nowrap" },
  chevron: { color: "var(--noema-text-muted)" },
  empty: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 }
});
