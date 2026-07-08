import type { MemoryGraphQuery } from "@/generated/graphql";

type MemoryGraph = MemoryGraphQuery["memoryGraph"];
type MemoryGraphDocument = MemoryGraph["documents"][number];
type MemoryGraphEntry = MemoryGraphDocument["memoryEntries"][number];
type MemoryGraphArticle = MemoryGraph["article"];

export type MemoryArticleEntry = {
  id: string;
  text: string;
  displayText: string;
  createdAt: string | null;
  updatedAt: string | null;
  sourceTitle: string;
  sourceObservation: string | null;
  scoreLabel: string | null;
};

export type MemoryArticleSection = {
  id: string;
  title: string;
  paragraphs: string[];
};

export type MemoryFigureCluster = {
  label: string;
  strength: number;
  status: string;
};

export type MemoryArticleModel = {
  title: string;
  subtitle: string;
  subjectName: string | null;
  totalMemories: number;
  totalLabel: string;
  lastUpdatedLabel: string;
  primaryPattern: string;
  recurringMotif: string;
  leadText: string;
  leadParagraphs: string[];
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

const FALLBACK_TITLE = "Local human";
const FALLBACK_SUBTITLE = "A biographical article from local memory";

export function buildMemoryArticleModel(graph: MemoryGraph | undefined): MemoryArticleModel {
  const entries = flattenEntries(graph?.documents ?? []);
  const totalMemories = graph?.pageInfo.total ?? entries.length;
  const lastUpdatedLabel = formatLatestUpdated(entries);
  const sourceObservations = uniqueSourceObservations(entries);
  const subjectName = inferSubjectName(entries) ?? subjectNameFromArticle(graph?.article);
  const articleContent = parseArticleMarkdown(graph?.article, entries);
  const primaryPattern = entries.length > 0 ? "Brief biography" : "No stable facts yet";
  const recurringMotif = entries.length > 0 ? inferRecurringMotif(entries) : "None yet";

  return {
    title: graph?.article.title?.trim() || subjectName || FALLBACK_TITLE,
    subtitle: graph?.article.subtitle?.trim() || FALLBACK_SUBTITLE,
    subjectName,
    totalMemories,
    totalLabel: formatCount(totalMemories, "fact", "facts"),
    lastUpdatedLabel,
    primaryPattern,
    recurringMotif,
    leadText: articleContent.leadText,
    leadParagraphs: articleContent.leadParagraphs,
    isStub: entries.length < 3,
    stubText: buildStubText(entries.length),
    figureTitle: buildFigureTitle(entries.length),
    figureCopy: buildFigureCopy(entries.length),
    figureCaption: "Fig. 1. Prominent themes in the memory record, grouped by loaded entries.",
    clusters: buildClusters(entries),
    sections: articleContent.sections,
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
    displayText: biographicalTextFromFact(text.trim()),
    createdAt: entry.createdAt || null,
    updatedAt: entry.updatedAt || null,
    sourceTitle: document.title ?? "Memory source",
    sourceObservation: sourceObservationFromMetadata(entry.metadata),
    scoreLabel: null
  };
}

function buildStubText(entryCount: number): string {
  if (entryCount === 0) {
    return "This biographical article is a stub. It will expand once the first durable facts are recorded.";
  }
  return "This biographical article is a stub. Additional durable facts may expand it over time.";
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
  return `${formatCount(entryCount, "biographical fact", "biographical facts")} are available for inspection. More specific themes will appear as richer metadata is recorded.`;
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
      return `${index + 1}. Source observation${timestampLabel}.`;
    });
  }
  const sourceTitles = [...new Set(entries.map((entry) => entry.sourceTitle))];
  return sourceTitles.map((title) => `Source group: ${title}.`);
}

