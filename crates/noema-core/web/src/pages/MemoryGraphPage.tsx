import React from "react";
import { useQuery } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";

import { MemoryGraphCanvas } from "@/components/memory/MemoryGraphCanvas";
import { MemoryGraphControls } from "@/components/memory/MemoryGraphControls";
import { MemoryGraphDetailPanel } from "@/components/memory/MemoryGraphDetailPanel";
import { MemoryGraphDocument } from "@/generated/graphql";
import { graphStatusDefaults, normalizeMemoryGraph } from "@/memoryGraph";
import { layoutMemoryGraph } from "@/memoryGraphLayout";

export function MemoryGraphPage() {
  const [query, setQuery] = React.useState("");
  const [statuses, setStatuses] = React.useState(() => graphStatusDefaults());
  const [selectedClaimId, setSelectedClaimId] = React.useState<string | null>(null);
  const defaultStatuses = React.useMemo(() => graphStatusDefaults(), []);

  const variables = React.useMemo(
    () => ({
      input: {
        query: query.trim() || null,
        statuses,
        limit: 150,
      },
    }),
    [query, statuses],
  );

  const result = useQuery(MemoryGraphDocument, {
    variables,
    fetchPolicy: "cache-and-network",
  });
  const graphData = result.data?.memoryGraph ?? null;

  const graph = React.useMemo(
    () => (graphData ? normalizeMemoryGraph(graphData) : null),
    [graphData],
  );

  const layout = React.useMemo(
    () => (graph ? layoutMemoryGraph(graph, { width: 920, height: 620 }) : { nodes: [], edges: [] }),
    [graph],
  );

  const selectedEdge = React.useMemo(
    () => graph?.edges.find((edge) => edge.claimId === selectedClaimId) ?? null,
    [graph?.edges, selectedClaimId],
  );

  function handleStatusesChange(nextStatuses: string[]) {
    setSelectedClaimId(null);
    setStatuses(nextStatuses.length > 0 ? nextStatuses : graphStatusDefaults());
  }

  const hasDefaultStatuses =
    statuses.length === defaultStatuses.length && statuses.every((status) => defaultStatuses.includes(status));
  const hasActiveFilters = query.trim().length > 0 || !hasDefaultStatuses;

  if (result.loading && !graph) {
    return (
      <section {...stylex.props(styles.centerState)} aria-label="Memory Graph">
        Loading memory graph...
      </section>
    );
  }

  if (result.error) {
    return (
      <section {...stylex.props(styles.messageState)} aria-label="Memory Graph">
        <h1 {...stylex.props(styles.title)}>
          Memory graph unavailable
        </h1>
        <p {...stylex.props(styles.messageText)}>{result.error.message}</p>
      </section>
    );
  }

  if (!graph) {
    return (
      <section {...stylex.props(styles.messageState)} aria-label="Memory Graph">
        <h1 {...stylex.props(styles.title)}>
          No memory graph yet
        </h1>
        <p {...stylex.props(styles.messageText)}>
          Start a chat with Noema or use remember-this notes to create inspectable memories.
        </p>
      </section>
    );
  }

  return (
    <section {...stylex.props(styles.root)} aria-label="Memory Graph">
      <MemoryGraphControls
        query={query}
        statuses={statuses}
        limit={graph.summary.limit}
        truncated={graph.summary.truncated}
        onQueryChange={setQuery}
        onStatusesChange={handleStatusesChange}
      />
      {graph.edges.length === 0 ? (
        <div {...stylex.props(styles.messageState)}>
          <h1 {...stylex.props(styles.title)}>
            {hasActiveFilters ? "No matching memories" : "No memory graph yet"}
          </h1>
          <p {...stylex.props(styles.messageText)}>
            {hasActiveFilters
              ? "Adjust the search or status filters to bring memories back into view."
              : "Start a chat with Noema or use remember-this notes to create inspectable memories."}
          </p>
        </div>
      ) : (
        <div {...stylex.props(styles.graphLayout)}>
          <MemoryGraphCanvas
            nodes={layout.nodes}
            edges={layout.edges}
            selectedClaimId={selectedClaimId}
            onSelectClaim={setSelectedClaimId}
          />
          <MemoryGraphDetailPanel selectedEdge={selectedEdge} />
        </div>
      )}
    </section>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    minHeight: 0,
    gridTemplateRows: "auto minmax(0, 1fr)",
    overflow: "hidden"
  },
  centerState: {
    display: "grid",
    minHeight: 0,
    placeItems: "center",
    paddingInline: 24,
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  messageState: {
    display: "grid",
    width: "min(760px, 100%)",
    minHeight: 0,
    alignContent: "center",
    gap: 12,
    marginInline: "auto",
    padding: "28px 24px"
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 32,
    lineHeight: 1.1,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  messageText: {
    margin: 0,
    maxWidth: 560,
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  graphLayout: {
    display: "grid",
    minHeight: 0,
    gridTemplateColumns: "minmax(0, 1fr) minmax(300px, 360px)",
    overflow: "hidden",
    "@media (max-width: 900px)": {
      gridTemplateColumns: "1fr",
      gridTemplateRows: "minmax(420px, 1fr) minmax(0, 360px)"
    }
  }
});
