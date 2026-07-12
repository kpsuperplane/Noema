import { ChatToolCalls, type ChatToolCallStatus } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";
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
    <section aria-label="Timeline" id="task-timeline-title" tabIndex={-1} {...stylex.props(styles.section)}>
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
                <ChatToolCalls
                  aria-label={`Open ${label} conversation, Revision ${revision.revision}, Attempt ${run.attemptIndex + 1}`}
                  calls={[{
                    key: run.id,
                    name: label,
                    target: `Revision ${revision.revision} · Attempt ${run.attemptIndex + 1}`,
                    status,
                    duration: status === "complete" ? duration : undefined,
                    stats: status === "complete" ? undefined : duration,
                    errorMessage: run.error ?? undefined
                  }]}
                  onClick={openRun}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      openRun();
                    }
                  }}
                  role="button"
                  tabIndex={0}
                  {...stylex.props(styles.toolCall)}
                />
              </li>
            );
          })}
        </ol>
      )}
    </section>
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

function runToolCallStatus(run: TaskRun, review?: TaskReview | null): ChatToolCallStatus {
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
  section: {
    minWidth: 0,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)",
    paddingBlock: 10,
    paddingInline: 8
  },
  timeline: {
    display: "grid",
    gap: 4,
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  item: { minWidth: 0 },
  toolCall: {
    width: "100%",
    borderRadius: 8,
    cursor: "pointer",
    ":hover": { backgroundColor: "var(--noema-surface-hover)" },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)",
      outlineOffset: -2
    }
  },
  empty: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 }
});
