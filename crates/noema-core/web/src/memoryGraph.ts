import type { MemoryGraphQuery } from "./generated/graphql";

type WithOptionalTypename<T, Typename extends string> = T & {
  __typename?: Typename;
};

export type MemoryGraphResult = Omit<MemoryGraphQuery["memoryGraph"], "nodes" | "edges" | "summary"> & {
  nodes: MemoryGraphNodeResult[];
  edges: MemoryGraphEdgeResult[];
  summary: WithOptionalTypename<MemoryGraphQuery["memoryGraph"]["summary"], "GraphqlMemoryGraphSummary">;
};

export type MemoryGraphNodeResult = WithOptionalTypename<
  MemoryGraphQuery["memoryGraph"]["nodes"][number],
  "GraphqlMemoryGraphNode"
>;

export type MemoryGraphEdgeResult = WithOptionalTypename<
  MemoryGraphQuery["memoryGraph"]["edges"][number],
  "GraphqlMemoryGraphEdge"
>;

export type NormalizedMemoryGraphNode = MemoryGraphNodeResult;

export type NormalizedMemoryGraphEdge = MemoryGraphEdgeResult & {
  label: string;
};

export type NormalizedMemoryGraph = Omit<MemoryGraphResult, "nodes" | "edges"> & {
  nodes: NormalizedMemoryGraphNode[];
  edges: NormalizedMemoryGraphEdge[];
};

export type GraphStatusOption = {
  value: string;
  label: string;
};

const STATUS_OPTIONS: GraphStatusOption[] = [
  { value: "candidate", label: "Candidate" },
  { value: "active", label: "Active" },
  { value: "confirmed", label: "Confirmed" },
  { value: "disputed", label: "Disputed" },
  { value: "superseded", label: "Superseded" },
  { value: "archived", label: "Archived" },
  { value: "deleted", label: "Deleted" },
];

export function graphStatusDefaults(): string[] {
  return ["candidate", "active", "confirmed"];
}

export function graphStatusOptions(): GraphStatusOption[] {
  return [...STATUS_OPTIONS];
}

export function normalizeMemoryGraph(graph: MemoryGraphResult): NormalizedMemoryGraph {
  return {
    nodes: [...graph.nodes].sort((left, right) => left.nodeId.localeCompare(right.nodeId)),
    edges: [...graph.edges]
      .sort((left, right) => left.claimId.localeCompare(right.claimId))
      .map((edge) => ({
        ...edge,
        label: edge.predicateLabel,
      })),
    summary: graph.summary,
  };
}

export function toggleStatus(current: string[], status: string): string[] {
  if (current.includes(status)) {
    return current.filter((currentStatus) => currentStatus !== status);
  }

  return [...current, status];
}
