import * as stylex from "@stylexjs/stylex";
import { WrenchIcon } from "lucide-react";
import { statusLabel } from "@/shared/format";
import { formatToolDetail, toolMarkerLabel } from "./markerModel";
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

  return (
    <TranscriptAttachmentCard
      id={id}
      title={title}
      description={description}
      icon={<WrenchIcon />}
      tone={failed ? "error" : "info"}
    >
      <dl {...stylex.props(styles.details)}>
        {call ? <ToolDetailRow label="Call" value={formatToolDetail(call.title, call.metadata)} /> : null}
        {result ? (
          <ToolDetailRow label="Result" value={formatToolDetail(result.summary ?? result.title, result.metadata)} />
        ) : null}
        <ToolDetailRow label="Status" value={statusLabel(result?.status ?? call?.status ?? "COMPLETED")} />
      </dl>
    </TranscriptAttachmentCard>
  );
}
