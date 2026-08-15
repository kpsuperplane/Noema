import { Citation } from "@astryxdesign/core/Citation";
import { HStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import type { MarkdownSource } from "@/components/MarkdownContent";

export type ProviderCitation = {
  title: string;
  url: string;
  startIndex: number | null;
  endIndex: number | null;
};

export type ProviderCitationContent = {
  text: string;
  sources: Record<string, MarkdownSource>;
  fallbackCitations: ProviderCitation[];
};

const styles = stylex.create({
  tags: {
    paddingBlockEnd: "var(--spacing-1)"
  }
});

export function ProviderCitationTags({ citations }: { citations: readonly ProviderCitation[] }) {
  if (citations.length === 0) {
    return null;
  }

  return (
    <HStack as="nav" aria-label="Sources" gap={1.5} wrap="wrap" xstyle={styles.tags}>
      {citations.map((citation, index) => (
        <Citation
          key={citation.url}
          number={index + 1}
          source={{ title: citation.title, url: citation.url }}
          variant="label"
        />
      ))}
    </HStack>
  );
}

export function providerCitationsFromMetadata(metadata: unknown): ProviderCitation[] {
  if (!isRecord(metadata) || !Array.isArray(metadata.citations)) {
    return [];
  }

  const seen = new Set<string>();
  return metadata.citations.flatMap((citation) => {
    if (!isRecord(citation)) {
      return [];
    }
    const title = typeof citation.title === "string" ? citation.title.trim() : "";
    const url = typeof citation.url === "string" ? citation.url.trim() : "";
    const startIndex = nonNegativeInteger(citation.start_index);
    const endIndex = nonNegativeInteger(citation.end_index);
    const key = `${url}\u0000${startIndex ?? ""}\u0000${endIndex ?? ""}`;
    if (!title || !/^https?:\/\//i.test(url) || seen.has(key)) {
      return [];
    }
    seen.add(key);
    return [{ title, url, startIndex, endIndex }];
  });
}

export function providerCitationContent(
  text: string,
  citations: readonly ProviderCitation[]
): ProviderCitationContent {
  const sourceIdByUrl = new Map<string, string>();
  const sources: Record<string, MarkdownSource> = {};
  const markersByIndex = new Map<number, Set<string>>();
  const citedUrls = new Set<string>();

  for (const citation of citations) {
    const { startIndex, endIndex } = citation;
    if (
      endIndex === null ||
      (startIndex !== null && startIndex >= endIndex) ||
      endIndex > text.length
    ) {
      continue;
    }

    let sourceId = sourceIdByUrl.get(citation.url);
    if (!sourceId) {
      sourceId = unusedSourceId(text, sources, sourceIdByUrl.size + 1);
      sourceIdByUrl.set(citation.url, sourceId);
      sources[sourceId] = { title: citation.title, url: citation.url };
    }
    const markers = markersByIndex.get(endIndex) ?? new Set<string>();
    markers.add(sourceId);
    markersByIndex.set(endIndex, markers);
    citedUrls.add(citation.url);
  }

  let citedText = text;
  for (const [index, sourceIds] of [...markersByIndex].sort(([left], [right]) => right - left)) {
    const markers = [...sourceIds].map((sourceId) => `[${sourceId}]`).join("");
    citedText = `${citedText.slice(0, index)}${markers}${citedText.slice(index)}`;
  }

  const fallbackCitations = citations.filter(
    (citation, index) =>
      !citedUrls.has(citation.url) &&
      citations.findIndex((candidate) => candidate.url === citation.url) === index
  );
  return { text: citedText, sources, fallbackCitations };
}

function unusedSourceId(
  text: string,
  sources: Record<string, MarkdownSource>,
  initialNumber: number
): string {
  let number = initialNumber;
  while (
    sources[`noema-citation-${number}`] ||
    text.includes(`[noema-citation-${number}]`) ||
    text.includes(`【noema-citation-${number}】`)
  ) {
    number += 1;
  }
  return `noema-citation-${number}`;
}

function nonNegativeInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
