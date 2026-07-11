import type { TaskRunItem, TaskRunRole } from "./taskTypes";

export type TaskRunItemSource = {
  itemId: string;
  runId?: string | null;
  sequenceIndex?: number | null;
  roundIndex?: number | null;
  kind: string;
  status?: string | null;
  correlationId?: string | null;
  parentItemId?: string | null;
  contentText?: string | null;
  payload?: unknown;
  createdAt: string;
  updatedAt?: string | null;
};

export function mapTaskRunItem(item: TaskRunItemSource, role: TaskRunRole): TaskRunItem {
  const isToolCall = item.kind === "tool_call";
  const isToolResult = item.kind === "tool_result";
  const details = isToolCall || isToolResult ? jsonText(item.payload) : null;
  return {
    id: item.itemId,
    runId: item.runId,
    sequenceIndex: item.sequenceIndex,
    roundIndex: item.roundIndex,
    kind:
      item.kind === "model_input"
        ? "input"
        : isToolCall
          ? "tool"
          : item.kind === "assistant_output"
            ? "message"
            : isToolResult
              ? "result"
              : item.kind === "artifact"
                ? "artifact"
                : "status",
    title:
      item.kind === "model_input"
        ? "Model input"
        : isToolCall
          ? `Tool call · ${item.contentText || "unnamed"}`
          : item.kind === "assistant_output"
            ? "Agent"
            : isToolResult
              ? `Tool result · ${item.contentText || "unnamed"}`
              : humanize(item.kind),
    summary: item.contentText,
    details,
    role,
    status: runItemStatus(item.status),
    correlationId: item.correlationId,
    parentItemId: item.parentItemId,
    occurredAt: item.createdAt,
    updatedAt: item.updatedAt
  };
}

export function mergeTaskRunItems(
  ...collections: readonly (readonly TaskRunItem[])[]
): TaskRunItem[] {
  const byId = new Map<string, TaskRunItem>();
  for (const collection of collections) {
    for (const item of collection) {
      byId.set(item.id, { ...byId.get(item.id), ...item });
    }
  }
  return [...byId.values()].sort((left, right) => {
    const leftSequence = left.sequenceIndex ?? Number.MAX_SAFE_INTEGER;
    const rightSequence = right.sequenceIndex ?? Number.MAX_SAFE_INTEGER;
    if (leftSequence !== rightSequence) {
      return leftSequence - rightSequence;
    }
    return (left.occurredAt ?? "").localeCompare(right.occurredAt ?? "");
  });
}

function jsonText(value: unknown): string | null {
  if (value === null || value === undefined) {
    return null;
  }
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function runItemStatus(value: string | null | undefined): TaskRunItem["status"] {
  switch (value) {
    case "queued":
    case "running":
    case "completed":
    case "failed":
    case "cancelled":
    case "skipped":
      return value;
    default:
      return null;
  }
}

function humanize(value: string): string {
  return value.replaceAll("_", " ").replace(/^./, (character) => character.toUpperCase());
}
