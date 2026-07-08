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
  figureTitle: string;
  figureCopy: string;
  figureCaption: string;
  clusters: MemoryFigureCluster[];
  sections: MemoryArticleSection[];
  recallSample: MemoryArticleEntry | null;
  references: string[];
};

const FALLBACK_TITLE = "Human memory";
const FALLBACK_SUBTITLE = "From Noema, the local personal memory record";

export function buildMemoryArticleModel(graph: MemoryGraph | undefined): MemoryArticleModel {
  const entries = flattenEntries(graph?.documents ?? []);
  const totalMemories = graph?.pageInfo.total ?? entries.length;
  const lastUpdatedLabel = formatLatestUpdated(entries);
  const primaryPattern = entries.length > 0 ? "Known memories" : "No stable pattern yet";
  const recurringMotif = entries.length > 0 ? "Memory entries" : "None yet";

  return {
    title: FALLBACK_TITLE,
    subtitle: FALLBACK_SUBTITLE,
    totalMemories,
    totalLabel: formatCount(totalMemories, "memory", "memories"),
    lastUpdatedLabel,
    primaryPattern,
    recurringMotif,
    leadText: buildLeadText(entries.length),
    figureTitle: buildFigureTitle(entries.length),
    figureCopy: buildFigureCopy(entries.length),
    figureCaption: "Fig. 1. Prominent themes in the memory record, grouped by loaded entries.",
    clusters: buildClusters(entries),
    sections: buildSections(entries),
    recallSample: entries[0] ?? null,
    references: buildReferences(entries)
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
    scoreLabel: null
  };
}

function buildLeadText(entryCount: number): string {
  if (entryCount === 0) {
    return "Noema has not formed durable human memories yet. New user-authored observations will appear here after Mnemosyne processes them.";
  }
  return "This page collects user-authored observations Noema may use when responding to the local human. The current record is shown as an inspectable article rather than hidden model state.";
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
  return `${formatCount(entryCount, "loaded memory", "loaded memories")} are available for inspection. More specific themes will appear as Noema receives richer provenance and categorization metadata.`;
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
      id: "memory-entries",
      title: "Memory entries",
      entries
    }
  ];
}

function buildReferences(entries: MemoryArticleEntry[]): string[] {
  if (entries.length === 0) {
    return ["No source memories have been returned by Mnemosyne yet."];
  }
  const sourceTitles = [...new Set(entries.map((entry) => entry.sourceTitle))];
  return sourceTitles.map((title) => `Source group: ${title}.`);
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
