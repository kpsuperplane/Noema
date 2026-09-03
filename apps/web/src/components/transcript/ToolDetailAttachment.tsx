import * as stylex from "@stylexjs/stylex";
import { VStack } from "@astryxdesign/core/VStack";
import { toolHumanDetailRows, toolMarkerScreenshot } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailRow } from "./ToolDetailRow";

const styles = stylex.create({
  screenshotFrame: {
    position: "relative",
    margin: "var(--spacing-0)",
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--noema-surface-sunken)"
  },
  screenshot: {
    display: "block",
    width: "100%",
    height: "auto"
  },
  screenshotUrl: {
    position: "absolute",
    insetBlockStart: 0,
    insetInline: 0,
    overflow: "hidden",
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)",
    backgroundColor: "var(--color-overlay)",
    color: "var(--color-on-dark)",
    fontFamily: "var(--font-family-code)",
    fontSize: "var(--font-size-sm)",
    lineHeight: "var(--text-code-leading)",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap"
  }
});

export function ToolDetailAttachment({ id, marker }: { id: string; marker: ToolMarkerGroup }) {
  const humanRows = toolHumanDetailRows(marker);
  const screenshot = toolMarkerScreenshot(marker);
  if (humanRows.length === 0 && !screenshot) {
    return null;
  }

  return (
    <VStack id={id} gap={2} maxWidth={520}>
      {screenshot ? (
        <figure {...stylex.props(styles.screenshotFrame)}>
          <img {...stylex.props(styles.screenshot)} alt="Rendered browser page" src={screenshot.src} width={screenshot.width} height={screenshot.height} />
          {screenshot.url ? (
            <figcaption {...stylex.props(styles.screenshotUrl)} title={screenshot.url}>
              {screenshot.url}
            </figcaption>
          ) : null}
        </figure>
      ) : null}
      {humanRows.length > 0 ? (
        <VStack as="dl" gap={2}>
          {humanRows.map((row, index) => (
            <ToolDetailRow key={`${row.label}:${index}`} label={row.label} value={row.value} />
          ))}
        </VStack>
      ) : null}
    </VStack>
  );
}
