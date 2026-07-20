import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import { HoverCard } from "@astryxdesign/core/HoverCard";
import { Item, type ItemProps } from "@astryxdesign/core/Item";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { ArrowRight } from "lucide-react";
import type { ReactNode } from "react";
import { memoryPageUrlPath } from "@/app/routes";
import { buildMemoryArticle, memoryHeadingId } from "@/pages/memoryArticleModel";
import { styles } from "@/pages/memoryPageStyles";

type MarkdownComponents = NonNullable<MarkdownProps["components"]>;
type ItemXStyle = ItemProps["xstyle"];

interface MemoryArticlePage {
  id: string;
  title: string;
  body: string;
  sourceReferences: Array<{ source: string; excerpt: string | null }>;
  children: Array<{ id: string; path: string; title: string; excerpt: string }>;
}

const articleComponents: MarkdownComponents = {
  citation: ArticleCitation,
  heading: ArticleHeading,
  paragraph: ArticleParagraph
};

export function MemoryArticle({ page }: { page: MemoryArticlePage }) {
  const article = buildMemoryArticle(page.body, page.sourceReferences);
  const hasContents = article.outline.length > 0 || page.children.length > 0;
  return (
    <article {...stylex.props(styles.article)}>
      <header>
        <h1 {...stylex.props(styles.articleTitle)}>{page.title}</h1>
        <div {...stylex.props(styles.articleSubtitle)}>From Noema, the private memory encyclopedia</div>
      </header>

      {hasContents ? (
        <nav {...stylex.props(styles.contentsBox)} aria-label="Article contents">
          <strong {...stylex.props(styles.contentsTitle)}>Contents</strong>
          <ol {...stylex.props(styles.contentsList)}>
            {article.outline.map((item) => (
              <li key={item.id} {...stylex.props(item.level > 2 && styles.nestedContentsItem)}>
                <a href={`#${item.id}`} {...stylex.props(styles.articleLink)}>{item.label}</a>
              </li>
            ))}
            {page.children.length > 0 ? (
              <li><a href="#related-articles" {...stylex.props(styles.articleLink)}>Related Articles</a></li>
            ) : null}
          </ol>
        </nav>
      ) : null}

      {article.content ? (
        <Markdown
          autolink="gfm"
          citationStyle="number"
          components={articleComponents}
          contentWidth="100%"
          density="default"
          headingLevelStart={1}
          sources={article.sources}
          xstyle={markdownXStyle(styles.articleBody)}
        >
          {article.content}
        </Markdown>
      ) : (
        <p {...stylex.props(styles.stub)}>This memory article has not developed a lead yet.</p>
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
                    xstyle={itemXStyle(styles.relatedArticleCard)}
                  />
                </Link>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </article>
  );
}

function ArticleCitation({ source, number }: { source: { title?: string }; number: number; variant: "label" | "number" }) {
  const excerpt = source.title ?? "The source conversation message is no longer available.";
  return (
    <sup {...stylex.props(styles.citation)}>
      <HoverCard
        content={(
          <span {...stylex.props(styles.citationCard)}>
            <span {...stylex.props(styles.citationExcerpt)}>&ldquo;{excerpt}&rdquo;</span>
            <span {...stylex.props(styles.citationContext)}>Your message in the primary conversation</span>
          </span>
        )}
        placement="above"
        alignment="start"
        delay={120}
        hasHoverIndication={false}
      >
        <button type="button" aria-label={`Show source ${number}`} {...stylex.props(styles.citationTrigger)}>[{number}]</button>
      </HoverCard>
    </sup>
  );
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

function markdownXStyle(...xstyle: unknown[]): MarkdownProps["xstyle"] {
  return xstyle as unknown as MarkdownProps["xstyle"];
}

function itemXStyle(...xstyle: unknown[]): ItemXStyle {
  return xstyle as unknown as ItemXStyle;
}
