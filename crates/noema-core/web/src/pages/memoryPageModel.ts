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

export type MemoryInfoboxRow = {
  label: string;
  value: string;
};

export type MemoryArticleModel = {
  title: string;
  subtitle: string;
  subjectName: string | null;
  totalMemories: number;
  initials: string;
  portraitCaption: string;
  infoboxRows: MemoryInfoboxRow[];
  referenceCountLabel: string;
  leadText: string;
  leadParagraphs: string[];
  isStub: boolean;
  stubText: string;
  sections: MemoryArticleSection[];
  sourceObservations: MemoryArticleEntry[];
  recallSample: MemoryArticleEntry | null;
  references: string[];
};

const FALLBACK_TITLE = "Local human";
const FALLBACK_SUBTITLE = "From Noema, the private memory encyclopedia";

export function buildMemoryArticleModel(graph: MemoryGraph | undefined): MemoryArticleModel {
  const entries = flattenEntries(graph?.documents ?? []);
  const totalMemories = graph?.pageInfo.total ?? entries.length;
  const lastUpdatedLabel = formatLatestUpdated(entries);
  const sourceObservations = uniqueSourceObservations(entries);
  const subjectName = inferSubjectName(entries) ?? subjectNameFromArticle(graph?.article);
  const articleContent = parseArticleMarkdown(graph?.article, entries);
  const title = graph?.article.title?.trim() || subjectName || FALLBACK_TITLE;
  const referenceCountLabel = formatCount(
    sourceObservations.length || entries.length,
    "citation",
    "citations"
  );

  return {
    title,
    subtitle: graph?.article.subtitle?.trim() || FALLBACK_SUBTITLE,
    subjectName,
    totalMemories,
    initials: initialsForTitle(title),
    portraitCaption: subjectName ?? title,
    infoboxRows: buildInfoboxRows({
      entries,
      knownFor: articleContent.leadText,
      lastUpdatedLabel,
      referenceCountLabel,
      subjectName
    }),
    referenceCountLabel,
    leadText: articleContent.leadText,
    leadParagraphs: articleContent.leadParagraphs,
    isStub: entries.length < 3,
    stubText: buildStubText(entries.length),
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

function buildInfoboxRows({
  entries,
  knownFor,
  lastUpdatedLabel,
  referenceCountLabel,
  subjectName
}: {
  entries: MemoryArticleEntry[];
  knownFor: string;
  lastUpdatedLabel: string;
  referenceCountLabel: string;
  subjectName: string | null;
}): MemoryInfoboxRow[] {
  const rows: MemoryInfoboxRow[] = [
    { label: "Name", value: subjectName ?? "Unknown" },
    { label: "Known for", value: truncateInfoboxValue(knownFor) }
  ];
  const website = findWebsite(entries);
  if (website) {
    rows.push({ label: "Website", value: website });
  }
  rows.push(
    { label: "References", value: referenceCountLabel },
    { label: "Last updated", value: lastUpdatedLabel }
  );
  return rows;
}

function findWebsite(entries: MemoryArticleEntry[]): string | null {
  for (const entry of entries) {
    const match = /\b(?:https?:\/\/)?(?:www\.)?([a-z0-9-]+\.[a-z]{2,}(?:\/[^\s]*)?)/iu.exec(
      entry.text
    );
    if (match?.[1]) {
      return match[1].replace(/[),.;]+$/u, "");
    }
  }
  return null;
}

function truncateInfoboxValue(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (!normalized) {
    return "Not enough information";
  }
  return normalized.length > 96 ? `${normalized.slice(0, 93)}...` : normalized;
}

function buildReferences(
  entries: MemoryArticleEntry[],
  sourceObservations: MemoryArticleEntry[]
): string[] {
  if (entries.length === 0) {
    return ["No local memory citations are available yet."];
  }
  if (sourceObservations.length > 0) {
    return sourceObservations.map((entry) => {
      const timestamp = entry.updatedAt ?? entry.createdAt;
      const timestampLabel = timestamp ? `, ${formatDateTime(timestamp)}` : "";
      return `Local memory citation${timestampLabel}.`;
    });
  }
  const sourceTitles = [...new Set(entries.map((entry) => entry.sourceTitle))];
  return sourceTitles.map((title) => `Local memory source: ${title}.`);
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

function initialsForTitle(title: string): string {
  const words = title
    .split(/\s+/u)
    .map((word) => word.replace(/[^A-Za-z0-9]/gu, ""))
    .filter(Boolean);
  if (words.length === 0) {
    return "?";
  }
  return words
    .slice(0, 2)
    .map((word) => word[0]?.toUpperCase() ?? "")
    .join("");
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
