import type { MemoryGraphQuery } from "@/generated/graphql";

type MemoryGraph = MemoryGraphQuery["memoryGraph"];
type MemoryGraphDocument = MemoryGraph["documents"][number];
type MemoryGraphEntry = MemoryGraphDocument["memoryEntries"][number];

export type MemoryArticleEntry = {
  id: string;
  text: string;
  createdAt: string | null;
  updatedAt: string | null;
  sourceTitle: string;
  sourceObservation: string | null;
  scoreLabel: string | null;
};

export type MemoryArticleSection = {
  id: string;
  title: string;
  entries: MemoryArticleEntry[];
};

export type MemoryFigureCluster = {
  label: string;
  strength: number;
  status: string;
};

export type MemoryArticleModel = {
  title: string;
  subtitle: string;
  totalMemories: number;
  totalLabel: string;
  lastUpdatedLabel: string;
  primaryPattern: string;
  recurringMotif: string;
  leadText: string;
  isStub: boolean;
  stubText: string;
  figureTitle: string;
  figureCopy: string;
  figureCaption: string;
  clusters: MemoryFigureCluster[];
  sections: MemoryArticleSection[];
  sourceObservations: MemoryArticleEntry[];
  recallSample: MemoryArticleEntry | null;
  references: string[];
};

const FALLBACK_TITLE = "Human memory";
const FALLBACK_SUBTITLE = "From Noema, the local personal memory record";

export function buildMemoryArticleModel(graph: MemoryGraph | undefined): MemoryArticleModel {
  const entries = flattenEntries(graph?.documents ?? []);
  const totalMemories = graph?.pageInfo.total ?? entries.length;
  const lastUpdatedLabel = formatLatestUpdated(entries);
  const sourceObservations = uniqueSourceObservations(entries);
  const primaryPattern = entries.length > 0 ? "Remembered facts" : "No stable facts yet";
  const recurringMotif = entries.length > 0 ? inferRecurringMotif(entries) : "None yet";

  return {
    title: FALLBACK_TITLE,
    subtitle: FALLBACK_SUBTITLE,
    totalMemories,
    totalLabel: formatCount(totalMemories, "memory", "memories"),
    lastUpdatedLabel,
    primaryPattern,
    recurringMotif,
    leadText: buildLeadText(entries.length),
    isStub: entries.length < 3,
    stubText: buildStubText(entries.length),
    figureTitle: buildFigureTitle(entries.length),
    figureCopy: buildFigureCopy(entries.length),
    figureCaption: "Fig. 1. Prominent themes in the memory record, grouped by loaded entries.",
    clusters: buildClusters(entries),
    sections: buildSections(entries),
    sourceObservations,
    recallSample: entries[0] ?? null,
    references: buildReferences(entries, sourceObservations)
  };
}

function flattenEntries(documents: MemoryGraphDocument[]): MemoryArticleEntry[] {
  return documents.flatMap((document) =>
    document.memoryEntries
      .map((entry) => memoryEntryFromGraph(document, entry))
      .filter((entry): entry is MemoryArticleEntry => entry !== null)
  );
}

function memoryEntryFromGraph(
  document: MemoryGraphDocument,
  entry: MemoryGraphEntry
): MemoryArticleEntry | null {
  const text = entry.content ?? entry.summary ?? entry.title;
  if (!text?.trim()) {
    return null;
  }
  return {
    id: entry.id,
    text: text.trim(),
    createdAt: entry.createdAt || null,
    updatedAt: entry.updatedAt || null,
    sourceTitle: document.title ?? "Memory source",
    sourceObservation: sourceObservationFromMetadata(entry.metadata),
    scoreLabel: null
  };
}

function buildLeadText(entryCount: number): string {
  if (entryCount === 0) {
    return "Noema has not formed durable facts about the human yet. User-authored observations will appear here only after Mnemosyne extracts facts from them.";
  }
  return "This article summarizes the durable facts Noema currently knows about the human. Each fact is linked back to the user-authored observation that produced it, keeping memory visible instead of hidden in model state.";
}

