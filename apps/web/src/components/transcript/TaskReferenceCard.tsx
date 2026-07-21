import { useQuery, useSubscription } from "@apollo/client/react";
import {
  WorkTaskReferenceDocument,
  WorkTaskEventsDocument,
  type WorkTaskReferenceQuery
} from "@/generated/graphql";
import { IconButton, type IconButtonProps } from "@astryxdesign/core/IconButton";
import { Item, type ItemProps } from "@astryxdesign/core/Item";
import * as stylex from "@stylexjs/stylex";
import { ListTodo, PanelRightOpen } from "lucide-react";
import { taskDetailTarget, type ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { StageBadge } from "@/components/work/StageBadge";
import { useTaskEventCursor } from "@/components/chatDetail/task/taskEventCursor";

type ItemXStyle = ItemProps["xstyle"];
type IconButtonXStyle = IconButtonProps["xstyle"];

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
  const stageName = task?.stage.name ?? "Task";
  const progressLine = taskProgress(task);
  const description = (
    <span {...stylex.props(styles.description)}>
      <StageBadge key={task ? "ready" : "placeholder"} name={stageName} behavior="" />
      <span {...stylex.props(styles.progress)}>{progressLine}</span>
    </span>
  );
  const open = opensDetail && taskTarget ? () => onOpenDetail?.(taskTarget) : undefined;

  return (
    <Item
      align="start"
      data-slot="task-reference-item"
      data-testid="task-reference-item"
      density="balanced"
      description={description}
      descriptionLines={2}
      endContent={opensDetail ? <IconButton data-slot="task-reference-action" icon={<PanelRightOpen aria-hidden="true" size={14} strokeWidth={2} />} label="Open task details" onClick={open} size="sm" tabIndex={-1} tooltip="Open task details" variant="ghost" xstyle={iconButtonXStyle(styles.action)} /> : undefined}
      isDisabled={!opensDetail}
      label={title}
      labelLines={2}
      onClick={open}
      startContent={<span {...stylex.props(styles.iconFrame)} aria-hidden="true"><ListTodo size={18} strokeWidth={2} /></span>}
      xstyle={itemXStyle(styles.item, opensDetail && styles.actionItem, !opensDetail && styles.disabledItem)}
    />
  );
}

type TaskReference = NonNullable<WorkTaskReferenceQuery["task"]>;

function taskProgress(task: TaskReference | null): string {
  if (!task) return "Unavailable";
  if (task.attention?.summary) return task.attention.summary;
  if (task.completedAt) return "Completed";
  if (task.currentRun) return task.currentRun.activityLabel;
  return `Revision ${task.revision}`;
}

function itemXStyle(...xstyle: unknown[]): ItemXStyle {
  return xstyle as unknown as ItemXStyle;
}

function iconButtonXStyle(...xstyle: unknown[]): IconButtonXStyle {
  return xstyle as unknown as IconButtonXStyle;
}

const styles = stylex.create({
  item: { display: "inline-flex", width: "fit-content", maxWidth: "min(100%, 520px)", minWidth: "min(240px, 100%)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 8, backgroundColor: "var(--noema-surface-card)", color: "var(--noema-text-primary)", boxShadow: "0 1px 0 color-mix(in srgb, black 4%, transparent)", transition: "background-color 140ms ease, border-color 140ms ease, box-shadow 140ms ease", ":hover": { borderColor: "color-mix(in srgb, var(--noema-pine-500) 30%, var(--noema-border-subtle))", backgroundColor: "var(--noema-surface-hover)" } },
  disabledItem: { opacity: 0.72 },
  actionItem: { paddingInlineEnd: 48 },
  iconFrame: { display: "inline-flex", width: 32, height: 32, alignItems: "center", justifyContent: "center", borderRadius: 8, backgroundColor: "var(--noema-surface-sunken)", color: "var(--noema-pine-700)", flexShrink: 0 },
  action: { position: "absolute", insetInlineEnd: 8, insetBlockEnd: 8, zIndex: 1, display: "inline-flex", color: "var(--noema-text-muted)" },
  description: { display: "inline-flex", flexWrap: "wrap", alignItems: "center", gap: 6, minWidth: 0 },
  progress: { minWidth: 0, color: "var(--noema-text-muted)", fontSize: 11, overflowWrap: "anywhere" }
});

export type TaskReferenceCardTarget = NonNullable<ReturnType<typeof taskDetailTarget>>;
