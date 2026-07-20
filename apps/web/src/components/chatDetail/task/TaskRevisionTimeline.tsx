import * as React from "react";
import type { TaskReview, TaskRun, TaskRunStatus } from "./taskTypes";

export function runTimelineLabel(run: TaskRun, review?: TaskReview | null): string {
  if (run.role === "planner") {
    return terminalRoleLabel(`${run.instanceName} · Planner`, run.status);
  }
  if (run.role === "executor") {
    return terminalRoleLabel(`${run.instanceName} · Executor`, run.status);
  }
  if (run.status === "failed" || run.status === "cancelled" || run.status === "interrupted") {
    return terminalRoleLabel(`${run.instanceName} · Review`, run.status);
  }
  if (review?.reviewerRunId === run.id) {
    switch (review.verdict) {
      case "approve":
        return `${run.instanceName} · Review, Passed`;
      case "request_changes":
        return `${run.instanceName} · Review, Failed`;
      case "needs_human":
        return `${run.instanceName} · Review, Needs input`;
    }
  }
  return terminalRoleLabel(`${run.instanceName} · Review`, run.status);
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

function parseTimestamp(value?: string | null): number | null {
  if (!value) {
    return null;
  }
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? null : timestamp;
}