function buildStubText(entryCount: number): string {
  if (entryCount === 0) {
    return "This memory article is a stub. Noema can expand it once Mnemosyne extracts its first durable facts.";
  }
  return "This memory article is a stub. Noema can expand it as Mnemosyne extracts more durable facts from future conversations.";
}

function buildFigureTitle(entryCount: number): string {
  if (entryCount === 0) {
    return "No memory themes have taken root yet.";
  }
  return "The memory record is beginning to take shape.";
}

function buildFigureCopy(entryCount: number): string {
  if (entryCount === 0) {
    return "Once memories exist, this figure summarizes loaded themes, recency, and retrieval visibility.";
  }
  return `${formatCount(entryCount, "remembered fact", "remembered facts")} are available for inspection. More specific themes will appear as Noema receives richer fact metadata.`;
}

function buildClusters(entries: MemoryArticleEntry[]): MemoryFigureCluster[] {
  const loaded = entries.length;
  return [
    {
      label: "Loaded entries",
      strength: loaded > 0 ? 100 : 0,
      status: loaded > 0 ? "available" : "empty"
    },
    {
      label: "Stable roots",
      strength: loaded > 0 ? 62 : 0,
      status: "metadata pending"
    },
    {
      label: "Recent growth",
      strength: loaded > 0 ? 38 : 0,
      status: "metadata pending"
    },
    {
      label: "Open questions",
      strength: loaded > 0 ? 18 : 0,
      status: "review later"
    }
  ];
}

function buildSections(entries: MemoryArticleEntry[]): MemoryArticleSection[] {
  return [
    {
      id: "remembered-facts",
      title: "Remembered facts",
      entries
    }
  ];
}

function buildReferences(
  entries: MemoryArticleEntry[],
  sourceObservations: MemoryArticleEntry[]
): string[] {
  if (entries.length === 0) {
    return ["No extracted facts have been returned by Mnemosyne yet."];
  }
  if (sourceObservations.length > 0) {
    return sourceObservations.map((entry, index) => {
      const timestamp = entry.updatedAt ?? entry.createdAt;
      const timestampLabel = timestamp ? `, ${formatDateTime(timestamp)}` : "";
      return `${index + 1}. ${entry.sourceTitle}${timestampLabel}.`;
    });
  }
  const sourceTitles = [...new Set(entries.map((entry) => entry.sourceTitle))];
  return sourceTitles.map((title) => `Source group: ${title}.`);
}

function uniqueSourceObservations(entries: MemoryArticleEntry[]): MemoryArticleEntry[] {
  const seen = new Set<string>();
  const observations: MemoryArticleEntry[] = [];
  for (const entry of entries) {
    const source = entry.sourceObservation?.trim();
    if (!source || seen.has(source)) {
      continue;
    }
    seen.add(source);
    observations.push(entry);
  }
  return observations;
}

function sourceObservationFromMetadata(metadata: unknown): string | null {
  if (!metadata || typeof metadata !== "object" || !("sourceObservation" in metadata)) {
    return null;
  }
  const value = (metadata as { sourceObservation?: unknown }).sourceObservation;
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function inferRecurringMotif(entries: MemoryArticleEntry[]): string {
  const first = entries[0]?.text.trim();
  if (!first) {
    return "None yet";
  }
  return first.length > 42 ? `${first.slice(0, 39)}...` : first;
}

function formatLatestUpdated(entries: MemoryArticleEntry[]): string {
  const latest = entries
    .map((entry) => entry.updatedAt ?? entry.createdAt)
    .filter((value): value is string => Boolean(value))
    .sort()
    .at(-1);
  if (!latest) {
    return "Not yet";
  }
  return formatDateTime(latest);
}

export function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(date);
}

export function formatCount(count: number, singular: string, plural: string): string {
  return `${count} ${count === 1 ? singular : plural}`;
}
