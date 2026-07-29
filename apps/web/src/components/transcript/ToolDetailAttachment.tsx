import * as stylex from "@stylexjs/stylex";
import { VStack } from "@astryxdesign/core/VStack";
import { WrenchIcon } from "lucide-react";
import { toolDetailRows, toolMarkerLabel } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
import { ToolDetailRow } from "./ToolDetailRow";

const styles = stylex.create({
  details: {
    maxWidth: 520,
    margin: "var(--spacing-0)",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    paddingTop: "var(--spacing-1-5)"
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
      <VStack as="dl" gap={1} className={stylex.props(styles.details).className}>
        {rows.map((row, index) => (
          <ToolDetailRow key={`${row.label}:${index}`} label={row.label} value={row.value} />
        ))}
      </VStack>
    </TranscriptAttachmentCard>
  );
}
