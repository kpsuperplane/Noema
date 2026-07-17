import type { MemoryGraphQuery } from "@/generated/graphql";

type MemoryGraph = MemoryGraphQuery["memoryGraph"];
type MemoryGraphDocument = MemoryGraph["documents"][number];
type MemoryGraphEntry = MemoryGraphDocument["memoryEntries"][number];
type MemoryGraphArticle = MemoryGraph["article"];

export type MemoryArticleEntry = {
  id: string;
  citationKey: string;
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
  paragraphs: MemoryArticleParagraph[];
};

export type MemoryArticleParagraphPart =
  | { kind: "text"; text: string }
  | {
      kind: "citation";
      number: number;
      anchorId: string;
    };

export type MemoryArticleParagraph = {
  id: string;
  plainText: string;
  parts: MemoryArticleParagraphPart[];
};

export type MemoryInfoboxRow = {
  label: string;
  value: string;
};

export type MemoryArticleReference = {
  id: string;
  label: string;
  citationKeys: string[];
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
  leadParagraphs: MemoryArticleParagraph[];
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
  const allReferences = buildReferences(entries);
  const citationContext = buildCitationContext(graph?.article?.markdown, allReferences, entries);
  const articleContent = parseArticleMarkdown(
    graph?.article,
    entries,
    citationContext.targetsByKey
  );
  const title = graph?.article.title?.trim() || subjectName || FALLBACK_TITLE;
  const references = citationContext.references;
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
    citationKey: entry.citationKey,
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
      citationKeys: citedFacts.map((fact) => fact.citationKey),
      memoryUpdatedAtLabel: timestamp ? formatDateTime(timestamp) : null,
      sourceMessage: first?.sourceMessageText ?? first?.sourceObservation ?? null,
      sourceMeta: sourceMetaLabels(first),
      citedFacts
    };
  });
}

type CitationTarget = {
  number: number;
  referenceId: string;
};

function buildCitationContext(
  markdown: string | undefined,
  allReferences: MemoryArticleReference[],
  entries: MemoryArticleEntry[]
): { references: MemoryArticleReference[]; targetsByKey: Map<string, CitationTarget> } {
  const referenceByKey = new Map<string, MemoryArticleReference>();
  for (const reference of allReferences) {
    for (const citationKey of reference.citationKeys) {
      referenceByKey.set(citationKey, reference);
    }
  }

  const references: MemoryArticleReference[] = [];
  const numberByReferenceId = new Map<string, number>();
  const targetsByKey = new Map<string, CitationTarget>();
  const register = (citationKey: string) => {
    const reference = referenceByKey.get(citationKey);
    if (!reference) {
      return;
    }
    let number = numberByReferenceId.get(reference.id);
    if (!number) {
      references.push(reference);
      number = references.length;
      numberByReferenceId.set(reference.id, number);
      for (const groupedKey of reference.citationKeys) {
        targetsByKey.set(groupedKey, { number, referenceId: reference.id });
      }
    }
  };

  const citationKeys = markdown?.trim()
    ? articleCitationKeys(markdown)
    : entries.map((entry) => entry.citationKey);
  for (const citationKey of citationKeys) {
    register(citationKey);
  }
  return { references, targetsByKey };
}

