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
  sourceKind: string | null;
  sourceConversationId: string | null;
  sourceTurnId: string | null;
  sourceItemId: string | null;
  sourceMessageText: string | null;
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

export type MemoryArticleReference = {
  id: string;
  label: string;
  memoryUpdatedAtLabel: string | null;
  sourceMessage: string | null;
  sourceMeta: string[];
  citedFacts: MemoryArticleEntry[];
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
  recallSample: MemoryArticleEntry | null;
  references: MemoryArticleReference[];
};

const FALLBACK_TITLE = "Local human";
const FALLBACK_SUBTITLE = "From Noema, the private memory encyclopedia";

export function buildMemoryArticleModel(graph: MemoryGraph | undefined): MemoryArticleModel {
  const entries = flattenEntries(graph?.documents ?? []);
  const totalMemories = graph?.pageInfo.total ?? entries.length;
  const lastUpdatedLabel = formatLatestUpdated(entries);
  const subjectName = inferSubjectName(entries) ?? subjectNameFromArticle(graph?.article);
  const articleContent = parseArticleMarkdown(graph?.article, entries);
  const title = graph?.article.title?.trim() || subjectName || FALLBACK_TITLE;
  const references = buildReferences(entries);
  const referenceCountLabel = formatCount(references.length, "citation", "citations");

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
    recallSample: entries[0] ?? null,
    references
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
    sourceKind: entry.source?.kind ?? sourceStringFromMetadata(entry.metadata, [
      "sourceKind",
      "source_kind"
    ]),
    sourceConversationId:
      entry.source?.conversationId ??
      sourceStringFromMetadata(entry.metadata, ["noemaConversationId", "conversation_id"]),
    sourceTurnId: entry.source?.turnId ?? sourceStringFromMetadata(entry.metadata, ["turnId", "turn_id"]),
    sourceItemId:
      entry.source?.itemId ??
      sourceStringFromMetadata(entry.metadata, ["userItemId", "source_item_id", "item_id"]),
    sourceMessageText: entry.source?.messageText ?? null,
    sourceObservation: sourceObservationFromEntry(entry),
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

function buildReferences(entries: MemoryArticleEntry[]): MemoryArticleReference[] {
  if (entries.length === 0) {
    return [];
  }
  const groups = new Map<string, MemoryArticleEntry[]>();
  for (const entry of entries) {
    const key = citationSourceKey(entry);
    const group = groups.get(key) ?? [];
    group.push(entry);
    groups.set(key, group);
  }
  return [...groups.entries()].map(([sourceKey, citedFacts], index) => {
    const first = citedFacts[0];
    const timestamp = first?.updatedAt ?? first?.createdAt;
    return {
      id: sourceKey || first?.id || `citation-${index + 1}`,
      label: referenceLabel(first),
      memoryUpdatedAtLabel: timestamp ? formatDateTime(timestamp) : null,
      sourceMessage: first?.sourceMessageText ?? first?.sourceObservation ?? null,
      sourceMeta: sourceMetaLabels(first),
      citedFacts
    };
  });
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

function sourceObservationFromEntry(entry: MemoryGraphEntry): string | null {
  return (
    entry.source?.messageText ??
    sourceStringFromMetadata(entry.metadata, ["sourceObservation", "source_observation"])
  );
}

function sourceStringFromMetadata(metadata: unknown, keys: string[]): string | null {
  if (!metadata || typeof metadata !== "object") {
    return null;
  }
  for (const key of keys) {
    const value = (metadata as Record<string, unknown>)[key];
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return null;
}

function citationSourceKey(entry: MemoryArticleEntry): string {
  return (
    entry.sourceItemId ??
    entry.sourceTurnId ??
    entry.sourceMessageText ??
    entry.sourceObservation ??
    entry.sourceConversationId ??
    entry.sourceTitle ??
    entry.id
  );
}

function referenceLabel(entry: MemoryArticleEntry | undefined): string {
  if (!entry) {
    return "Local memory source";
  }
  if (entry.sourceConversationId || entry.sourceTurnId || entry.sourceItemId) {
    return "Noema conversation";
  }
  return entry.sourceTitle || sourceKindLabel(entry.sourceKind) || "Local memory source";
}

function sourceMetaLabels(entry: MemoryArticleEntry | undefined): string[] {
  if (!entry) {
    return [];
  }
  return [
    sourceKindLabel(entry.sourceKind),
    entry.sourceConversationId ? `Conversation ${shortId(entry.sourceConversationId)}` : null,
    entry.sourceTurnId ? `Turn ${shortId(entry.sourceTurnId)}` : null,
    entry.sourceItemId ? `Message ${shortId(entry.sourceItemId)}` : null
  ].filter((label): label is string => Boolean(label));
}

function sourceKindLabel(kind: string | null): string | null {
  if (!kind) {
    return null;
  }
  return kind
    .split(/[_-]+/u)
    .filter(Boolean)
    .map((part) => `${part[0]?.toUpperCase() ?? ""}${part.slice(1)}`)
    .join(" ");
}

function shortId(id: string): string {
  if (id.length <= 14) {
    return id;
  }
  return `${id.slice(0, 7)}...${id.slice(-4)}`;
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
