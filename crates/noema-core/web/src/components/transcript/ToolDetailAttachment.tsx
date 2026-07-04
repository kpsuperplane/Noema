import * as stylex from "@stylexjs/stylex";
import { WrenchIcon } from "lucide-react";
import { statusLabel } from "@/shared/format";
import { toolDetailRows, toolMarkerLabel } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
import { ToolDetailRow } from "./ToolDetailRow";

const styles = stylex.create({
  details: {
    display: "grid",
    gap: 8,
    margin: 0,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    paddingTop: 10
  }
});

export function ToolDetailAttachment({ id, marker }: { id: string; marker: ToolMarkerGroup }) {
  const result = marker.result?.item;
  const call = marker.call?.item;
  const failed = result?.status === "FAILED";
  const title = toolMarkerLabel(marker);
  const description = result?.summary ?? call?.summary ?? statusLabel(result?.status ?? call?.status ?? "COMPLETED");
  const rows = toolDetailRows(marker);
  const status = statusLabel(result?.status ?? call?.status ?? "COMPLETED");
  const showStatus = rows.length === 0 || failed || !result;

  return (
    <TranscriptAttachmentCard
      id={id}
      title={title}
      description={description}
      icon={<WrenchIcon />}
      tone={failed ? "error" : "info"}
    >
      <dl {...stylex.props(styles.details)}>
        {rows.map((row, index) => (
          <ToolDetailRow key={`${row.label}:${index}`} label={row.label} value={row.value} />
        ))}
        {showStatus ? <ToolDetailRow label="Status" value={status} /> : null}
      </dl>
    </TranscriptAttachmentCard>
  );
}
