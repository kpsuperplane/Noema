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
    sourceKind: item.kind,
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

export function taskRunItemsToTranscriptEntries(items: readonly TaskRunItem[]): TranscriptEntry[] {
  const entries: TranscriptEntry[] = [];
  for (const item of items) {
    const entry = taskRunItemToTranscriptEntry(item);
    if (!entry) {
      continue;
    }
    const previous = entries.at(-1);
    if (entry.type === "assistant" && previous?.type === "assistant" && entry.turnId === previous.turnId) {
      entries[entries.length - 1] = { ...previous, text: `${previous.text}${entry.text}` };
    } else {
      entries.push(entry);
    }
  }
  return entries;
}

function taskRunItemToTranscriptEntry(item: TaskRunItem): TranscriptEntry | null {
  const turnId = `${item.runId ?? "task-run"}:${item.roundIndex ?? "setup"}:${item.responseIndex ?? "default"}`;
  const base = { id: item.id, source: "replay" as const, turnId };

  if (item.kind === "tool" || item.kind === "result") {
    const isCall = item.kind === "tool";
    const persisted = recordValue(item.payload);
    const correlationId = item.correlationId ?? persistedCorrelationId(persisted) ?? item.id;
    const toolName = taskToolName(item);
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
        : { success: typeof success === "boolean" ? success : item.status !== "failed" })
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
            name: toolName
          }
        }
      }
    };
  }

  if (item.kind === "status") {
    if (item.sourceKind === "context_checkpoint") {
      return checkpointSystemEntry(base, item, "Context compacted");
    }
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
    const rawInput = item.details ?? item.summary;
    if (isBoundedEvidenceInput(rawInput)) {
      return checkpointSystemEntry(base, item, "Evidence checkpoint");
    }
    if (recordValue(item.payload)?.context_mode === "cumulative_replay") {
      return checkpointSystemEntry(base, item, "Model context");
    }
    const text = visibleModelInput(rawInput);
    if (!text) {
      return null;
    }
    return {
      ...base,
      type: "input",
      label: "Model input",
      text
    };
  }

  return {
    ...base,
    type: "assistant",
    text: item.summary ?? item.details ?? item.title
  };
}

function checkpointSystemEntry(
  base: { id: string; source: "replay"; turnId: string },
  item: TaskRunItem,
  label: string
): TranscriptEntry {
  return {
    ...base,
    type: "input",
    label,
    text: persistedItemDetail(item)
  };
}

function persistedItemDetail(item: TaskRunItem): string {
  const contentText = (item.details ?? item.summary)?.trim();
  const payloadText = jsonText(item.payload);
  return [contentText, payloadText]
    .filter((section): section is string => Boolean(section))
    .join("\n\n");
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

function visibleModelInput(value: string | null | undefined): string | null {
  const text = value?.trim();
  if (!text) {
    return null;
  }

  try {
    const parsed: unknown = JSON.parse(text);
    if (Array.isArray(parsed)) {
      const visible = parsed.filter((item) => !isToolContextRecord(item));
      return visible.length > 0 ? JSON.stringify(visible, null, 2) : null;
    }
    return isToolContextRecord(parsed) ? null : JSON.stringify(parsed, null, 2);
  } catch {
    const visibleLines = text
      .split("\n")
      .filter((line) => !isSerializedToolContext(line));
    const visible = visibleLines.join("\n").trim();
    return visible || null;
  }
}

function isBoundedEvidenceInput(value: string | null | undefined): boolean {
  const parsed = parseJson(value);
  return recordValue(parsed)?.type === "NOEMA_BOUNDED_TASK_EVIDENCE";
}

function isSerializedToolContext(value: string): boolean {
  const text = value.trim();
  if (!text) {
    return false;
  }
  try {
    return isToolContextRecord(JSON.parse(text));
  } catch {
    return false;
  }
}

function isToolContextRecord(value: unknown): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return false;
  }
  const record = value as Record<string, unknown>;
  return record.type === "tool_call" ||
    record.type === "tool_result" ||
    record.type === "NOEMA_LOCAL_TOOL_RESULT" ||
    (typeof record.call_id === "string" && typeof record.name === "string");
}

function parseJson(value: string | null | undefined): unknown {
  if (!value?.trim()) {
    return undefined;
  }
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
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
  const persistedName = recordValue(item.payload)?.name;
  if (typeof persistedName === "string" && persistedName.trim()) {
    return persistedName;
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
