import { forceCenter, forceCollide, forceLink, forceManyBody, forceSimulation } from "d3-force";
import type { Edge, Node } from "@xyflow/react";
import type { NormalizedMemoryGraph, NormalizedMemoryGraphEdge, NormalizedMemoryGraphNode } from "./memoryGraph";

export type MemoryGraphNodeData = NormalizedMemoryGraphNode;
export type MemoryGraphEdgeData = NormalizedMemoryGraphEdge;

type LayoutOptions = {
  width: number;
  height: number;
};

type SimulationNode = {
  id: string;
  data: MemoryGraphNodeData;
  x: number;
  y: number;
};

type SimulationLink = {
  source: string;
  target: string;
};

const SIMULATION_TICKS = 180;

export function layoutMemoryGraph(
  graph: NormalizedMemoryGraph,
  { width, height }: LayoutOptions,
): {
  nodes: Node<MemoryGraphNodeData>[];
  edges: Edge<MemoryGraphEdgeData>[];
} {
  const orderedNodes = [...graph.nodes].sort((left, right) => left.nodeId.localeCompare(right.nodeId));
  const orderedEdges = [...graph.edges].sort((left, right) => left.claimId.localeCompare(right.claimId));
  const centerX = width / 2;
  const centerY = height / 2;
  const radius = Math.max(80, Math.min(width, height) * 0.32);
  const nodeCount = Math.max(orderedNodes.length, 1);

  const simulationNodes: SimulationNode[] = orderedNodes.map((node, index) => {
    const angle = (index / nodeCount) * Math.PI * 2;

    return {
      id: node.nodeId,
      data: node,
      x: centerX + Math.cos(angle) * radius,
      y: centerY + Math.sin(angle) * radius,
    };
  });

  const simulationLinks: SimulationLink[] = orderedEdges.map((edge) => ({
    source: edge.sourceNodeId,
    target: edge.targetNodeId,
  }));

  forceSimulation(simulationNodes)
    .force(
      "link",
      forceLink<SimulationNode, SimulationLink>(simulationLinks)
        .id((node) => node.id)
        .distance(180)
        .strength(0.45),
    )
    .force("charge", forceManyBody().strength(-420))
    .force("center", forceCenter(centerX, centerY))
    .force("collide", forceCollide<SimulationNode>().radius(72).strength(0.7))
    .stop()
    .tick(SIMULATION_TICKS);

  return {
    nodes: simulationNodes.map((node) => ({
      id: node.id,
      type: "memoryEntity",
      position: {
        x: node.x,
        y: node.y,
      },
      data: node.data,
    })),
    edges: orderedEdges.map((edge) => ({
      id: edge.claimId,
      source: edge.sourceNodeId,
      target: edge.targetNodeId,
      type: "memoryClaim",
      label: edge.label,
      data: edge,
    })),
  };
}
