import { useQuery, useSubscription } from "@apollo/client/react";
import * as React from "react";
import { IconButton, type IconButtonProps } from "@astryxdesign/core/IconButton";
import { Item, type ItemProps } from "@astryxdesign/core/Item";
import * as stylex from "@stylexjs/stylex";
import { ListTodo, PanelRightOpen } from "lucide-react";
import { taskDetailTarget, type ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import {
  TaskEventsDocument,
  TaskReferenceStatusDocument
} from "@/generated/graphql";
import { TaskStatusBadge } from "@/components/chatDetail/task/TaskStatusBadge";
import type { TaskStatus } from "@/components/chatDetail/task/taskTypes";

type ItemXStyle = ItemProps["xstyle"];
type IconButtonXStyle = IconButtonProps["xstyle"];

export function TaskReferenceCard({
  taskId,
  title,
  status,
  progress,
  revision,
  onOpenDetail
}: {
  taskId: string;
  title: string;
  status: TaskStatus;
  progress?: string | null;
  revision?: number | null;
  onOpenDetail?: (target: Extract<ChatDetailTarget, { type: "task" }>) => void;
}) {
  const target = taskDetailTarget(taskId);
  const taskTarget = target?.type === "task" ? target : null;
  const opensDetail = Boolean(taskTarget && onOpenDetail);
  const { data } = useQuery(TaskReferenceStatusDocument, {
    fetchPolicy: "cache-and-network",
    variables: { taskId },
    skip: !taskTarget
  });
  const [liveStatus, setLiveStatus] = React.useState<TaskStatus | null>(null);
  useSubscription(TaskEventsDocument, {
    variables: { taskId, after: null },
    skip: !taskTarget,
    onData: ({ data: result }) => {
      const event = result.data?.taskEvents;
      if (event?.kind === "TASK_UPDATED") {
        const nextStatus = taskStatusFromGraphql(event.status);
        if (nextStatus) {
          setLiveStatus(nextStatus);
        }
      }
    }
  });
  const currentStatus = liveStatus ?? taskStatusFromGraphql(data?.task?.status) ?? status;
  const currentRevision = data?.task?.revisionIndex ?? revision;
  const progressLine = progress || (currentRevision ? `Revision ${currentRevision}` : "Background task");
  const description = (
    <span {...stylex.props(styles.description)}>
      <TaskStatusBadge status={currentStatus} />
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
      endContent={
        opensDetail ? (
          <IconButton
            data-slot="task-reference-action"
            icon={<PanelRightOpen aria-hidden="true" size={14} strokeWidth={2} />}
            label="Open task details"
            onClick={open}
            size="sm"
            tabIndex={-1}
            tooltip="Open task details"
            variant="ghost"
            xstyle={iconButtonXStyle(styles.action)}
          />
        ) : undefined
      }
      isDisabled={!opensDetail}
      label={title}
      labelLines={2}
      onClick={open}
      startContent={
        <span {...stylex.props(styles.iconFrame)} aria-hidden="true">
          <ListTodo size={18} strokeWidth={2} />
        </span>
      }
      xstyle={itemXStyle(styles.item, !opensDetail && styles.disabledItem)}
    />
  );
}

function taskStatusFromGraphql(value?: string | null): TaskStatus | null {
  switch (value?.toLowerCase()) {
    case "queued":
    case "executing":
    case "reviewing":
    case "revision_requested":
    case "waiting_for_human":
    case "completed":
    case "failed":
    case "cancelled":
      return value.toLowerCase() as TaskStatus;
    default:
      return null;
  }
}

function itemXStyle(...xstyle: unknown[]): ItemXStyle {
  return xstyle as unknown as ItemXStyle;
}

function iconButtonXStyle(...xstyle: unknown[]): IconButtonXStyle {
  return xstyle as unknown as IconButtonXStyle;
}

const styles = stylex.create({
  item: {
    display: "inline-flex",
    width: "fit-content",
    maxWidth: "min(100%, 520px)",
    minWidth: "min(240px, 100%)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-card)",
    color: "var(--noema-text-primary)",
    boxShadow: "0 1px 0 color-mix(in srgb, black 4%, transparent)",
    transition: "background-color 140ms ease, border-color 140ms ease, box-shadow 140ms ease",
    ":hover": {
      borderColor: "color-mix(in srgb, var(--noema-pine-500) 30%, var(--noema-border-subtle))",
      backgroundColor: "var(--noema-surface-hover)"
    }
  },
  disabledItem: { opacity: 0.72 },
  iconFrame: {
    display: "inline-flex",
    width: 32,
    height: 32,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-pine-700)",
    flexShrink: 0
  },
  action: {
    position: "absolute",
    insetInlineEnd: 8,
    insetBlockEnd: 8,
    zIndex: 1,
    display: "inline-flex",
    color: "var(--noema-text-muted)"
  },
  description: {
    display: "inline-flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 6,
    minWidth: 0
  },
  progress: {
    minWidth: 0,
    color: "var(--noema-text-muted)",
    fontSize: 11,
    overflowWrap: "anywhere"
  }
});

export type TaskReferenceCardTarget = NonNullable<ReturnType<typeof taskDetailTarget>>;
