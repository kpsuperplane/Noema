import { Divider } from "@astryxdesign/core/Divider";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { parseMarkdown } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { BookOpen } from "lucide-react";
import { useMemo, useState } from "react";
import {
  MarkdownContent,
  type MarkdownComponents,
  type MarkdownContentProps,
  type MarkdownSource
} from "@/components/MarkdownContent";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";

export type ProviderCitation = {
  title: string;
  url: string;
  startIndex: number | null;
  endIndex: number | null;
};

type ProviderCitationContent = {
  text: string;
  sources: Record<string, MarkdownSource>;
  citations: ProviderCitation[];
};

type ProviderCitationMarkdownProps = Omit<
  MarkdownContentProps,
  "children" | "components" | "sources"
> & {
  text: string;
  citations: readonly ProviderCitation[];
};

const SOURCES_ACTION_URL = "noema-sources://open";

const styles = stylex.create({
  sourcesAction: {
    marginInlineStart: "var(--spacing-1)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: "var(--radius-pill)",
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "none",
    verticalAlign: "middle",
    ":hover": { backgroundColor: "var(--noema-surface-hover)" }
  },
  sourceLink: {
    display: "block",
    padding: "var(--spacing-2)",
    borderRadius: "var(--radius-element)",
    color: "inherit",
    textDecoration: "none",
    cursor: "pointer",
    ":hover": { backgroundColor: "var(--color-background-muted)" },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 2
    }
  },
  sourceNumber: { minWidth: "var(--spacing-4)" },
  sourceText: { minWidth: 0 },
  sourceTitle: { overflowWrap: "anywhere" },
  sourceHost: { overflowWrap: "anywhere" }
});

export function ProviderCitationMarkdown({
  text,
  citations,
  ...markdownProps
}: ProviderCitationMarkdownProps) {
  const [open, setOpen] = useState(false);
  const content = useMemo(() => {
    const cited = providerCitationContent(text, citations);
    if (cited.citations.length === 0) return cited;
    return { ...cited, ...citationContentWithSourcesAction(cited.text, cited.sources) };
  }, [citations, text]);
  const components = useMemo<MarkdownComponents>(
    () => ({
      citation: (props) => <CitationReference {...props} onOpenSources={() => setOpen(true)} />
    }),
    []
  );

  return (
    <>
      <MarkdownContent
        {...markdownProps}
        citationStyle="number"
        components={components}
        sources={content.sources}
      >
        {content.text}
      </MarkdownContent>
      <ProviderCitationDialog citations={content.citations} open={open} onOpenChange={setOpen} />
    </>
  );
}

export function CitationReference({
  source,
  number,
  onOpenSources
}: {
  source: MarkdownSource;
  number: number;
  variant: "label" | "number";
  onOpenSources: () => void;
}) {
  if (source.url === SOURCES_ACTION_URL) {
    return (
      <IconButton
        type="button"
        size="sm"
        variant="ghost"
        label="Sources"
        tooltip="Sources"
        icon={<BookOpen aria-hidden="true" size={12} />}
        onClick={onOpenSources}
        xstyle={styles.sourcesAction}
      />
    );
  }
  return <sup>{number}</sup>;
}

