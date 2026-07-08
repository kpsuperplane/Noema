import { useQuery } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";
import {
  MemoryGraphDocument as MemoryGraphQueryDocument,
  type MemoryGraphQuery,
  type MemoryGraphQueryVariables
} from "@/generated/graphql";
import {
  buildMemoryArticleModel,
  formatCount,
  formatDateTime,
  type MemoryArticleEntry,
  type MemoryArticleModel
} from "@/pages/memoryPageModel";
import { styles } from "@/pages/memoryPageStyles";

const PAGE_SIZE = 25;

export function MemoryPage() {
  const [loadingMore, setLoadingMore] = useState(false);
  const { data, error, fetchMore, loading } = useQuery<
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
        <nav {...stylex.props(styles.tabs)} aria-label="Memory article views">
          <span {...stylex.props(styles.tabActive)}>Article</span>
        </nav>
        <div {...stylex.props(styles.pageShell)}>
          <article {...stylex.props(styles.page)}>
            <MemoryFigure article={article} />

            <h1 id="memory-surface-title" {...stylex.props(styles.articleTitle)}>
              {article.title}
            </h1>
            <div {...stylex.props(styles.subtitle)}>{article.subtitle}</div>

            <MemoryInfobox article={article} />

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

            <p {...stylex.props(styles.lead)}>{article.leadText}</p>

            <nav {...stylex.props(styles.contents)} aria-label="Memory article contents">
              <div {...stylex.props(styles.contentsTitle)}>Contents</div>
              <ol {...stylex.props(styles.contentsList)}>
                <li>
                  <a {...stylex.props(styles.link)} href="#remembered-facts">
                    Remembered facts
                  </a>
                </li>
                <li>
                  <a {...stylex.props(styles.link)} href="#source-observations">
                    Source observations
                  </a>
                </li>
                <li>
                  <a {...stylex.props(styles.link)} href="#references">
                    References
                  </a>
                </li>
              </ol>
            </nav>

            <p {...stylex.props(styles.bodyText)}>
              The figure above summarizes the currently loaded memory record.
              The sections below list extracted facts first, then the source
              observations Mnemosyne used to form them.
            </p>

            {article.sections.map((section) => (
              <section key={section.id} id={section.id} {...stylex.props(styles.articleSection)}>
                <h2 {...stylex.props(styles.sectionTitle)}>{section.title}</h2>
                {section.entries.length > 0 ? (
                  <ul {...stylex.props(styles.entryList)}>
                    {section.entries.map((entry) => (
                      <MemoryEntryItem key={entry.id} entry={entry} />
                    ))}
                  </ul>
                ) : (
                  <p {...stylex.props(styles.bodyText)}>
                    No durable memories have been returned by Mnemosyne yet.
                  </p>
                )}
              </section>
            ))}

            <section id="source-observations" {...stylex.props(styles.articleSection)}>
              <h2 {...stylex.props(styles.sectionTitle)}>Source observations</h2>
              {article.sourceObservations.length > 0 ? (
                <ol {...stylex.props(styles.sourceList)}>
                  {article.sourceObservations.map((entry) => (
                    <li key={`source:${entry.id}`} {...stylex.props(styles.sourceItem)}>
                      <blockquote {...stylex.props(styles.sourceQuote)}>
                        {entry.sourceObservation}
                      </blockquote>
                      <div {...stylex.props(styles.entryMeta)}>
                        <span>{entry.sourceTitle}</span>
                        {entry.updatedAt ?? entry.createdAt ? (
                          <>
                            <span aria-hidden="true">·</span>
                            <time dateTime={(entry.updatedAt ?? entry.createdAt) as string}>
                              {formatDateTime((entry.updatedAt ?? entry.createdAt) as string)}
                            </time>
                          </>
                        ) : null}
                      </div>
                    </li>
                  ))}
                </ol>
              ) : (
                <p {...stylex.props(styles.bodyText)}>
                  Mnemosyne has not returned source observations for the loaded
                  facts yet.
                </p>
              )}
            </section>

            <section id="references" {...stylex.props(styles.articleSection)}>
              <h2 {...stylex.props(styles.sectionTitle)}>References</h2>
              <ol {...stylex.props(styles.references)}>
                {article.references.map((reference) => (
                  <li key={reference}>{reference}</li>
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

function MemoryFigure({ article }: { article: MemoryArticleModel }) {
  return (
    <figure {...stylex.props(styles.figure)}>
      <div {...stylex.props(styles.memoryPlate)}>
        <div {...stylex.props(styles.plateMain)}>
          <div>
            <div {...stylex.props(styles.plateLabel)}>Memory cluster diagram</div>
            <div {...stylex.props(styles.plateTitle)}>{article.figureTitle}</div>
            <p {...stylex.props(styles.plateCopy)}>{article.figureCopy}</p>
          </div>
          <div {...stylex.props(styles.legend)}>
            <span {...stylex.props(styles.legendItem)}>
              <i {...stylex.props(styles.swatch, styles.swatchStable)} />
              Stable roots
            </span>
            <span {...stylex.props(styles.legendItem)}>
              <i {...stylex.props(styles.swatch, styles.swatchRecent)} />
              Recent growth
            </span>
            <span {...stylex.props(styles.legendItem)}>
              <i {...stylex.props(styles.swatch, styles.swatchInterest)} />
              Interests
            </span>
            <span {...stylex.props(styles.legendItem)}>
              <i {...stylex.props(styles.swatch, styles.swatchQuestion)} />
              Open questions
            </span>
          </div>
        </div>

        <table {...stylex.props(styles.clusterTable)}>
          <thead>
            <tr>
              <th {...stylex.props(styles.clusterHeader)}>Cluster</th>
              <th {...stylex.props(styles.clusterHeader)}>Strength</th>
              <th {...stylex.props(styles.clusterHeader)}>Status</th>
            </tr>
          </thead>
          <tbody>
            {article.clusters.map((cluster) => (
              <tr key={cluster.label}>
                <td {...stylex.props(styles.clusterCell)}>{cluster.label}</td>
                <td {...stylex.props(styles.clusterCell)}>
                  <div {...stylex.props(styles.bar)}>
                    <span
                      {...stylex.props(styles.barFill)}
                      style={{ width: `${cluster.strength}%` }}
                    />
                  </div>
                </td>
                <td {...stylex.props(styles.clusterCell)}>{cluster.status}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <figcaption {...stylex.props(styles.caption)}>{article.figureCaption}</figcaption>
    </figure>
  );
}

function MemoryInfobox({ article }: { article: MemoryArticleModel }) {
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
            <td colSpan={2} {...stylex.props(styles.discCell)}>
              <div {...stylex.props(styles.clusterDisc)} />
              <div {...stylex.props(styles.discLabel)}>Topic distribution</div>
            </td>
          </tr>
          <InfoRow label="Primary pattern" value={article.primaryPattern} />
          <InfoRow label="Known memories" value={article.totalLabel} />
          <InfoRow label="Stable roots" value="Metadata pending" />
          <InfoRow label="Fresh shoots" value="Metadata pending" />
          <InfoRow label="Recurring motif" value={article.recurringMotif} />
          <InfoRow label="Last updated" value={article.lastUpdatedLabel} />
        </tbody>
      </table>
      <div {...stylex.props(styles.sidebox)}>
        <strong>Sources</strong>
        <div {...stylex.props(styles.actionLinks)}>
          <span>{formatCount(article.sourceObservations.length, "source observation", "source observations")}</span>
          <span>Mnemosyne fact annotations</span>
        </div>
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

function MemoryEntryItem({ entry }: { entry: MemoryArticleEntry }) {
  const timestamp = entry.updatedAt ?? entry.createdAt;
  return (
    <li {...stylex.props(styles.entryItem)}>
      <p {...stylex.props(styles.entryText)}>{entry.text}</p>
      <div {...stylex.props(styles.entryMeta)}>
        <span>{entry.sourceTitle}</span>
        {timestamp ? (
          <>
            <span aria-hidden="true">·</span>
            <time dateTime={timestamp}>{formatDateTime(timestamp)}</time>
          </>
        ) : null}
      </div>
    </li>
  );
}