function parseArticleMarkdown(
  article: MemoryGraphArticle | undefined,
  entries: MemoryArticleEntry[]
): { leadText: string; leadParagraphs: string[]; sections: MemoryArticleSection[] } {
  const markdown = article?.markdown?.trim();
  if (!markdown) {
    return fallbackArticleContent(entries);
  }

  const sections: MemoryArticleSection[] = [];
  const leadParagraphs: string[] = [];
  let current: MemoryArticleSection | null = null;
  let paragraphLines: string[] = [];

  const flushParagraph = () => {
    const paragraph = paragraphLines.join(" ").replace(/\s+/gu, " ").trim();
    if (paragraph) {
      const stripped = stripInlineMarkdown(paragraph);
      if (current) {
        current.paragraphs.push(stripped);
      } else {
        leadParagraphs.push(stripped);
      }
    }
    paragraphLines = [];
  };

  const flushSection = () => {
    flushParagraph();
    if (current && current.paragraphs.length > 0) {
      sections.push(current);
    }
  };

  for (const rawLine of markdown.split(/\r?\n/u)) {
    const line = rawLine.trim();
    if (!line) {
      flushParagraph();
      continue;
    }
    if (line.startsWith("# ")) {
      continue;
    }
    if (line.startsWith("## ")) {
      flushSection();
      const title = stripInlineMarkdown(line.slice(3).trim()) || "Biography";
      current = {
        id: slugFromTitle(title),
        title,
        paragraphs: []
      };
      continue;
    }
    paragraphLines.push(line);
  }
  flushSection();

  if (leadParagraphs.length === 0 && sections.length === 0) {
    return fallbackArticleContent(entries);
  }
  const leadText =
    leadParagraphs[0] ??
    sections[0]?.paragraphs[0] ??
    "Little is currently known about the local human.";
  return {
    leadText,
    leadParagraphs: leadParagraphs.length > 0 ? leadParagraphs : [leadText],
    sections
  };
}

function fallbackArticleContent(entries: MemoryArticleEntry[]): {
  leadText: string;
  leadParagraphs: string[];
  sections: MemoryArticleSection[];
} {
  if (entries.length === 0) {
    const leadText = "Little is currently known about the local human.";
    return {
      leadText,
      leadParagraphs: [leadText],
      sections: []
    };
  }
  const paragraphs = entries.map((entry) => entry.displayText);
  const leadText = paragraphs[0] ?? "The local human is described by the available facts.";
  return {
    leadText,
    leadParagraphs: [leadText],
    sections:
      paragraphs.length > 1
        ? [{ id: "biography", title: "Biography", paragraphs: paragraphs.slice(1) }]
        : []
  };
}

function stripInlineMarkdown(value: string): string {
  return value
    .replace(/\*\*([^*]+)\*\*/gu, "$1")
    .replace(/\*([^*]+)\*/gu, "$1")
    .replace(/`([^`]+)`/gu, "$1")
    .replace(/\[([^\]]+)\]\([^)]+\)/gu, "$1")
    .trim();
}

function subjectNameFromArticle(article: MemoryGraphArticle | undefined): string | null {
  const title = article?.title?.trim();
  if (!title || title === FALLBACK_TITLE) {
    return null;
  }
  return title;
}

function slugFromTitle(title: string): string {
  const slug = title
    .toLowerCase()
    .replace(/[^a-z0-9]+/gu, "-")
    .replace(/^-|-$/gu, "");
  return slug || "biography";
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

function inferSubjectName(entries: MemoryArticleEntry[]): string | null {
  for (const entry of entries) {
    const match = /^I(?:'m| am)\s+([A-Z][A-Za-z0-9_-]{1,40})!?\.?$/u.exec(entry.text.trim());
    if (match?.[1]) {
      return match[1];
    }
  }
  return null;
}

function biographicalTextFromFact(text: string): string {
  const selfIntroduction = /^I(?:'m| am)\s+([A-Z][A-Za-z0-9_-]{1,40})!?\.?$/u.exec(text);
  if (selfIntroduction?.[1]) {
    return `${selfIntroduction[1]} is the local human.`;
  }
  return text.replace(/^The user\b/u, "The human");
}

function inferRecurringMotif(entries: MemoryArticleEntry[]): string {
  const first = entries[0]?.displayText.trim();
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
