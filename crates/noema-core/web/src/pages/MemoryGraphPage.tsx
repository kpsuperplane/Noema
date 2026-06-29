import React from "react";
import { useQuery } from "@apollo/client/react";

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

  if (result.loading && !graph) {
    return (
      <section className="grid min-h-0 place-items-center px-6 text-sm text-muted-foreground" aria-label="Memory Graph">
        Loading memory graph...
      </section>
    );
  }

  if (result.error) {
    return (
      <section className="mx-auto grid min-h-0 w-[min(760px,100%)] content-center gap-3 px-6 py-7" aria-label="Memory Graph">
        <h1 className="m-0 font-heading text-[32px] leading-[1.1] tracking-normal text-foreground">
          Memory graph unavailable
        </h1>
        <p className="m-0 text-sm text-muted-foreground">{result.error.message}</p>
      </section>
    );
  }

  if (!graph || graph.edges.length === 0) {
    return (
      <section className="mx-auto grid min-h-0 w-[min(760px,100%)] content-center gap-3 px-6 py-7" aria-label="Memory Graph">
        <h1 className="m-0 font-heading text-[32px] leading-[1.1] tracking-normal text-foreground">
          No memory graph yet
        </h1>
        <p className="m-0 max-w-[560px] text-sm text-muted-foreground">
          Start a chat with Noema or use remember-this notes to create inspectable memories.
        </p>
      </section>
    );
  }

  return (
    <section className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden" aria-label="Memory Graph">
      <MemoryGraphControls
        query={query}
        statuses={statuses}
        limit={graph.summary.limit}
        truncated={graph.summary.truncated}
        onQueryChange={setQuery}
        onStatusesChange={handleStatusesChange}
      />
      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_minmax(300px,360px)] overflow-hidden max-[900px]:grid-cols-1 max-[900px]:grid-rows-[minmax(420px,1fr)_auto]">
        <MemoryGraphCanvas
          nodes={layout.nodes}
          edges={layout.edges}
          selectedClaimId={selectedClaimId}
          onSelectClaim={setSelectedClaimId}
        />
        <MemoryGraphDetailPanel selectedEdge={selectedEdge} />
      </div>
    </section>
  );
}
