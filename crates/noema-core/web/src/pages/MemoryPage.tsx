import { MemoryGraph as SupermemoryGraph } from "@supermemory/memory-graph";
import type {
  GraphApiDocument,
  GraphApiMemory,
  GraphThemeColors,
  MemoryRelation
} from "@supermemory/memory-graph";
import { useQuery } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useState } from "react";
import {
  MemoryGraphDocument as MemoryGraphQueryDocument,
  type MemoryGraphQuery,
  type MemoryGraphQueryVariables
} from "@/generated/graphql";

const PAGE_SIZE = 25;
const HUMAN_SPACE_ID = "human:local";
const LIGHT_GRAPH_COLORS = {
  bg: "#fbfaf7",
  docFill: "#ffffff",
  docStroke: "#d7d1c8",
  docInnerFill: "#f5f1ea",
  memFill: "#eef6f3",
  memFillHover: "#e0eee9",
  memStrokeDefault: "#2d7f73",
  accent: "#2d7f73",
  textPrimary: "#171612",
  textSecondary: "#4c4941",
  textMuted: "#817b70",
  edgeDerives: "#b66d12",
  edgeUpdates: "#7763c4",
  edgeExtends: "#7a8a93",
  memBorderForgotten: "#b42318",
  memBorderExpiring: "#c27a12",
  memBorderRecent: "#20815f",
  glowColor: "#5a9c8f",
  iconColor: "#2d7f73",
  popoverBg: "#ffffff",
  popoverBorder: "#d7d1c8",
  popoverTextPrimary: "#171612",
  popoverTextSecondary: "#4c4941",
  popoverTextMuted: "#817b70",
  controlBg: "#ffffff",
  controlBorder: "#d7d1c8"
} satisfies Partial<GraphThemeColors>;

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
  const documents = useMemo(
    () => toGraphDocuments(graph?.documents ?? []),
    [graph?.documents]
  );
  const statusError = useMemo(() => {
    if (error) {
      return error;
    }
    if (graph?.status.status && graph.status.status !== "READY") {
      return new Error(graph.status.lastErrorMessage ?? "Supermemory is unavailable.");
    }
    return null;
  }, [error, graph]);

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
      </div>
      <div {...stylex.props(styles.graphFrame)}>
        <SupermemoryGraph
          colors={LIGHT_GRAPH_COLORS}
          documents={documents}
          error={statusError}
          hasMore={graph?.pageInfo.hasMore ?? false}
          isLoading={loading && documents.length === 0}
          isLoadingMore={loadingMore}
          maxNodes={400}
          onLoadMore={() => {
            void loadMore();
          }}
          totalCount={graph?.pageInfo.total ?? undefined}
          variant="console"
        >
          <div {...stylex.props(styles.emptyState)}>No human memories yet.</div>
        </SupermemoryGraph>
      </div>
    </section>
  );
}

type GraphqlMemoryGraphDocument = MemoryGraphQuery["memoryGraph"]["documents"][number];
type GraphqlMemoryGraphEntry = GraphqlMemoryGraphDocument["memoryEntries"][number];

function toGraphDocuments(documents: GraphqlMemoryGraphDocument[]): GraphApiDocument[] {
  return documents.map((document) => ({
    id: document.id,
    title: document.title ?? null,
    summary: document.summary ?? null,
    documentType: document.type ?? "document",
    createdAt: document.createdAt,
    updatedAt: document.updatedAt,
    memories: document.memoryEntries.map((entry) => toGraphMemory(entry))
  }));
}

function toGraphMemory(entry: GraphqlMemoryGraphEntry): GraphApiMemory {
  const text = entry.content ?? entry.summary ?? entry.title ?? "";
  return {
    id: entry.id,
    memory: text,
    content: text,
    isStatic: false,
    spaceId: entry.spaceId ?? HUMAN_SPACE_ID,
    isLatest: entry.isLatest ?? true,
    isForgotten: false,
    forgetAfter: null,
    forgetReason: null,
    version: 1,
    parentMemoryId: null,
    rootMemoryId: null,
    createdAt: entry.createdAt,
    updatedAt: entry.updatedAt,
    relation: relationFromValue(entry.relation),
    spaceContainerTag: entry.spaceContainerTag ?? HUMAN_SPACE_ID
  };
}

function relationFromValue(value: string | null | undefined): MemoryRelation | null {
  if (value === "updates" || value === "extends" || value === "derives") {
    return value;
  }
  return null;
}

const styles = stylex.create({
  surface: {
    boxSizing: "border-box",
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    gap: 16,
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    padding: "24px 24px",
    "@media (max-width: 760px)": {
      paddingInline: 20
    }
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
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
  graphFrame: {
    minHeight: 420,
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 8,
    backgroundColor: "#fbfaf7"
  },
  emptyState: {
    display: "grid",
    placeItems: "center",
    minHeight: 220,
    color: "var(--muted-foreground)",
    fontSize: 14,
    lineHeight: 1.4
  }
});
