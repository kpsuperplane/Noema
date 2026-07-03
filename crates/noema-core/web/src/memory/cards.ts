import type { TurnTranscriptItem } from "@/shared/types";

type JsonRecord = Record<string, unknown>;

export type MemoryCardData = {
  id?: string;
  title: string;
  content: string;
  memoryType?: string;
  sensitivity?: string;
  status?: string;
  confidence?: number;
  evidenceExcerpt?: string;
};

export function memoryCardsFromStructuredItem(
  item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>
): MemoryCardData[] | null {
  const payload = asRecord(item.payload);
  if (!payload) {
    return null;
  }

  const createdMemoryIds = asStringArray(payload.created_memory_ids);
  if (item.schema === "memory_proposals") {
    const proposals = asArray(payload.proposals)
      .map((entry, index) => memoryCardFromProposal(entry, createdMemoryIds[index]))
      .filter((memory): memory is MemoryCardData => memory !== null);
    return proposals.length > 0 ? proposals : null;
  }

  if (item.schema === "memory_cards") {
    const memories = asArray(payload.memories)
      .map((entry, index) => memoryCardFromMemory(entry, createdMemoryIds[index]))
      .filter((memory): memory is MemoryCardData => memory !== null);
    return memories.length > 0 ? memories : null;
  }

  return null;
}

function memoryCardFromProposal(value: unknown, id?: string): MemoryCardData | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const proposal = asRecord(record.proposal) ?? record;
  const content = asString(proposal.content);
  if (!content) {
    return null;
  }

  return {
    id,
    title: asString(proposal.title) ?? titleFromContent(content),
    content,
    memoryType: asString(proposal.memory_type),
    sensitivity: asString(proposal.sensitivity),
    status: asString(record.status),
    confidence: asNumber(proposal.confidence),
    evidenceExcerpt: asString(proposal.evidence_excerpt)
  };
}

function memoryCardFromMemory(value: unknown, id?: string): MemoryCardData | null {
  const record = asRecord(value);
  if (!record) {
    return null;
  }

  const content = asString(record.content);
  if (!content) {
    return null;
  }

  return {
    id,
    title: asString(record.title) ?? titleFromContent(content),
    content,
    memoryType: asString(record.memory_type),
    sensitivity: asString(record.sensitivity),
    status: asString(record.status)
  };
}

function asRecord(value: unknown): JsonRecord | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as JsonRecord;
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function asStringArray(value: unknown): string[] {
  return asArray(value).filter((item): item is string => typeof item === "string");
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function asNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function titleFromContent(content: string) {
  const trimmed = content.trim();
  if (trimmed.length <= 64) {
    return trimmed || "Memory";
  }
  return `${trimmed.slice(0, 61)}...`;
}
