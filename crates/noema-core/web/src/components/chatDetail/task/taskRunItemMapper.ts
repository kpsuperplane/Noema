import type { TaskRunItem, TaskRunRole } from "./taskTypes";
import type { TurnActivityStatus } from "@/generated/graphql";
import type { TranscriptEntry } from "@/shared/types";

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
  const isModelInput = item.kind === "model_input";
  const details = isModelInput ? item.contentText : isToolCall || isToolResult ? jsonText(item.payload) : null;
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
    summary: isModelInput ? modelInputSummary(item.contentText) : item.contentText,
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

export function taskRunItemsToTranscriptEntries(items: readonly TaskRunItem[]): TranscriptEntry[] {
  return items.map(taskRunItemToTranscriptEntry);
}

function taskRunItemToTranscriptEntry(item: TaskRunItem): TranscriptEntry {
  const turnId = `${item.runId ?? "task-run"}:${item.roundIndex ?? "setup"}`;
  const base = { id: item.id, source: "replay" as const, turnId };

  if (item.kind === "tool" || item.kind === "result") {
    const isCall = item.kind === "tool";
    const correlationId = item.correlationId ?? item.id;
    const toolName = taskToolName(item);
    const action = {
      ...(isCall ? { id: correlationId } : { call_id: correlationId }),
      name: toolName,
      payload: parseDetails(item.details),
      ...(isCall ? {} : { success: item.status !== "failed" })
    };
    return {
      ...base,
      type: "activity",
      item: {
        kind: "activity",
        id: item.id,
        activity_kind: isCall ? "tool_call" : "tool_result",
        status: taskActivityStatus(item.status),
        title: item.title,
        summary: item.summary,
        metadata: {
          action,
          display: {
            name: toolName,
            ...(item.summary ? { result: item.summary } : {})
          }
        }
      }
    };
  }

  if (item.kind === "status") {
    return {
      ...base,
      type: "activity",
      item: {
        kind: "activity",
        id: item.id,
        activity_kind: "task_status",
        status: taskActivityStatus(item.status),
        title: item.title,
        summary: item.summary,
        metadata: null
      }
    };
  }

  if (item.kind === "input") {
    return {
      ...base,
      type: "activity",
      item: {
        kind: "activity",
        id: item.id,
        activity_kind: "model_input",
        status: taskActivityStatus(item.status),
        title: item.title,
        summary: item.summary,
        metadata: item.details ? { detail: item.details } : null
      }
    };
  }

  return {
    ...base,
    type: "assistant",
    text: item.summary?.trim() || item.details?.trim() || item.title
  };
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

function modelInputSummary(value: string | null | undefined): string {
  const text = value?.trim();
  if (!text) {
    return "Provider context";
  }

  try {
    const parsed: unknown = JSON.parse(text);
    if (Array.isArray(parsed)) {
      const count = parsed.length;
      return count === 0
        ? "Context refreshed"
        : `Context refreshed · ${count} recorded ${count === 1 ? "result" : "results"}`;
    }
    return "Provider context refreshed";
  } catch {
    const firstLine = text.split("\n").map((line) => line.trim()).find(Boolean);
    return firstLine && firstLine.length <= 120 ? firstLine : "Initial task context";
  }
}

function parseDetails(value: string | null | undefined): unknown {
  if (!value?.trim()) {
    return undefined;
  }
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

function taskToolName(item: TaskRunItem): string {
  return item.title.replace(/^Tool (?:call|result) ·\s*/i, "").trim() || "Tool activity";
}

function taskActivityStatus(status: TaskRunItem["status"]): TurnActivityStatus {
  if (status === "running" || status === "queued") {
    return "STARTED";
  }
  if (status === "failed") {
    return "FAILED";
  }
  return "COMPLETED";
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
