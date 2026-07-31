import * as stylex from "@stylexjs/stylex";
import type { MarkdownSource } from "@astryxdesign/core/Markdown";

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
  root: {
    display: "flex",
    alignItems: "baseline",
    flexWrap: "wrap",
    gap: "var(--spacing-1-5)",
    minWidth: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 11,
    lineHeight: 1.4
  },
  label: {
    fontWeight: 600
  },
  links: {
    display: "inline-flex",
    flexWrap: "wrap",
    columnGap: "var(--spacing-1-5)",
    rowGap: "var(--spacing-1)"
  },
  link: {
    color: "var(--noema-text-link)",
    textDecoration: "none",
    ":hover": {
      textDecoration: "underline"
    }
  }
});

export function ProviderCitationSources({ citations }: { citations: readonly ProviderCitation[] }) {
  if (citations.length === 0) {
    return null;
  }

  return (
    <nav aria-label="Sources" {...stylex.props(styles.root)}>
      <span {...stylex.props(styles.label)}>Sources</span>
      <span {...stylex.props(styles.links)}>
        {citations.map((citation, index) => (
          <a
            key={`${citation.url}:${index}`}
            href={citation.url}
            target="_blank"
            rel="noopener noreferrer"
            {...stylex.props(styles.link)}
          >
            {index + 1}. {citation.title}
          </a>
        ))}
      </span>
    </nav>
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
      startIndex === null ||
      endIndex === null ||
      startIndex >= endIndex ||
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
