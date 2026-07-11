import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import {
  AlertCircle,
  Check,
  CircleDot,
  Clock3,
  LoaderCircle,
  RotateCcw,
  UserRound,
  X
} from "lucide-react";
import type { ReactNode } from "react";
import type { TaskStatus } from "./taskTypes";

type TaskStatusMeta = {
  label: string;
  variant: "neutral" | "info" | "success" | "warning" | "error";
  icon: ReactNode;
  animated?: boolean;
};

const iconProps = { "aria-hidden": true, size: 12, strokeWidth: 2 } as const;

const statusMeta: Record<TaskStatus, TaskStatusMeta> = {
  queued: { label: "Queued", variant: "neutral", icon: <Clock3 {...iconProps} /> },
  executing: { label: "Working", variant: "info", icon: <LoaderCircle {...iconProps} />, animated: true },
  reviewing: { label: "Reviewing", variant: "warning", icon: <CircleDot {...iconProps} /> },
  revision_requested: { label: "Revising", variant: "warning", icon: <RotateCcw {...iconProps} /> },
  waiting_for_human: { label: "Needs you", variant: "warning", icon: <UserRound {...iconProps} /> },
  completed: { label: "Complete", variant: "success", icon: <Check {...iconProps} /> },
  failed: { label: "Failed", variant: "error", icon: <AlertCircle {...iconProps} /> },
  cancelled: { label: "Cancelled", variant: "neutral", icon: <X {...iconProps} /> }
};

export function taskStatusMeta(status: TaskStatus): TaskStatusMeta {
  return statusMeta[status] ?? statusMeta.queued;
}

export function taskStatusLabel(status: TaskStatus): string {
  return taskStatusMeta(status).label;
}

export function TaskStatusBadge({ status }: { status: TaskStatus }) {
  const meta = taskStatusMeta(status);
  const icon = meta.animated ? <span {...stylex.props(styles.spinner)}>{meta.icon}</span> : meta.icon;
  return (
    <Badge
      aria-label={`Task status: ${meta.label}`}
      icon={icon}
      label={meta.label}
      variant={meta.variant}
      {...stylex.props(styles.badge)}
    />
  );
}

const styles = stylex.create({
  badge: {
    width: "fit-content",
    fontSize: 11,
    lineHeight: 1.2
  },
  spinner: {
    display: "inline-flex",
    animationDuration: "900ms",
    animationIterationCount: "infinite",
    animationName: "tool-marker-spinner-rotate",
    animationTimingFunction: "linear",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none"
    }
  }
});
