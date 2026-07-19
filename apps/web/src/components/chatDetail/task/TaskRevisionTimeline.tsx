import * as stylex from "@stylexjs/stylex";
import * as React from "react";
import { TaskStaticSection } from "./TaskSection";
import { TaskToolMarker, type TaskToolMarkerStatus } from "./TaskToolMarker";
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
    <TaskStaticSection id="task-timeline-title" tabIndex={-1} title="Timeline">
      {entries.length === 0 ? (
        <p {...stylex.props(styles.empty)}>Executor and review activity will appear here.</p>
      ) : (
        <ol {...stylex.props(styles.timeline)}>
          {entries.map(({ revision, run }) => {
            const label = runTimelineLabel(run, revision.review);
            const status = runToolCallStatus(run, revision.review);
            const duration = runDurationLabel(run, now);
            const openRun = () => onSelectRun?.(run);
            return (
              <li key={run.id} {...stylex.props(styles.item)}>
                <TaskToolMarker
                  activationLabel={`Open ${label} conversation`}
                  errorMessage={run.error ?? undefined}
                  id={run.id}
                  name={label}
                  onActivate={openRun}
                  status={status}
                  target={duration}
                />
              </li>
            );
          })}
        </ol>
      )}
    </TaskStaticSection>
  );
}

export function runTimelineLabel(run: TaskRun, review?: TaskReview | null): string {
  if (run.role === "planner") {
    return terminalRoleLabel("Planner", run.status);
  }
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

function runToolCallStatus(run: TaskRun, review?: TaskReview | null): TaskToolMarkerStatus {
  const status = run.status;
  if (status === "failed" || status === "cancelled" || status === "interrupted") {
    return "error";
  }
  if (status === "running") {
    return "running";
  }
  if (status !== "completed") {
    return "pending";
  }
  if (run.role === "reviewer" && review?.reviewerRunId === run.id && review.verdict === "request_changes") {
    return "error";
  }
  if (run.role === "reviewer" && review?.reviewerRunId === run.id && review.verdict === "needs_human") {
    return "pending";
  }
  return "complete";
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
  empty: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 }
});
