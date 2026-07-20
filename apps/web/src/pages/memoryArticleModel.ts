import { parseOutlineFromMarkdown, type OutlineItem } from "@astryxdesign/core/Outline";
import type { MarkdownSource } from "@astryxdesign/core/Markdown";

export interface MemoryArticleModel {
  content: string;
  outline: OutlineItem[];
  references: Array<{ source: string; number: number }>;
  sources: Record<string, MarkdownSource>;
}

export function buildMemoryArticle(body: string, manifest: readonly string[]): MemoryArticleModel {
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
  const manifestSet = new Set(manifest);
  const references = orderedSources
    .filter((source) => manifestSet.has(source))
    .map((source, index) => ({ source, number: index + 1 }));
  const sources = Object.fromEntries(
    references.map(({ source, number }) => [source, { title: `Conversation source ${number}` }])
  );
  return { content, outline: parseOutlineFromMarkdown(content), references, sources };
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