export function citationContentWithSourcesAction(
  text: string,
  sources: Record<string, MarkdownSource>
): { text: string; sources: Record<string, MarkdownSource> } {
  const sourceId = unusedSourcesActionId(text, sources);
  const lastBlock = parseMarkdown(text).at(-1)?.type;
  const separator = lastBlock === "paragraph" ? " " : "\n\n";
  return {
    text: `${text}${separator}[${sourceId}]`,
    sources: {
      ...sources,
      [sourceId]: { title: "Sources", url: SOURCES_ACTION_URL }
    }
  };
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
  const uniqueCitations: ProviderCitation[] = [];
  const markersByIndex = new Map<number, Set<string>>();
  const fallbackMarkers = new Set<string>();

  const orderedCitations = citations
    .map((citation, inputIndex) => ({ citation, inputIndex }))
    .sort((left, right) => {
      const leftEnd = validCitationEnd(text, left.citation);
      const rightEnd = validCitationEnd(text, right.citation);
      if (leftEnd === null && rightEnd !== null) return 1;
      if (leftEnd !== null && rightEnd === null) return -1;
      if (leftEnd !== rightEnd) return (leftEnd ?? 0) - (rightEnd ?? 0);
      return left.inputIndex - right.inputIndex;
    });

  for (const { citation } of orderedCitations) {
    let sourceId = sourceIdByUrl.get(citation.url);
    if (!sourceId) {
      sourceId = unusedSourceId(text, sources, sourceIdByUrl.size + 1);
      sourceIdByUrl.set(citation.url, sourceId);
      sources[sourceId] = { title: citation.title, url: citation.url };
      uniqueCitations.push(citation);
    }
    const endIndex = validCitationEnd(text, citation);
    if (endIndex === null) {
      fallbackMarkers.add(sourceId);
      continue;
    }
    const markers = markersByIndex.get(endIndex) ?? new Set<string>();
    markers.add(sourceId);
    markersByIndex.set(endIndex, markers);
  }

  let citedText = text;
  for (const [index, sourceIds] of [...markersByIndex].sort(([left], [right]) => right - left)) {
    citedText = `${citedText.slice(0, index)}${citationMarkers(sourceIds)}${citedText.slice(index)}`;
  }
  if (fallbackMarkers.size > 0) {
    citedText = `${citedText}${citedText.trim() ? " " : ""}${citationMarkers(fallbackMarkers)}`;
  }
  return { text: citedText, sources, citations: uniqueCitations };
}

function validCitationEnd(text: string, citation: ProviderCitation): number | null {
  const { startIndex, endIndex } = citation;
  return endIndex !== null &&
    (startIndex === null || startIndex < endIndex) &&
    endIndex <= text.length &&
    isUtf16Boundary(text, endIndex)
    ? endIndex
    : null;
}

function isUtf16Boundary(text: string, index: number): boolean {
  if (index <= 0 || index >= text.length) return true;
  const previous = text.charCodeAt(index - 1);
  const next = text.charCodeAt(index);
  return !(previous >= 0xd800 && previous <= 0xdbff && next >= 0xdc00 && next <= 0xdfff);
}

function ProviderCitationDialog({
  citations,
  open,
  onOpenChange
}: {
  citations: readonly ProviderCitation[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="info"
      width={520}
      maxHeight="min(680px, calc(100dvh - var(--spacing-8)))"
      aria-label="Sources"
    >
      <Layout
        height="auto"
        header={<DialogHeader title="Sources" onOpenChange={onOpenChange} hasDivider />}
        content={
          <LayoutContent>
            <VStack gap={1}>
              {citations.map((citation, index) => (
                <VStack key={citation.url} gap={1}>
                  <a
                    href={citation.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    aria-label={`Source ${index + 1}: ${citation.title}`}
                    {...stylex.props(styles.sourceLink)}
                  >
                    <HStack gap={2} vAlign="start">
                      <Text
                        type="supporting"
                        color="accent"
                        weight="semibold"
                        xstyle={styles.sourceNumber}
                      >
                        {index + 1}
                      </Text>
                      <VStack gap={0.5} xstyle={styles.sourceText}>
                        <Text type="body" weight="semibold" xstyle={styles.sourceTitle}>
                          {citation.title}
                        </Text>
                        <Text type="supporting" color="secondary" xstyle={styles.sourceHost}>
                          {sourceHost(citation.url)}
                        </Text>
                      </VStack>
                    </HStack>
                  </a>
                  {index + 1 < citations.length ? <Divider /> : null}
                </VStack>
              ))}
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

function citationMarkers(sourceIds: Iterable<string>): string {
  return [...sourceIds].map((sourceId) => `[${sourceId}]`).join("");
}

function sourceHost(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
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

function unusedSourcesActionId(text: string, sources: Record<string, MarkdownSource>): string {
  let number = 1;
  while (
    sources[`noema-sources-${number}`] ||
    text.includes(`[noema-sources-${number}]`) ||
    text.includes(`【noema-sources-${number}】`)
  ) {
    number += 1;
  }
  return `noema-sources-${number}`;
}

function nonNegativeInteger(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