function parseArticleMarkdown(
  article: MemoryGraphArticle | undefined,
  entries: MemoryArticleEntry[],
  targetsByKey: Map<string, CitationTarget>
): {
  leadText: string;
  leadParagraphs: MemoryArticleParagraph[];
  sections: MemoryArticleSection[];
} {
  const markdown = article?.markdown?.trim();
  if (!markdown) {
    return fallbackArticleContent(entries, targetsByKey);
  }

  const sections: MemoryArticleSection[] = [];
  const leadParagraphs: MemoryArticleParagraph[] = [];
  const citationOccurrences = new Map<number, number>();
  let current: MemoryArticleSection | null = null;
  let paragraphLines: string[] = [];
  let paragraphNumber = 0;

  const flushParagraph = () => {
    const paragraph = paragraphLines.join(" ").replace(/\s+/gu, " ").trim();
    if (paragraph) {
      paragraphNumber += 1;
      const parsed = articleParagraphFromMarkdown(
        paragraph,
        paragraphNumber,
        targetsByKey,
        citationOccurrences
      );
      if (current) {
        current.paragraphs.push(parsed);
      } else {
        leadParagraphs.push(parsed);
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
    return fallbackArticleContent(entries, targetsByKey);
  }
  const leadParagraph =
    leadParagraphs[0] ??
    sections[0]?.paragraphs[0] ??
    articleParagraphFromMarkdown(
      "Little is currently known about the local human.",
      paragraphNumber + 1,
      targetsByKey,
      citationOccurrences
    );
  return {
    leadText: leadParagraph.plainText,
    leadParagraphs: leadParagraphs.length > 0 ? leadParagraphs : [leadParagraph],
    sections
  };
}

function fallbackArticleContent(
  entries: MemoryArticleEntry[],
  targetsByKey: Map<string, CitationTarget>
): {
  leadText: string;
  leadParagraphs: MemoryArticleParagraph[];
  sections: MemoryArticleSection[];
} {
  const citationOccurrences = new Map<number, number>();
  if (entries.length === 0) {
    const leadText = "Little is currently known about the local human.";
    const leadParagraph = articleParagraphFromMarkdown(
      leadText,
      1,
      targetsByKey,
      citationOccurrences
    );
    return {
      leadText,
      leadParagraphs: [leadParagraph],
      sections: []
    };
  }
  const paragraphs = entries.map((entry, index) =>
    articleParagraphFromMarkdown(
      `${entry.displayText} [^${entry.citationKey}]`,
      index + 1,
      targetsByKey,
      citationOccurrences
    )
  );
  const leadParagraph = paragraphs[0] ??
    articleParagraphFromMarkdown(
      "The local human is described by the available facts.",
      1,
      targetsByKey,
      citationOccurrences
    );
  return {
    leadText: leadParagraph.plainText,
    leadParagraphs: [leadParagraph],
    sections:
      paragraphs.length > 1
        ? [{ id: "biography", title: "Biography", paragraphs: paragraphs.slice(1) }]
        : []
  };
}

function articleParagraphFromMarkdown(
  value: string,
  paragraphNumber: number,
  targetsByKey: Map<string, CitationTarget>,
  citationOccurrences: Map<number, number>
): MemoryArticleParagraph {
  const normalized = stripInlineMarkdown(value);
  const parts: MemoryArticleParagraphPart[] = [];
  let cursor = 0;
  let previousCitationReferenceId: string | null = null;
  for (const match of normalized.matchAll(/\[\^([a-z0-9_-]+)\]/giu)) {
    const index = match.index ?? cursor;
    const text = normalized.slice(cursor, index);
    if (text) {
      parts.push({ kind: "text", text });
      if (text.trim()) {
        previousCitationReferenceId = null;
      }
    }
    const target = match[1] ? targetsByKey.get(match[1]) : undefined;
    if (target && previousCitationReferenceId !== target.referenceId) {
      const occurrence = (citationOccurrences.get(target.number) ?? 0) + 1;
      citationOccurrences.set(target.number, occurrence);
      parts.push({
        kind: "citation",
        number: target.number,
        anchorId: `citation-${target.number}-${occurrence}`
      });
      previousCitationReferenceId = target.referenceId;
    }
    cursor = index + match[0].length;
  }
  const trailingText = normalized.slice(cursor);
  if (trailingText) {
    parts.push({ kind: "text", text: trailingText });
  }
  const plainText = parts
    .filter(
      (part): part is Extract<MemoryArticleParagraphPart, { kind: "text" }> =>
        part.kind === "text"
    )
    .map((part) => part.text)
    .join("")
    .replace(/\s+/gu, " ")
    .trim();
  return {
    id: `article-paragraph-${paragraphNumber}`,
    plainText,
    parts: parts.length > 0 ? parts : [{ kind: "text", text: plainText }]
  };
}

function articleCitationKeys(markdown: string): string[] {
  return [...markdown.matchAll(/\[\^([a-z0-9_-]+)\]/giu)]
    .map((match) => match[1])
    .filter((key): key is string => Boolean(key));
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
  if (entry.sourceMessageText || entry.sourceObservation) {
    return "Personal statement";
  }
  return sourceKindLabel(entry.sourceKind) || entry.sourceTitle || "Local memory source";
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
