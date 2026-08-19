import type { TaskRunItem, TaskRunRole } from "./taskTypes";
import type { TurnActivityStatus } from "@/generated/graphql";
import type { TranscriptEntry } from "@/shared/types";

const NO_ARRIVAL_ITEM_IDS: ReadonlySet<string> = new Set();

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
  const sourceKind = item.kind.toLowerCase();
  const isToolCall = sourceKind === "tool_call";
  const isToolResult = sourceKind === "tool_result";
  const details = isToolCall || isToolResult ? jsonText(item.payload) : null;
  return {
    id: item.itemId,
    runId: item.runId,
    sequenceIndex: item.sequenceIndex,
    roundIndex: item.roundIndex,
    sourceKind,
    kind:
      isToolCall
          ? "tool"
          : sourceKind === "assistant_output"
            ? "message"
            : isToolResult
              ? "result"
              : sourceKind === "artifact_reference"
                ? "artifact"
                : "status",
    title:
      isToolCall
          ? `Tool call · ${item.contentText || "unnamed"}`
          : sourceKind === "assistant_output"
            ? "Agent"
            : isToolResult
              ? `Tool result · ${item.contentText || "unnamed"}`
              : humanize(sourceKind),
    summary: item.contentText,
    details,
    payload: item.payload,
    role,
    status: runItemStatus(item.status),
    correlationId: item.correlationId,
    parentItemId: item.parentItemId,
    responseIndex: responseIndex(item.payload),
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

export function taskRunItemsToTranscriptEntries(
  items: readonly TaskRunItem[],
  arrivalItemIds: ReadonlySet<string> = NO_ARRIVAL_ITEM_IDS
): TranscriptEntry[] {
  const entries: TranscriptEntry[] = [];
  for (const [index, item] of items.entries()) {
    if (isRedundantLifecycleNotice(items, index)) {
      continue;
    }
    const entry = taskRunItemToTranscriptEntry(item, arrivalItemIds.has(item.id));
    if (!entry) {
      continue;
    }
    const previous = entries.at(-1);
    if (entry.type === "assistant" && previous?.type === "assistant" && entry.turnId === previous.turnId) {
      entries[entries.length - 1] = {
        ...previous,
        source: previous.source === "replay" && entry.source === "replay" ? "replay" : undefined,
        text: `${previous.text}${entry.text}`
      };
    } else {
      entries.push(entry);
    }
  }
  return entries;
}

function isRedundantLifecycleNotice(items: readonly TaskRunItem[], index: number): boolean {
  const item = items[index];
  if (item?.sourceKind !== "progress_notice") {
    return false;
  }
  if (recordValue(item.payload)?.phase === "provider_response") {
    return true;
  }

  const next = items[index + 1];
  return next?.kind === "result" && next.runId === item.runId;
}

function taskRunItemToTranscriptEntry(item: TaskRunItem, animateArrival: boolean): TranscriptEntry | null {
  if (item.sourceKind === "model_input" || item.sourceKind === "context_checkpoint") {
    return null;
  }
  const turnId = `${item.runId ?? "task-run"}:${item.roundIndex ?? "setup"}:${item.responseIndex ?? "default"}`;
  const base = {
    id: item.id,
    ...(animateArrival ? {} : { source: "replay" as const }),
    turnId,
    debugScope: item.runId
      ? { kind: "TASK_RUN" as const, scopeId: item.runId }
      : undefined
  };

  if (item.kind === "tool" || item.kind === "result") {
    const isCall = item.kind === "tool";
    const persisted = recordValue(item.payload);
    const acpCallId = persisted?.toolCallId;
    const correlationId =
      (typeof acpCallId === "string" && acpCallId.trim() ? acpCallId : null) ??
      item.correlationId ??
      persistedCorrelationId(persisted) ??
      item.id;
    const toolName = taskToolName(item);
    const display = recordValue(persisted?.display);
    const marker = recordValue(display?.marker);
    if (marker?.visibility === "fold") {
      return null;
    }
    const argumentsPayload = persisted?.arguments;
    const resultPayload = isCall ? undefined : persistedResultPayload(persisted);
    const success = persisted?.success;
    const action = {
      ...(isCall ? { id: correlationId } : { call_id: correlationId }),
      name: toolName,
      correlation_id: correlationId,
      detail_mode: "complete",
      ...(argumentsPayload === undefined ? {} : { arguments: argumentsPayload }),
      payload: isCall ? argumentsPayload : resultPayload,
      ...(isCall
        ? {}
        : { success: typeof success === "boolean" ? success : item.status === "completed" })
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
            status: item.status,
            ...(display ?? {})
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

  return {
    ...base,
    type: "assistant",
    responseIndex: item.responseIndex ?? undefined,
    debugRoundIndex: item.roundIndex ?? undefined,
    text: item.summary ?? item.details ?? item.title
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

function responseIndex(value: unknown): number | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  const candidate = (value as { response_index?: unknown }).response_index;
  return typeof candidate === "number" && Number.isInteger(candidate) ? candidate : null;
}

function recordValue(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function persistedCorrelationId(payload: Record<string, unknown> | null): string | null {
  if (!payload) {
    return null;
  }
  for (const key of ["provider_call_id", "call_id", "id"]) {
    const value = payload[key];
    if (typeof value === "string" && value.trim()) {
      return value;
    }
  }
  return null;
}

function persistedResultPayload(payload: Record<string, unknown> | null): unknown {
  if (!payload) {
    return undefined;
  }
  return "payload" in payload ? payload.payload : payload;
}

function taskToolName(item: TaskRunItem): string {
  const persisted = recordValue(item.payload);
  for (const candidate of [persisted?.name, persisted?.title]) {
    if (typeof candidate === "string" && candidate.trim()) {
      return candidate;
    }
  }
  return item.summary?.trim() || "Tool activity";
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
  const normalized = value?.toLowerCase();
  switch (normalized) {
    case "pending":
      return "queued";
    case "queued":
    case "running":
    case "completed":
    case "failed":
    case "cancelled":
    case "skipped":
      return normalized;
    default:
      return null;
  }
}

function humanize(value: string): string {
  return value.replaceAll("_", " ").replace(/^./, (character) => character.toUpperCase());
}
