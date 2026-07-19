import * as stylex from "@stylexjs/stylex";
import * as React from "react";
import { Message } from "@/components/transcript/Message";
import { TranscriptChatBubble } from "@/components/transcript/TranscriptChatBubble";
import { TaskToolMarker, type TaskToolMarkerStatus } from "./TaskToolMarker";
import type { TaskDetail, TaskRevision, TaskReview, TaskRun, TaskRunStatus } from "./taskTypes";

type TimelineEntry = {
  revision: TaskRevision;
  run: TaskRun;
};

type TaskMessage = NonNullable<TaskDetail["messages"]>[number];
type ActivityEntry =
  | { kind: "run"; id: string; occurredAt?: string | null; revision: TaskRevision; run: TaskRun }
  | { kind: "message"; id: string; occurredAt: string; message: TaskMessage };

export function TaskCurrentRun({
  revisions,
  onSelectRun
}: {
  revisions: readonly TaskRevision[];
  onSelectRun?: (run: TaskRun) => void;
}) {
  const entries = timelineEntries(revisions);
  const currentRunId = revisions.find((revision) => revision.latestRunId)?.latestRunId;
  const entry = entries.find(({ run }) => run.id === currentRunId);
  const now = useTaskRunClock(entry?.run.status === "running");

  if (!entry) {
    return null;
  }

  const { revision, run } = entry;
  const label = runTimelineLabel(run, revision.review);
  return (
    <TaskToolMarker
      activationLabel={`Open ${label} conversation`}
      errorMessage={run.error ?? undefined}
      id={`current-${run.id}`}
      name={label}
      onActivate={() => onSelectRun?.(run)}
      status={runToolCallStatus(run, revision.review)}
      target={runDurationLabel(run, now)}
    />
  );
}

export function TaskActivityTimeline({
  revisions,
  messages,
  onSelectRun,
  excludeRunId = null
}: {
  revisions: readonly TaskRevision[];
  messages: readonly TaskMessage[];
  onSelectRun: (run: TaskRun) => void;
  excludeRunId?: string | null;
}) {
  const entries = activityEntries(revisions, messages, excludeRunId);
  const now = useTaskRunClock(entries.some((entry) => entry.kind === "run" && entry.run.status === "running"));

  if (entries.length === 0) {
    return null;
  }

  return (
    <ol {...stylex.props(styles.timeline)}>
      {entries.map((entry) => {
        if (entry.kind === "message") {
          return (
            <li key={entry.id} {...stylex.props(styles.messageItem)}>
              <Message
                animate={false}
                reserveAvatarSpace={false}
                role="user"
                showAvatar={false}
                text={entry.message.body}
              />
              <time dateTime={entry.message.createdAt} {...stylex.props(styles.messageTime)}>
                {formatActivityDate(entry.message.createdAt)}
              </time>
            </li>
          );
        }

        const label = runTimelineLabel(entry.run, entry.revision.review);
        return (
          <li key={entry.id} {...stylex.props(styles.item)}>
            <TranscriptChatBubble interactive reserveAvatarSpace={false} role="assistant" showAvatar={false}>
              <button
                type="button"
                aria-label={`Open ${label} conversation`}
                title={`Open ${label} conversation`}
                onClick={() => onSelectRun(entry.run)}
                {...stylex.props(styles.runButton)}
              >
                <span {...stylex.props(styles.runLabel)}>{label}</span>
                <span {...stylex.props(styles.runDuration)}>{runDurationLabel(entry.run, now)}</span>
              </button>
            </TranscriptChatBubble>
          </li>
        );
      })}
    </ol>
  );
}

export function taskActivityEntryCount(
  revisions: readonly TaskRevision[],
  messages: readonly TaskMessage[],
  excludeRunId: string | null = null
): number {
  return activityEntries(revisions, messages, excludeRunId).length;
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

function activityEntries(
  revisions: readonly TaskRevision[],
  messages: readonly TaskMessage[],
  excludeRunId: string | null
): ActivityEntry[] {
  const entries: ActivityEntry[] = [
    ...timelineEntries(revisions)
      .filter(({ run }) => run.id !== excludeRunId)
      .map(({ revision, run }) => ({ kind: "run" as const, id: run.id, occurredAt: run.createdAt, revision, run })),
    ...messages.map((message) => ({
      kind: "message" as const,
      id: message.id,
      occurredAt: message.createdAt,
      message
    }))
  ];

  return entries.sort((left, right) => {
    const leftTimestamp = parseTimestamp(left.occurredAt) ?? Number.MAX_SAFE_INTEGER;
    const rightTimestamp = parseTimestamp(right.occurredAt) ?? Number.MAX_SAFE_INTEGER;
    return leftTimestamp - rightTimestamp || left.id.localeCompare(right.id);
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

function formatActivityDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp)
    ? value
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

const styles = stylex.create({
  timeline: {
    display: "grid",
    gap: "var(--spacing-2)",
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  item: { minWidth: 0 },
  messageItem: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-0-5)",
    justifyItems: "end"
  },
  messageTime: {
    paddingInlineEnd: "var(--spacing-2)",
    color: "var(--noema-text-faint)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 9,
    lineHeight: 1.35,
    whiteSpace: "nowrap"
  },
  runButton: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-2)",
    marginBlock: "calc(-1 * var(--spacing-2))",
    marginInline: "calc(-1 * var(--spacing-4))",
    borderWidth: 0,
    borderRadius: 6,
    backgroundColor: "transparent",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-4)",
    color: "inherit",
    font: "inherit",
    outline: "none",
    textAlign: "left",
    cursor: "pointer"
  },
  runLabel: {
    minWidth: 0,
    color: "var(--noema-text-primary)",
    fontWeight: 650,
    overflowWrap: "anywhere"
  },
  runDuration: {
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10,
    fontVariantNumeric: "tabular-nums",
    whiteSpace: "nowrap"
  }
});
