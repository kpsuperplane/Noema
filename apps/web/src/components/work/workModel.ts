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

export function eventLabel(event: WorkEvent): string {
  const taskTitle = stringField(event.payload, "title");
  const labels: Record<string, string> = {
    "task.captured": "Task captured",
    "task.updated": "Task updated",
    "task.queued": "Task queued",
    "task.stage_changed": "Task stage changed",
    "task.cancelled": "Task cancelled",
    "task.reopened": "Task reopened",
    "contract.created": "Contract created",
    "run.queued": "Run queued",
    "run.claimed": "Run claimed",
    "run.started": "Run started",
    "run.completed": "Run completed",
    "run.failed": "Run needs recovery",
    "gate.opened": "Human input requested",
    "gate.resolved": "Human input received",
    "submission.created": "Result submitted",
    "review.created": "Review completed",
    "task.accepted": "Result accepted"
  };
  const label = labels[event.kind] ?? sentenceCase(event.kind);
  return taskTitle ? `${label}: ${taskTitle}` : label;
}

export function sentenceCase(value: string): string {
  const words = value.replaceAll(".", " ").replaceAll("_", " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

function stringField(value: unknown, key: string): string | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const field = (value as Record<string, unknown>)[key];
  return typeof field === "string" && field.trim() ? field : null;
}
