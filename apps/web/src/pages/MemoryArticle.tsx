import { Divider } from "@astryxdesign/core/Divider";
import { Item } from "@astryxdesign/core/Item";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { ArrowRight } from "lucide-react";
import { useState, type ReactNode } from "react";
import { memoryPageUrlPath } from "@/app/routes";
import { MarkdownContent, type MarkdownComponents } from "@/components/MarkdownContent";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { ShellPageLayout, ShellPageSubtitle, ShellPageTrack } from "@/components/shell/ShellPageLayout";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";
import {
  CitationReference,
  citationContentWithSourcesAction
} from "@/components/transcript/ProviderCitationSources";
import type { GraphqlNativeMemorySourceKind } from "@/generated/graphql";
import { buildMemoryArticle, memoryHeadingId, type MemoryCitationGroup } from "@/pages/memoryArticleModel";
import { styles } from "@/pages/memoryPageStyles";
import { MemoryUpdateControl } from "@/pages/MemoryUpdateControl";

interface MemoryArticlePage {
  id: string;
  title: string;
  body: string;
  citations: Array<{ sources: Array<{ source: string; kind: GraphqlNativeMemorySourceKind; excerpt: string | null; createdAt: string | null }> }>;
  children: Array<{ id: string; path: string; title: string; excerpt: string }>;
}

const baseArticleComponents: MarkdownComponents = {
  heading: ArticleHeading,
  paragraph: ArticleParagraph
};

export function MemoryArticle({ page }: { page: MemoryArticlePage }) {
  const article = buildMemoryArticle(page.body, page.citations);
  const [sourcesOpen, setSourcesOpen] = useState(false);
  const citationContent = article.content && article.citationGroups.length > 0
    ? citationContentWithSourcesAction(article.content, article.sources)
    : { text: article.content, sources: article.sources };
  const articleComponents: MarkdownComponents = {
    ...baseArticleComponents,
    citation: (props) => <CitationReference {...props} onOpenSources={() => setSourcesOpen(true)} />
  };
  const hasContents = article.outline.length > 0 || page.children.length > 0;
  return (
    <ShellPageLayout width="centered">
      <article {...stylex.props(styles.article)}>
        <ShellSectionHeader
          title={page.title}
          titleFont="serif"
        />

        <ShellPageTrack>
          <div {...stylex.props(styles.articleContent)}>
            <ShellPageSubtitle>From Noema, the private memory encyclopedia</ShellPageSubtitle>
            <MemoryUpdateControl />

            {hasContents ? (
              <nav {...stylex.props(styles.contentsBox)} aria-label="Article contents">
                <strong {...stylex.props(styles.contentsTitle)}>Contents</strong>
                <VStack as="ol" gap={1} {...stylex.props(styles.contentsList)}>
                  {article.outline.map((item) => (
                    <li key={item.id} {...stylex.props(item.level > 2 && styles.nestedContentsItem)}>
                      <a href={`#${item.id}`} {...stylex.props(styles.articleLink)}>{item.label}</a>
                    </li>
                  ))}
                  {page.children.length > 0 ? (
                    <li><a href="#related-articles" {...stylex.props(styles.articleLink)}>Related Articles</a></li>
                  ) : null}
                </VStack>
              </nav>
            ) : null}

            {citationContent.text ? (
              <MarkdownContent
                citationStyle="number"
                components={articleComponents}
                density="default"
                headingLevelStart={1}
                sources={citationContent.sources}
                xstyle={styles.articleBody}
              >
                {citationContent.text}
              </MarkdownContent>
            ) : (
              <p {...stylex.props(styles.stub)}>This biographical article is a stub. It will expand once the first durable facts are recorded.</p>
            )}

            {page.children.length > 0 ? (
              <section id="related-articles" {...stylex.props(styles.relatedArticlesSection)}>
                <h2 {...stylex.props(styles.articleHeading, styles.articleHeadingMajor)}>Related Articles</h2>
                <ul {...stylex.props(styles.relatedArticleList)}>
                  {page.children.map((child) => (
                    <li key={child.id} {...stylex.props(styles.relatedArticleEntry)}>
                      <Link
                        data-slot="memory-related-article"
                        to="/memory/$"
                        params={{ _splat: memoryPageUrlPath(child.path) }}
                        {...stylex.props(styles.relatedArticleCardLink)}
                      >
                        <Item
                          align="start"
                          density="balanced"
                          description={child.excerpt || "Focused memory article"}
                          descriptionLines={2}
                          endContent={<ArrowRight aria-hidden="true" size={14} strokeWidth={2} />}
                          label={child.title}
                          labelLines={1}
                          xstyle={styles.relatedArticleCard}
                        />
                      </Link>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}
          </div>
        </ShellPageTrack>
      </article>
      <MemoryCitationDialog
        citations={article.citationGroups}
        open={sourcesOpen}
        onOpenChange={setSourcesOpen}
      />
    </ShellPageLayout>
  );
}

function MemoryCitationDialog({
  citations,
  open,
  onOpenChange
}: {
  citations: readonly MemoryCitationGroup[];
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
            <VStack gap={3}>
              {citations.map((citation, citationIndex) => (
                <VStack key={citationIndex} gap={2}>
                  <Text type="supporting" color="accent" weight="semibold">
                    Citation {citationIndex + 1}
                  </Text>
                  {citation.sources.map((source) => (
                    <VStack key={source.source} gap={1} xstyle={styles.citationSource}>
                      <Text type="body" xstyle={styles.citationExcerpt}>
                        &ldquo;{source.excerpt ?? "This source is no longer available."}&rdquo;
                      </Text>
                      <Text type="supporting" color="secondary" xstyle={styles.citationContext}>
                        {evidenceKindLabel(source.kind)}
                        {source.createdAt ? ` · ${formatEvidenceDate(source.createdAt)}` : ""}
                      </Text>
                      <Text type="supporting" color="secondary" xstyle={styles.citationReference}>
                        {source.source}
                      </Text>
                    </VStack>
                  ))}
                  {citationIndex + 1 < citations.length ? <Divider /> : null}
                </VStack>
              ))}
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

function evidenceKindLabel(kind: GraphqlNativeMemorySourceKind): string {
  switch (kind) {
    case "HUMAN_MESSAGE": return "Human message";
    case "TOOL_RESULT": return "Tool result";
    case "UNAVAILABLE": return "Unavailable source";
  }
}

function formatEvidenceDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
}

function ArticleHeading({ level, children }: { level: 1 | 2 | 3 | 4 | 5 | 6; children: ReactNode }) {
  const label = textFromChildren(children);
  const props = stylex.props(styles.articleHeading, level <= 2 ? styles.articleHeadingMajor : styles.articleHeadingMinor);
  if (level <= 2) return <h2 id={memoryHeadingId(label)} {...props}>{children}</h2>;
  return <h3 id={memoryHeadingId(label)} {...props}>{children}</h3>;
}

function ArticleParagraph({ children }: { children: ReactNode }) {
  return <p {...stylex.props(styles.articleParagraph)}>{children}</p>;
}

function textFromChildren(children: ReactNode): string {
  if (typeof children === "string" || typeof children === "number") return String(children);
  if (Array.isArray(children)) return children.map(textFromChildren).join("");
  if (children && typeof children === "object" && "props" in children) {
    return textFromChildren((children as { props: { children?: ReactNode } }).props.children);
  }
  return "";
}
