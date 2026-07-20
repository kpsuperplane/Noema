import { parseOutlineFromMarkdown, type OutlineItem } from "@astryxdesign/core/Outline";
import type { MarkdownSource } from "@astryxdesign/core/Markdown";

export interface MemoryArticleModel {
  content: string;
  outline: OutlineItem[];
  sources: Record<string, MarkdownSource>;
}

export interface MemorySourceReference {
  source: string;
  excerpt: string | null;
}

export function buildMemoryArticle(body: string, sourceReferences: readonly MemorySourceReference[]): MemoryArticleModel {
  const sourceByLabel = new Map<string, string>();
  const definitionPattern = /^\[\^([^\]\s]+)\]:\s*`?([^`\n]+?)`?\s*$/gm;
  const withoutDefinitions = body.replace(definitionPattern, (_definition, label: string, source: string) => {
    sourceByLabel.set(label, source.trim());
    return "";
  });
  const orderedSources: string[] = [];
  const content = withoutDefinitions
    .replace(/\[\^([^\]\s]+)\]/g, (citation, label: string) => {
      const source = sourceByLabel.get(label);
      if (!source) return citation;
      if (!orderedSources.includes(source)) orderedSources.push(source);
      return `[${source}]`;
    })
    .replace(/\n{3,}/g, "\n\n")
    .trim();
  const referenceBySource = new Map(sourceReferences.map((reference) => [reference.source, reference]));
  const sources: Record<string, MarkdownSource> = {};
  for (const source of orderedSources) {
    const reference = referenceBySource.get(source);
    if (reference) sources[source] = { title: reference.excerpt ?? "The source conversation message is no longer available." };
  }
  return { content, outline: parseOutlineFromMarkdown(content), sources };
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
