import { useFragment } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ListTodo } from "lucide-react";
import { taskDetailTarget, type ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { taskStatusFromProjection } from "@/components/chatDetail/task/TaskStatusBadge";
import { TaskStatusIcon } from "@/components/chatDetail/task/TaskStatusIcon";
import type { TaskStatus } from "@/components/chatDetail/task/taskTypes";
import {
  TasksTaskReferenceSummaryFieldsFragmentDoc,
  type TasksTaskReferenceSummaryFieldsFragment
} from "@/generated/graphql";

export function TaskReferenceCard({
  taskId,
  onOpenDetail
}: {
  taskId: string;
  onOpenDetail?: (target: Extract<ChatDetailTarget, { type: "task" }>) => void;
}) {
  const result = useFragment({
    fragment: TasksTaskReferenceSummaryFieldsFragmentDoc,
    from: { __typename: "TaskSummary", taskId }
  });
  const task = result.complete ? result.data : null;
  const target = taskDetailTarget(taskId);
  const taskTarget = target?.type === "task" ? target : null;
  const opensDetail = Boolean(taskTarget && onOpenDetail);
  const title = task?.title ?? "Task unavailable";
  const chipProgressLine = taskChipProgress(task);
  const open = opensDetail && taskTarget ? () => onOpenDetail?.(taskTarget) : undefined;
  const status = taskChipStatus(task);
  const iconStatus = status === "reviewing" ? "executing" : status;
  const statusStyle = status === "done" ? styles.successIcon
    : status === "executing" || status === "reviewing" ? styles.activeIcon
    : status === "revision_requested" || status === "waiting_for_human" ? styles.attentionIcon
    : status === "failed" || !status ? styles.errorIcon
    : styles.neutralIcon;

  return (
    <Button
      data-slot="task-reference-chip"
      isDisabled={!opensDetail}
      label={`Open task: ${title}, ${chipProgressLine}`}
      onClick={open}
      size="sm"
      tooltip={opensDetail ? "Open task details" : undefined}
      variant="ghost"
      xstyle={styles.chip}
    >
      <span {...stylex.props(styles.chipContent)}>
        <ListTodo aria-hidden="true" size={13} strokeWidth={2} {...stylex.props(styles.taskIcon)} />
        <span {...stylex.props(styles.statusIcon, statusStyle)}>
          <TaskStatusIcon status={iconStatus ?? "failed"} size={14} />
        </span>
        <span {...stylex.props(styles.chipTitle)}>{title}</span>
      </span>
    </Button>
  );
}

type TaskReference = TasksTaskReferenceSummaryFieldsFragment;

function taskChipProgress(task: TaskReference | null): string {
  if (!task) return "Unavailable";
  if (task.attention?.title) return task.attention.title;
  if (task.completedAt) return "Completed";
  if (task.currentRun) return task.currentRun.activityLabel;
  return task.stage.name;
}

function taskChipStatus(task: TaskReference | null): TaskStatus | null {
  return task ? taskStatusFromProjection(task) : null;
}

const styles = stylex.create({
  chip: { width: "fit-content", maxWidth: "100%", justifyContent: "flex-start", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 999, cornerShape: "var(--corner-shape-full)", backgroundColor: "color-mix(in srgb, var(--noema-surface-card) 72%, transparent)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-primary)", boxShadow: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" } },
  chipContent: { display: "flex", maxWidth: "100%", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)" },
  taskIcon: { flexShrink: 0 },
  statusIcon: { display: "inline-flex", flexShrink: 0 },
  successIcon: { color: "var(--noema-pine-700)" },
  activeIcon: { color: "var(--noema-blue-700)" },
  attentionIcon: { color: "var(--noema-clay-600)" },
  errorIcon: { color: "var(--noema-red-700)" },
  neutralIcon: { color: "var(--noema-text-muted)" },
  chipTitle: { minWidth: 0, overflow: "hidden", fontSize: 12, fontWeight: 650, lineHeight: 1, textOverflow: "ellipsis", whiteSpace: "nowrap" },
});
