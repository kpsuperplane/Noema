import { parseOutlineFromMarkdown, type OutlineItem } from "@astryxdesign/core/Outline";
import type { MarkdownSource } from "@/components/MarkdownContent";
import type { GraphqlNativeMemorySourceKind } from "@/generated/graphql";

export interface MemoryArticleModel {
  content: string;
  outline: OutlineItem[];
  sources: Record<string, MarkdownSource>;
  citationGroups: MemoryCitationGroup[];
}

export interface MemoryEvidenceSource {
  source: string;
  kind: GraphqlNativeMemorySourceKind;
  excerpt: string | null;
  createdAt: string | null;
}

export interface MemoryCitationGroup {
  sources: MemoryEvidenceSource[];
}

export function buildMemoryArticle(body: string, citationGroups: readonly MemoryCitationGroup[]): MemoryArticleModel {
  const content = body.replace(/\[\^(\d+)\]/g, (marker, rawIndex: string) => {
    const index = Number(rawIndex) - 1;
    return citationGroups[index] ? `[memory-citation-${index + 1}]` : marker;
  });
  const sources: Record<string, MarkdownSource> = {};
  for (const [index, citation] of citationGroups.entries()) {
    sources[`memory-citation-${index + 1}`] = {
      title: citation.sources.map((source) => source.kind).join(", "),
      url: String(index)
    };
  }
  return { content, outline: parseOutlineFromMarkdown(content), sources, citationGroups: [...citationGroups] };
}

export function memoryHeadingId(label: string): string {
  return (
    label
      .trim()
      .toLowerCase()
      .replace(/['"]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "section"
  );
}
