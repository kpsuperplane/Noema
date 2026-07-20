import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import { buildMemoryArticle, memoryHeadingId } from "@/pages/memoryArticleModel";
import { styles } from "@/pages/memoryPageStyles";

type MarkdownComponents = NonNullable<MarkdownProps["components"]>;

interface MemoryArticlePage {
  title: string;
  path: string;
  body: string;
  sources: string[];
}

const articleComponents: MarkdownComponents = {
  heading: ArticleHeading,
  paragraph: ArticleParagraph
};

export function MemoryArticle({ page }: { page: MemoryArticlePage }) {
  const article = buildMemoryArticle(page.body, page.sources);
  return (
    <article {...stylex.props(styles.article)}>
      <header>
        <h1 {...stylex.props(styles.articleTitle)}>{page.title}</h1>
        <div {...stylex.props(styles.articleSubtitle)}>From Noema, the private memory encyclopedia · {page.path}</div>
      </header>

      {article.outline.length > 0 ? (
        <nav {...stylex.props(styles.contentsBox)} aria-label="Article contents">
          <strong {...stylex.props(styles.contentsTitle)}>Contents</strong>
          <ol {...stylex.props(styles.contentsList)}>
            {article.outline.map((item) => (
              <li key={item.id} {...stylex.props(item.level > 2 && styles.nestedContentsItem)}>
                <a href={`#${item.id}`} {...stylex.props(styles.articleLink)}>{item.label}</a>
              </li>
            ))}
            {article.references.length > 0 ? (
              <li><a href="#references" {...stylex.props(styles.articleLink)}>References</a></li>
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

      {article.references.length > 0 ? (
        <section id="references" {...stylex.props(styles.referencesSection)}>
          <h2 {...stylex.props(styles.articleHeading, styles.articleHeadingMajor)}>References</h2>
          <ol {...stylex.props(styles.referenceList)}>
            {article.references.map((reference) => (
              <li key={reference.source} {...stylex.props(styles.referenceItem)}>
                <span {...stylex.props(styles.referenceNumber)}>{reference.number}.</span>
                <span>Local conversation item <code {...stylex.props(styles.referenceCode)}>{reference.source}</code></span>
              </li>
            ))}
          </ol>
        </section>
      ) : null}
    </article>
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
