import { useQuery, useSubscription } from "@apollo/client/react";
import {
  WorkTaskReferenceDocument,
  WorkTaskEventsDocument,
  type WorkTaskReferenceQuery
} from "@/generated/graphql";
import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ListTodo } from "lucide-react";
import { taskDetailTarget, type ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { TaskStatusIcon } from "@/components/chatDetail/task/TaskStatusIcon";
import type { TaskStatus } from "@/components/chatDetail/task/taskTypes";
import { useTaskEventCursor } from "@/components/chatDetail/task/taskEventCursor";

type ButtonXStyle = ButtonProps["xstyle"];

export function TaskReferenceCard({
  taskId,
  onOpenDetail
}: {
  taskId: string;
  onOpenDetail?: (target: Extract<ChatDetailTarget, { type: "task" }>) => void;
}) {
  const result = useQuery(WorkTaskReferenceDocument, {
    variables: { taskId },
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true
  });
  const [cursor, recordCursor] = useTaskEventCursor(taskId);
  const queriedTask = result.data?.task ?? null;
  const task = queriedTask?.taskId === taskId ? queriedTask : null;
  useSubscription(WorkTaskEventsDocument, {
    variables: { taskId, after: cursor },
    skip: !task,
    onData: ({ data }) => {
      const event = data.data?.taskEvents;
      if (!event) return;
      recordCursor(event.cursor);
      void result.refetch();
    }
  });

  const target = taskDetailTarget(taskId);
  const taskTarget = target?.type === "task" ? target : null;
  const opensDetail = Boolean(taskTarget && onOpenDetail);
  const title = task?.title ?? (result.loading ? "Loading task…" : "Task unavailable");
  const chipProgressLine = taskChipProgress(task);
  const open = opensDetail && taskTarget ? () => onOpenDetail?.(taskTarget) : undefined;
  const status = taskChipStatus(task);
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
      style={{ border: "1px solid var(--noema-border-default)" }}
      tooltip={opensDetail ? "Open task details" : undefined}
      variant="ghost"
      xstyle={buttonXStyle(styles.chip)}
    >
      <span {...stylex.props(styles.chipContent)}>
        <ListTodo aria-hidden="true" size={13} strokeWidth={2} {...stylex.props(styles.taskIcon)} />
        <span {...stylex.props(styles.statusIcon, statusStyle)}>
          <TaskStatusIcon status={status ?? "failed"} size={14} />
        </span>
        <span {...stylex.props(styles.chipTitle)}>{title}</span>
      </span>
    </Button>
  );
}

type TaskReference = NonNullable<WorkTaskReferenceQuery["task"]>;

function taskChipProgress(task: TaskReference | null): string {
  if (!task) return "Unavailable";
  if (task.attention?.title) return task.attention.title;
  if (task.completedAt) return "Completed";
  if (task.currentRun) return task.currentRun.activityLabel;
  return task.stage.name;
}

function taskChipStatus(task: TaskReference | null): TaskStatus | null {
  if (!task) return null;
  if (task.completedAt || task.stage.behavior === "TERMINAL_SUCCESS") return "done";
  if (task.attention || task.stage.behavior === "HUMAN_GATE") return "waiting_for_human";
  if (task.stage.behavior === "TERMINAL_CANCELLED") return "cancelled";
  if (task.stage.behavior === "ACTIVE") return task.currentRun?.kind === "REVIEWER" ? "reviewing" : "executing";
  return "queued";
}

function buttonXStyle(...xstyle: unknown[]): ButtonXStyle {
  return xstyle as unknown as ButtonXStyle;
}

const styles = stylex.create({
  chip: { width: "fit-content", maxWidth: "100%", justifyContent: "flex-start", borderRadius: 999, backgroundColor: "color-mix(in srgb, var(--noema-surface-card) 72%, transparent)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-primary)", boxShadow: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" } },
  chipContent: { display: "flex", maxWidth: "100%", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)" },
  taskIcon: { flexShrink: 0 },
  statusIcon: { display: "inline-flex", flexShrink: 0 },
  successIcon: { color: "var(--noema-pine-700)" },
  activeIcon: { color: "var(--noema-blue-700)" },
  attentionIcon: { color: "var(--noema-clay-600)" },
  errorIcon: { color: "var(--noema-red-700)" },
  neutralIcon: { color: "var(--noema-text-muted)" },
  chipTitle: { minWidth: 0, overflow: "hidden", fontSize: 11, fontWeight: 650, lineHeight: 1, textOverflow: "ellipsis", whiteSpace: "nowrap" },
});
