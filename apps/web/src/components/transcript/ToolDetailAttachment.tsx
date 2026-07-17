import * as stylex from "@stylexjs/stylex";
import { WrenchIcon } from "lucide-react";
import { toolDetailRows, toolMarkerLabel } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
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
  const failed = marker.result?.item.status === "FAILED";

  return (
    <TranscriptAttachmentCard
      id={id}
      title={toolMarkerLabel(marker)}
      icon={<WrenchIcon />}
      tone={failed ? "error" : "info"}
    >
      <dl {...stylex.props(styles.details)}>
        {rows.map((row, index) => (
          <ToolDetailRow key={`${row.label}:${index}`} label={row.label} value={row.value} />
        ))}
      </dl>
    </TranscriptAttachmentCard>
  );
}
