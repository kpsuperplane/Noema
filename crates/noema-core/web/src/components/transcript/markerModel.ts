import type { MemoryCardData } from "@/memory/cards";
import type { TurnTranscriptItem } from "@/shared/types";
import type { ToolMarkerGroup } from "./renderModel";
type MemoryDetailRowData = { label: string; value: string };
type MemoryClaimOutcome = {
  claimId: string;
  outcome: "created" | "reinforced";
  factPreview?: string;
  sensitivity?: string;
};

type MemoryDetailItem = {
  title: string;
  rows: MemoryDetailRowData[];
};

function memoryClaimOutcomes(metadata: unknown): MemoryClaimOutcome[] {
  if (!metadata || typeof metadata !== "object") {
    return [];
  }
  const outcomes = (metadata as { claim_outcomes?: unknown }).claim_outcomes;
  if (!Array.isArray(outcomes)) {
    return [];
  }
  return outcomes.flatMap((outcome): MemoryClaimOutcome[] => {
    if (!outcome || typeof outcome !== "object") {
      return [];
    }
    const record = outcome as Record<string, unknown>;
    const claimId = typeof record.claim_id === "string" ? record.claim_id : "";
    const rawOutcome = record.outcome;
    if (!claimId || (rawOutcome !== "created" && rawOutcome !== "reinforced")) {
      return [];
    }
    return [
      {
        claimId,
        outcome: rawOutcome,
        factPreview: typeof record.fact_preview === "string" ? record.fact_preview : undefined,
        sensitivity: typeof record.sensitivity === "string" ? record.sensitivity : undefined
      }
    ];
  });
}

export function metadataCount(metadata: unknown, key: string): number {
  if (!metadata || typeof metadata !== "object") {
    return 0;
  }
  const value = (metadata as Record<string, unknown>)[key];
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0;
}

export function memoryMarkerLabel(extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>): string {
  if (!extraction) {
    return "Memory updated";
  }
  if (extraction.status === "STARTED") {
    return "Memory proposed";
  }

  const outcomes = memoryClaimOutcomes(extraction.metadata);
  const failedCount = metadataCount(extraction.metadata, "failed_proposal_count");
  if (outcomes.length === 0) {
    return extraction.status === "FAILED" ? "Memory update failed" : "Memory updated";
  }
  if (outcomes.length === 1 && failedCount === 0) {
    const outcome = outcomes[0];
    const verb = outcome.outcome === "reinforced" ? "Memory updated" : "Memory saved";
    return outcome.factPreview ? `${verb}: ${outcome.factPreview}` : verb;
  }

  const createdCount = metadataCount(extraction.metadata, "created_claim_count");
  const reinforcedCount = metadataCount(extraction.metadata, "reinforced_claim_count");
  const savedCount = createdCount + reinforcedCount || outcomes.length;
  const noun = savedCount === 1 ? "memory" : "memories";
  const prefix = createdCount > 0 ? "Memory saved" : "Memory updated";
  const failureSuffix = failedCount > 0 ? `; ${failedCount} failed` : "";
  return `${prefix}: ${savedCount} ${noun}${failureSuffix}`;
}

export function memoryCardsFromClaimOutcomes(
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>
): MemoryCardData[] {
  return memoryClaimOutcomes(extraction?.metadata).map((outcome) => ({
    id: outcome.claimId,
    title: outcome.factPreview ?? outcome.claimId,
    content: outcome.factPreview ?? outcome.claimId,
    status: outcome.outcome,
    sensitivity: outcome.sensitivity
  }));
}

export function memoryDetailItems(memories: MemoryCardData[]): MemoryDetailItem[] {
  return memories.map((memory) => {
    const rows: MemoryDetailRowData[] = [{ label: "Memory", value: memory.content }];
    if (memory.memoryType) {
      rows.push({ label: "Type", value: memory.memoryType });
    }
    if (memory.sensitivity) {
      rows.push({ label: "Sensitivity", value: memory.sensitivity });
    }
    if (memory.status) {
      rows.push({ label: "Status", value: memory.status });
    }
    if (typeof memory.confidence === "number") {
      rows.push({ label: "Confidence", value: `${Math.round(memory.confidence * 100)}%` });
    }
    if (memory.evidenceExcerpt) {
      rows.push({ label: "Evidence", value: memory.evidenceExcerpt });
    }
    if (memory.id) {
      rows.push({ label: "Memory ID", value: memory.id });
    }
    return { title: memory.title, rows };
  });
}

export function toolMarkerTone(marker: ToolMarkerGroup): "default" | "error" {
  return marker.result?.item.status === "FAILED" ? "error" : "default";
}

export function toolMarkerPending(marker: ToolMarkerGroup): boolean {
  return marker.call?.item.status === "STARTED" && !marker.result;
}

export function toolMarkerLabel(marker: ToolMarkerGroup): string {
  const toolName = toolNameFromMetadata(marker.call?.item.metadata) ?? toolNameFromMetadata(marker.result?.item.metadata);
  if (toolName) {
    if (marker.call && !marker.result && marker.call.item.status === "STARTED") {
      return `Using ${toolName}`;
    }
    return `Used ${toolName}`;
  }
  return marker.call?.item.title ?? marker.result?.item.title ?? "Tool activity";
}

export function toolMarkerName(marker: ToolMarkerGroup): string {
  return (
    toolNameFromMetadata(marker.call?.item.metadata) ??
    toolNameFromMetadata(marker.result?.item.metadata) ??
    marker.call?.item.title ??
    marker.result?.item.title ??
    "Tool activity"
  );
}

export function formatToolDetail(fallback: string, metadata: unknown): string {
  const preview = safeToolMetadataPreview(metadata);
  if (!preview.length) {
    return fallback;
  }
  return [fallback, ...preview].join("\n");
}

function safeToolMetadataPreview(metadata: unknown): string[] {
  if (!isRecord(metadata)) {
    return [];
  }

  const rows: string[] = [];
  const provider = stringValue(metadata.provider);
  if (provider) {
    rows.push(`Provider: ${provider}`);
  }

  const action = isRecord(metadata.action) ? metadata.action : null;
  const actionName = stringValue(action?.name);
  if (actionName) {
    rows.push(`Tool: ${actionName}`);
  }
  const actionId = stringValue(action?.id) ?? stringValue(action?.call_id);
  if (actionId) {
    rows.push(`Call ID: ${actionId}`);
  }
  const success = action?.success;
  if (typeof success === "boolean") {
    rows.push(`Success: ${success ? "yes" : "no"}`);
  }

  const hasHiddenMetadata =
    Object.keys(metadata).some((key) => !["provider", "action"].includes(key)) ||
    (action ? Object.keys(action).some((key) => !["id", "call_id", "name", "success"].includes(key)) : false);
  if (hasHiddenMetadata || rows.length === 0) {
    rows.push("Additional metadata hidden from normal transcript view.");
  }

  return rows;
}

function toolNameFromMetadata(metadata: unknown): string | null {
  if (!isRecord(metadata)) {
    return null;
  }

  const action = metadata.action;
  if (isRecord(action) && typeof action.name === "string" && action.name.trim()) {
    return action.name;
  }
  if (typeof metadata.name === "string" && metadata.name.trim()) {
    return metadata.name;
  }
  if (typeof metadata.tool_name === "string" && metadata.tool_name.trim()) {
    return metadata.tool_name;
  }
  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}
