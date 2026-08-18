import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { TaskStatusIcon } from "./TaskStatusIcon";
import type { TaskStageBehavior, TaskStatus } from "./taskTypes";

type TaskStatusMeta = {
  label: string;
  tone: "neutral" | "info" | "success" | "warning" | "error";
};

const statusMeta: Record<TaskStatus, TaskStatusMeta> = {
  queued: { label: "Queued", tone: "neutral" },
  executing: { label: "Working", tone: "info" },
  reviewing: { label: "Reviewing", tone: "info" },
  revision_requested: { label: "Revising", tone: "warning" },
  waiting_for_human: { label: "Needs you", tone: "warning" },
  done: { label: "Done", tone: "success" },
  failed: { label: "Failed", tone: "error" },
  cancelled: { label: "Cancelled", tone: "neutral" }
};

function taskStatusMeta(status: TaskStatus): TaskStatusMeta {
  return statusMeta[status] ?? statusMeta.queued;
}

export function TaskStatusBadge({
  status,
  stageBehavior,
  label: labelOverride
}: {
  status: TaskStatus;
  stageBehavior?: TaskStageBehavior;
  label?: string;
}) {
  const meta = taskStatusMeta(status);
  const label = labelOverride ?? stageStatusLabel(stageBehavior) ?? meta.label;
  const toneStyle = meta.tone === "info" ? styles.info
    : meta.tone === "success" ? styles.success
    : meta.tone === "warning" ? styles.warning
    : meta.tone === "error" ? styles.error
    : styles.neutral;
  return (
    <Badge
      aria-label={`Task status: ${label}`}
      icon={<TaskStatusIcon status={status} />}
      label={label}
      variant="neutral"
      xstyle={[styles.badge, toneStyle]}
    />
  );
}

export function taskStatusFromProjection(task: {
  completedAt?: string | null;
  attention?: unknown | null;
  stage: { behavior: TaskStageBehavior };
  currentRun?: { kind: string } | null;
}): TaskStatus {
  if (task.completedAt || task.stage.behavior === "TERMINAL_SUCCESS") return "done";
  if (task.attention || task.stage.behavior === "HUMAN_GATE") return "waiting_for_human";
  if (task.stage.behavior === "TERMINAL_CANCELLED") return "cancelled";
  if (task.stage.behavior === "ACTIVE") {
    if (task.currentRun?.kind === "REVIEWER") return "reviewing";
    return "executing";
  }
  return "queued";
}

function stageStatusLabel(behavior?: TaskStageBehavior): string | null {
  switch (behavior) {
    case "INTAKE": return "Inbox";
    default: return null;
  }
}

const styles = stylex.create({
  badge: {
    width: "fit-content",
    flexShrink: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    backgroundColor: "transparent",
    fontSize: 12,
    lineHeight: 1.2
  },
  neutral: { color: "var(--noema-text-secondary)" },
  info: { color: "var(--noema-blue-700)" },
  success: { color: "var(--noema-pine-700)" },
  warning: { color: "var(--noema-clay-600)" },
  error: { color: "var(--noema-red-700)" }
});
