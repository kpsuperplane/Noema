import type { WorkEvent, WorkTask } from "./workTypes";

export function relativeTime(value: string, now = Date.now()): string {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "recently";
  const minutes = Math.max(0, Math.floor((now - timestamp) / 60_000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  return `${days}d`;
}

export function timestampLabel(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp)
    ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp)
    : value;
}

export function taskRunLabel(task: Pick<WorkTask, "currentRun">): string | null {
  return task.currentRun?.activityLabel ?? null;
}

const DISPLAYED_ACTIVITY_KINDS = new Set([
  "project.created",
  "project.updated",
  "project.archived",
  "project.reopened",
  "task.captured",
  "task.updated",
  "task.queued",
  "task.stage_changed",
  "task.cancelled",
  "task.reopened",
  "task.accepted",
  "task.message_appended",
  "contract.created",
  "gate.opened",
  "gate.resolved",
  "gate.superseded",
  "run.completed",
  "run.interrupted",
  "run.failed",
  "run.cancelled",
  "submission.created",
  "review.created"
]);

export function isDisplayedActivityEvent(event: Pick<WorkEvent, "kind">): boolean {
  return DISPLAYED_ACTIVITY_KINDS.has(event.kind);
}

export function eventLabel(event: WorkEvent): string {
  const labels: Record<string, string> = {
    "project.created": "Project created",
    "project.updated": "Project updated",
    "project.archived": "Project archived",
    "project.reopened": "Project reopened",
    "task.captured": "Task captured",
    "task.updated": "Task updated",
    "task.queued": "Task queued",
    "task.stage_changed": "Task stage changed",
    "task.cancelled": "Task cancelled",
    "task.reopened": "Task reopened",
    "contract.created": "Contract created",
    "task.message_appended": "Task update added",
    "run.completed": "Run completed",
    "run.interrupted": "Run interrupted",
    "run.failed": "Run needs recovery",
    "run.cancelled": "Run cancelled",
    "gate.opened": "Human input requested",
    "gate.resolved": "Human input received",
    "gate.superseded": "Human request superseded",
    "submission.created": "Result submitted",
    "review.created": "Review completed",
    "task.accepted": "Result accepted"
  };
  return labels[event.kind] ?? sentenceCase(event.kind);
}

export function compactTaskIdentity(taskId: string): string {
  return compactIdentity("Task", taskId);
}

export function terminalRunIdentity(event: Pick<WorkEvent, "kind" | "runId">): string | null {
  if (!event.runId || !TERMINAL_RUN_KINDS.has(event.kind)) return null;
  return compactIdentity("Run", event.runId);
}

export function sentenceCase(value: string): string {
  const words = value.replaceAll(".", " ").replaceAll("_", " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

const TERMINAL_RUN_KINDS = new Set([
  "run.completed",
  "run.interrupted",
  "run.failed",
  "run.cancelled"
]);

function compactIdentity(noun: string, value: string): string {
  const segment = value.split(":").at(-1) ?? value;
  return `${noun} ${segment.length > 6 ? `…${segment.slice(-6)}` : segment}`;
}
