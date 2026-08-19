import * as stylex from "@stylexjs/stylex";
import { Collapsible } from "@astryxdesign/core/Collapsible";
import { VStack } from "@astryxdesign/core/VStack";
import { WrenchIcon } from "lucide-react";
import { toolDetailRows, toolMarkerIsBuiltIn, toolMarkerLabel, toolMarkerScreenshot } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
import { ToolDetailRow } from "./ToolDetailRow";

const styles = stylex.create({
  content: { maxWidth: 520 },
  screenshot: {
    display: "block",
    width: "100%",
    height: "auto",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--noema-surface-sunken)"
  },
  details: {
    margin: "var(--spacing-0)",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    paddingTop: "var(--spacing-1-5)"
  }
});

export function ToolDetailAttachment({ id, marker }: { id: string; marker: ToolMarkerGroup }) {
  const rows = toolDetailRows(marker);
  const screenshot = toolMarkerScreenshot(marker);
  if (rows.length === 0 && !screenshot) {
    return null;
  }
  const failed = marker.result?.item.status === "FAILED";
  const details = (
    <VStack as="dl" gap={1} className={stylex.props(styles.details).className}>
      {rows.map((row, index) => (
        <ToolDetailRow key={`${row.label}:${index}`} label={row.label} value={row.value} />
      ))}
    </VStack>
  );

  return (
    <TranscriptAttachmentCard
      id={id}
      title={toolMarkerLabel(marker)}
      icon={<WrenchIcon />}
      tone={failed ? "error" : "info"}
    >
      <VStack gap={2} className={stylex.props(styles.content).className}>
        {screenshot ? (
          <img
            {...stylex.props(styles.screenshot)}
            alt="Rendered browser page"
            src={screenshot.src}
            width={screenshot.width}
            height={screenshot.height}
          />
        ) : null}
        {rows.length > 0 ? (
          toolMarkerIsBuiltIn(marker) ? (
            <Collapsible trigger="Technical details" defaultIsOpen={false}>
              {details}
            </Collapsible>
          ) : details
        ) : null}
      </VStack>
    </TranscriptAttachmentCard>
  );
}
