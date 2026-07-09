import { useMutation, useQuery } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";
import {
  MemoryGraphDocument as MemoryGraphQueryDocument,
  RegenerateMemoryArticleDocument,
  type MemoryGraphQuery,
  type MemoryGraphQueryVariables,
  type RegenerateMemoryArticleMutation,
  type RegenerateMemoryArticleMutationVariables
} from "@/generated/graphql";
import {
  buildMemoryArticleModel,
  type MemoryArticleModel,
  type MemoryArticleReference
} from "@/pages/memoryPageModel";
import { styles } from "@/pages/memoryPageStyles";

const PAGE_SIZE = 25;

export function MemoryPage() {
  const [loadingMore, setLoadingMore] = useState(false);
  const { data, error, fetchMore, loading, refetch } = useQuery<
    MemoryGraphQuery,
    MemoryGraphQueryVariables
  >(MemoryGraphQueryDocument, {
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true,
    variables: { page: 1, limit: PAGE_SIZE }
  });
  const graph = data?.memoryGraph;
  const article = buildMemoryArticleModel(graph);
  const serviceError =
    error ??
    (graph?.status.status && graph.status.status !== "READY"
      ? new Error(graph.status.lastErrorMessage ?? "Memory service is unavailable.")
      : null);
  const [regenerateMemoryArticle, { loading: regeneratingArticle }] = useMutation<
    RegenerateMemoryArticleMutation,
    RegenerateMemoryArticleMutationVariables
  >(RegenerateMemoryArticleDocument);

  const loadMore = async () => {
    if (!graph?.pageInfo.hasMore || loadingMore) {
      return;
    }
    setLoadingMore(true);
    try {
      await fetchMore({
        variables: {
          page: graph.pageInfo.page + 1,
          limit: PAGE_SIZE
        },
        updateQuery: (previous, { fetchMoreResult }) => {
          if (!fetchMoreResult) {
            return previous;
          }
          return {
            memoryGraph: {
              ...fetchMoreResult.memoryGraph,
              documents: [
                ...previous.memoryGraph.documents,
                ...fetchMoreResult.memoryGraph.documents
              ]
            }
          };
        }
      });
    } finally {
      setLoadingMore(false);
    }
  };

  return (
    <section
      data-slot="memory-surface"
      {...stylex.props(styles.surface)}
      aria-labelledby="memory-surface-title"
    >
      <div {...stylex.props(styles.wikiShell)}>
        <div {...stylex.props(styles.pageShell)}>
          <article {...stylex.props(styles.page)}>
            <h1 id="memory-surface-title" {...stylex.props(styles.articleTitle)}>
              {article.title}
            </h1>
            <div {...stylex.props(styles.subtitle)}>{article.subtitle}</div>

            <MemoryInfobox
              article={article}
              regeneratingArticle={regeneratingArticle}
              onRegenerate={() => {
                void regenerateMemoryArticle().then(() => refetch());
              }}
            />

            {serviceError ? (
              <div role="status" {...stylex.props(styles.statusBlock, styles.errorBlock)}>
                Error loading memory: {serviceError.message}
              </div>
            ) : null}

            {loading && article.totalMemories === 0 ? (
              <div role="status" {...stylex.props(styles.statusBlock)}>
                Loading memory...
              </div>
            ) : null}

            {article.leadParagraphs.map((paragraph) => (
              <p key={paragraph} {...stylex.props(styles.lead)}>
                {paragraph}
              </p>
            ))}

            {article.isStub && !serviceError && !(loading && article.totalMemories === 0) ? (
              <div role="note" {...stylex.props(styles.stubNote)}>
                <span {...stylex.props(styles.stubLabel)}>Stub</span>
                <span>{article.stubText}</span>
              </div>
            ) : null}

            <nav {...stylex.props(styles.contents)} aria-label="Memory article contents">
              <div {...stylex.props(styles.contentsTitle)}>Contents</div>
              <ol {...stylex.props(styles.contentsList)}>
                {article.sections.map((section) => (
                  <li key={section.id}>
                    <a {...stylex.props(styles.link)} href={`#${section.id}`}>
                      {section.title}
                    </a>
                  </li>
                ))}
                <li>
                  <a {...stylex.props(styles.link)} href="#references">
                    References
                  </a>
                </li>
              </ol>
            </nav>

            {article.sections.map((section) => (
              <section key={section.id} id={section.id} {...stylex.props(styles.articleSection)}>
                <h2 {...stylex.props(styles.sectionTitle)}>{section.title}</h2>
                {section.paragraphs.length > 0 ? (
                  section.paragraphs.map((paragraph) => (
                    <p key={paragraph} {...stylex.props(styles.bodyText)}>
                      {paragraph}
                    </p>
                  ))
                ) : (
                  <p {...stylex.props(styles.bodyText)}>
                    No durable memories have been returned by Mnemosyne yet.
                  </p>
                )}
              </section>
            ))}

            <section id="references" {...stylex.props(styles.articleSection)}>
              <h2 {...stylex.props(styles.sectionTitle)}>References</h2>
              <ol {...stylex.props(styles.references)}>
                {article.references.map((reference) => (
                  <ReferenceItem key={reference.id} reference={reference} />
                ))}
              </ol>
            </section>

            {graph?.pageInfo.hasMore ? (
              <button
                type="button"
                {...stylex.props(styles.loadMore)}
                disabled={loadingMore}
                onClick={() => void loadMore()}
              >
                {loadingMore ? "Loading..." : "Load more memories"}
              </button>
            ) : null}
          </article>
        </div>
      </div>
    </section>
  );
}

