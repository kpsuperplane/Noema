import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import * as stylex from "@stylexjs/stylex";
import type { MemoryGraphNodeData } from "@/memory/graphLayout";

export function MemoryEntityNode({ data, selected }: NodeProps<Node<MemoryGraphNodeData>>) {
  return (
    <div {...stylex.props(styles.root, selected ? styles.selected : styles.unselected)}>
      <Handle type="target" position={Position.Left} style={hiddenHandleStyle} />
      <strong {...stylex.props(styles.label)}>{data.label}</strong>
      <span {...stylex.props(styles.meta)}>
        {data.entityType} / {data.claimCount} memories
      </span>
      <Handle type="source" position={Position.Right} style={hiddenHandleStyle} />
    </div>
  );
}

const hiddenHandleStyle = { opacity: 0 };

const styles = stylex.create({
  root: {
    display: "grid",
    alignContent: "center",
    gap: 4,
    width: 180,
    height: 76,
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderRadius: 6,
    backgroundColor: "white",
    padding: "8px 12px",
    boxShadow: "0 1px 2px rgba(23, 22, 15, 0.08)"
  },
  selected: {
    borderColor: "var(--pine-500)",
    boxShadow: "0 0 0 2px color-mix(in srgb, var(--pine-500) 15%, transparent)"
  },
  unselected: {
    borderColor: "var(--border-subtle)"
  },
  label: {
    display: "block",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 14,
    fontWeight: 600,
    lineHeight: 1.25
  },
  meta: {
    display: "block",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 12,
    color: "var(--muted-foreground)"
  }
});
