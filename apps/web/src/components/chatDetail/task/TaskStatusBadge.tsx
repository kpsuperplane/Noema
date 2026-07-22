import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { TaskStatusIcon } from "./TaskStatusIcon";
import type { TaskStageBehavior, TaskStatus } from "./taskTypes";

type TaskStatusMeta = {
  label: string;
  variant: "neutral" | "info" | "success" | "warning" | "error";
};

const statusMeta: Record<TaskStatus, TaskStatusMeta> = {
  queued: { label: "Queued", variant: "neutral" },
  executing: { label: "Working", variant: "info" },
  reviewing: { label: "Reviewing", variant: "warning" },
  revision_requested: { label: "Revising", variant: "warning" },
  waiting_for_human: { label: "Needs you", variant: "warning" },
  done: { label: "Done", variant: "success" },
  failed: { label: "Failed", variant: "error" },
  cancelled: { label: "Cancelled", variant: "neutral" }
};

export function taskStatusMeta(status: TaskStatus): TaskStatusMeta {
  return statusMeta[status] ?? statusMeta.queued;
}

export function taskStatusLabel(status: TaskStatus): string {
  return taskStatusMeta(status).label;
}

export function TaskStatusBadge({
  status,
  stageBehavior
}: {
  status: TaskStatus;
  stageBehavior?: TaskStageBehavior;
}) {
  const meta = taskStatusMeta(status);
  const label = stageStatusLabel(stageBehavior) ?? meta.label;
  return (
    <Badge
      aria-label={`Task status: ${label}`}
      icon={<TaskStatusIcon status={status} />}
      label={label}
      variant={meta.variant}
      {...stylex.props(styles.badge)}
    />
  );
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
    fontSize: 11,
    lineHeight: 1.2
  }
});
