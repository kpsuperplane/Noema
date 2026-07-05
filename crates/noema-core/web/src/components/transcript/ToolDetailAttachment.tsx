import * as stylex from "@stylexjs/stylex";
import { toolDetailRows, toolMarkerLabel } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailRow } from "./ToolDetailRow";

const styles = stylex.create({
  details: {
    display: "grid",
    maxWidth: 520,
    margin: 0,
    gap: 4,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    paddingTop: 6
  }
});

export function ToolDetailAttachment({ id, marker }: { id: string; marker: ToolMarkerGroup }) {
  const rows = toolDetailRows(marker);
  if (rows.length === 0) {
    return null;
  }

  return (
    <dl id={id} {...stylex.props(styles.details)} aria-label={`${toolMarkerLabel(marker)} details`}>
      {rows.map((row, index) => (
        <ToolDetailRow key={`${row.label}:${index}`} label={row.label} value={row.value} />
      ))}
    </dl>
  );
}
