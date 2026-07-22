import { useQuery, useSubscription } from "@apollo/client/react";
import {
  WorkTaskReferenceDocument,
  WorkTaskEventsDocument,
  type WorkTaskReferenceQuery
} from "@/generated/graphql";
import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { CircleCheck, ListTodo } from "lucide-react";
import { taskDetailTarget, type ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
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
  const completed = Boolean(task?.completedAt);

  return (
    <Button
      data-slot="task-reference-chip"
      isDisabled={!opensDetail}
      label={`Open task: ${title}, ${chipProgressLine}`}
      icon={<ListTodo aria-hidden="true" size={13} strokeWidth={2} />}
      onClick={open}
      size="sm"
      tooltip={opensDetail ? "Open task details" : undefined}
      variant="ghost"
      xstyle={buttonXStyle(styles.chip)}
    >
      <span {...stylex.props(styles.chipContent)}>
        {completed ? (
          <CircleCheck aria-hidden="true" size={14} strokeWidth={2} {...stylex.props(styles.completedIcon)} />
        ) : null}
        <span {...stylex.props(styles.chipTitle)}>{title}</span>
        {completed ? null : (
          <span aria-hidden="true" {...stylex.props(styles.chipProgress)}>
            {chipProgressLine}
          </span>
        )}
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

function buttonXStyle(...xstyle: unknown[]): ButtonXStyle {
  return xstyle as unknown as ButtonXStyle;
}

const styles = stylex.create({
  chip: { maxWidth: "100%", justifyContent: "flex-start", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 999, backgroundColor: "color-mix(in srgb, var(--noema-surface-card) 72%, transparent)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-primary)", boxShadow: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" } },
  chipContent: { display: "inline-flex", maxWidth: "100%", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)" },
  completedIcon: { flexShrink: 0, color: "var(--noema-pine-700)", transform: "translateY(1px)" },
  chipTitle: { minWidth: 0, overflow: "hidden", fontSize: 11, fontWeight: 650, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  chipProgress: { flexShrink: 0, color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 500 }
});
