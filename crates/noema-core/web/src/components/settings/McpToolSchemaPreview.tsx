import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import type { McpTool } from "./McpToolPermissionsModal";

export function McpToolSchemaPreview({ tool }: { tool: McpTool }) {
  return (
    <details {...stylex.props(styles.details)}>
      <summary {...stylex.props(styles.summary)}>Review discovered schema</summary>
      <pre {...stylex.props(styles.pre)}>{formatSchemaPreview(tool)}</pre>
    </details>
  );
}

export function McpToolDescription({ description }: { description: string }) {
  const [expanded, setExpanded] = React.useState(false);

  return (
    <div {...stylex.props(styles.description)}>
      <div {...stylex.props(expanded ? styles.descriptionExpanded : styles.descriptionCollapsed)}>
        <p {...stylex.props(expanded ? styles.descriptionTextExpanded : styles.descriptionText)}>
          {description}
        </p>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          label={expanded ? "Show less" : "Show more..."}
          {...stylex.props(styles.inlineButton)}
          onClick={() => setExpanded((current) => !current)}
        />
      </div>
    </div>
  );
}

function formatSchemaPreview(tool: McpTool) {
  return JSON.stringify(
    {
      inputSchema: tool.inputSchema,
      outputSchema: tool.outputSchema,
      annotations: tool.annotations
    },
    null,
    2
  );
}

const styles = stylex.create({
  details: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    padding: 12
  },
  summary: {
    cursor: "pointer",
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  pre: {
    maxHeight: 224,
    margin: "12px 0 0",
    overflow: "auto",
    whiteSpace: "pre-wrap",
    borderRadius: 6,
    backgroundColor: "var(--muted)",
    padding: 12,
    fontSize: 12,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  description: {
    minWidth: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  descriptionExpanded: {
    display: "grid",
    gap: 4
  },
  descriptionCollapsed: {
    display: "flex",
    minWidth: 0,
    alignItems: "baseline",
    gap: 8
  },
  descriptionText: {
    minWidth: 0,
    flex: 1,
    margin: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap"
  },
  descriptionTextExpanded: {
    margin: 0,
    whiteSpace: "pre-wrap"
  },
  inlineButton: {
    width: "fit-content",
    height: "auto",
    flexShrink: 0,
    padding: 0,
    fontSize: 12
  }
});