function MemoryInfobox({
  article,
  regeneratingArticle,
  onRegenerate
}: {
  article: MemoryArticleModel;
  regeneratingArticle: boolean;
  onRegenerate: () => void;
}) {
  return (
    <aside {...stylex.props(styles.infobox)}>
      <table {...stylex.props(styles.infoTable)}>
        <tbody>
          <tr>
            <th colSpan={2} {...stylex.props(styles.boxTitle)}>
              {article.title}
            </th>
          </tr>
          <tr>
            <td colSpan={2} {...stylex.props(styles.portraitCell)}>
              <div {...stylex.props(styles.portrait)}>{article.initials}</div>
              <div {...stylex.props(styles.portraitCaption)}>{article.portraitCaption}</div>
            </td>
          </tr>
          {article.infoboxRows.map((row) => (
            <InfoRow key={row.label} label={row.label} value={row.value} />
          ))}
        </tbody>
      </table>
      <div {...stylex.props(styles.sidebox)}>
        <strong>References</strong>
        <div {...stylex.props(styles.actionLinks)}>
          <span>{article.referenceCountLabel}</span>
          <span>Private local record</span>
        </div>
      </div>
      <div {...stylex.props(styles.sidebox)}>
        <strong>Article tools</strong>
        <button
          type="button"
          {...stylex.props(styles.regenerateButton)}
          disabled={regeneratingArticle}
          onClick={onRegenerate}
        >
          {regeneratingArticle ? "Regenerating..." : "Regenerate article"}
        </button>
      </div>
    </aside>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <tr>
      <th {...stylex.props(styles.boxKey)}>{label}</th>
      <td {...stylex.props(styles.boxValue)}>{value}</td>
    </tr>
  );
}

function ReferenceItem({ reference }: { reference: MemoryArticleReference }) {
  return (
    <li {...stylex.props(styles.referenceItem)}>
      <div {...stylex.props(styles.referenceLabel)}>{reference.label}</div>
      {reference.sourceMessage ? (
        <blockquote {...stylex.props(styles.referenceQuote)}>{reference.sourceMessage}</blockquote>
      ) : null}
      {reference.sourceMeta.length > 0 ? (
        <div {...stylex.props(styles.referenceMeta)}>{reference.sourceMeta.join(" · ")}</div>
      ) : null}
      {reference.citedFacts.length > 0 ? (
        <ul {...stylex.props(styles.citedFacts)}>
          {reference.citedFacts.map((fact) => (
            <li key={fact.id}>{fact.displayText}</li>
          ))}
        </ul>
      ) : null}
    </li>
  );
}
