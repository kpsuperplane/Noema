import type { Edge, EdgeProps } from "@xyflow/react";
import type { MemoryGraphEdgeData } from "@/memoryGraphLayout";

export function MemoryClaimEdge({
  id,
  label,
  selected,
  source,
  sourceX,
  sourceY,
  target,
  targetX,
  targetY
}: EdgeProps<Edge<MemoryGraphEdgeData>>) {
  const selfEdge = source === target;
  const edgePath = selfEdge
    ? `M ${sourceX} ${sourceY} C ${sourceX + 120} ${sourceY - 120}, ${targetX + 120} ${targetY + 120}, ${targetX} ${targetY}`
    : `M ${sourceX} ${sourceY} L ${targetX} ${targetY}`;
  const stroke = selected ? "var(--pine-500)" : "rgba(23, 22, 15, 0.32)";

  return (
    <g>
      <path
        className="react-flow__edge-interaction"
        d={edgePath}
        stroke="transparent"
        strokeWidth={18}
        fill="none"
        pointerEvents="stroke"
      />
      <path
        id={id}
        className="react-flow__edge-path"
        d={edgePath}
        stroke={stroke}
        strokeWidth={selected ? 2.5 : 1.5}
        fill="none"
      />
      {label ? (
        <text style={labelStyle}>
          <textPath href={`#${id}`} startOffset="50%" textAnchor="middle">
            {label}
          </textPath>
        </text>
      ) : null}
    </g>
  );
}

const labelStyle = {
  fill: "var(--text-secondary)",
  fontSize: 11
};
