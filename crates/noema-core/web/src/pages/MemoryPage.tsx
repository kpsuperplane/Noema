import { useQuery } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";
import {
  MemoryGraphDocument as MemoryGraphQueryDocument,
  type MemoryGraphQuery,
  type MemoryGraphQueryVariables
} from "@/generated/graphql";

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
  const documents = graph?.documents ?? [];
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
      <div {...stylex.props(styles.header)}>
        <h1 id="memory-surface-title" {...stylex.props(styles.title)}>
          Memory
        </h1>
        {graph?.pageInfo.total != null ? (
          <span {...stylex.props(styles.count)}>{graph.pageInfo.total} memories</span>
        ) : null}
      </div>

      {serviceError ? (
        <div role="status" {...stylex.props(styles.status)}>
          Error loading memory: {serviceError.message}
        </div>
      ) : null}

      {loading && documents.length === 0 ? (
        <div role="status" {...stylex.props(styles.status)}>
          Loading memory...
        </div>
      ) : null}

      {!loading && !serviceError && documents.length === 0 ? (
        <div {...stylex.props(styles.emptyState)}>No human memories yet.</div>
      ) : null}

      {documents.length > 0 ? (
        <div data-slot="memory-list" {...stylex.props(styles.list)}>
          {documents.map((document) => (
            <section key={document.id} {...stylex.props(styles.group)}>
              <div {...stylex.props(styles.groupHeader)}>
                <h2 {...stylex.props(styles.groupTitle)}>{document.title ?? "Memory source"}</h2>
                <span {...stylex.props(styles.groupMeta)}>
                  {document.memoryEntries.length} entries
                </span>
              </div>
              <ul {...stylex.props(styles.entries)}>
                {document.memoryEntries.map((entry) => (
                  <li key={entry.id} {...stylex.props(styles.entry)}>
                    <p {...stylex.props(styles.entryText)}>
                      {entry.content ?? entry.summary ?? entry.title ?? "Untitled memory"}
                    </p>
                    {entry.updatedAt ? (
                      <time {...stylex.props(styles.entryMeta)} dateTime={entry.updatedAt}>
                        {entry.updatedAt}
                      </time>
                    ) : null}
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      ) : null}

      {graph?.pageInfo.hasMore ? (
        <button
          type="button"
          {...stylex.props(styles.loadMore)}
          disabled={loadingMore}
          onClick={() => void loadMore()}
        >
          {loadingMore ? "Loading..." : "Load more"}
        </button>
      ) : null}
    </section>
  );
}

const styles = stylex.create({
  surface: {
    boxSizing: "border-box",
    display: "grid",
    alignContent: "start",
    gap: 16,
    height: "100%",
    minHeight: 0,
    overflow: "auto",
    padding: "24px 24px",
    "@media (max-width: 760px)": {
      paddingInline: 20
    }
  },
  header: {
    display: "flex",
    alignItems: "baseline",
    justifyContent: "space-between",
    gap: 12,
    minWidth: 0
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 24,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  count: {
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.4
  },
  status: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: 12,
    color: "var(--muted-foreground)",
    fontSize: 14,
    lineHeight: 1.4
  },
  emptyState: {
    display: "grid",
    placeItems: "center",
    minHeight: 220,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    color: "var(--muted-foreground)",
    fontSize: 14,
    lineHeight: 1.4
  },
  list: {
    display: "grid",
    gap: 14
  },
  group: {
    display: "grid",
    gap: 10,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: 14
  },
  groupHeader: {
    display: "flex",
    alignItems: "baseline",
    justifyContent: "space-between",
    gap: 12
  },
  groupTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  groupMeta: {
    color: "var(--muted-foreground)",
    fontSize: 12,
    lineHeight: 1.4
  },
  entries: {
    display: "grid",
    gap: 8,
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  entry: {
    display: "grid",
    gap: 4,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 8
  },
  entryText: {
    margin: 0,
    color: "var(--foreground)",
    fontSize: 14,
    lineHeight: 1.45
  },
  entryMeta: {
    color: "var(--muted-foreground)",
    fontSize: 12,
    lineHeight: 1.35
  },
  loadMore: {
    justifySelf: "start",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: "7px 10px",
    color: "var(--foreground)",
    fontSize: 13,
    fontWeight: 600,
    lineHeight: 1.3,
    cursor: "pointer",
    ":disabled": {
      cursor: "not-allowed",
      opacity: 0.6
    }
  }
});
