import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Pencil } from "lucide-react";
import type { McpTool, ToolPermissionDraft } from "./McpToolPermissionsModal";

export function McpToolPermissionRow({
  tool,
  draft,
  attention,
  autofilling,
  onEdit
}: {
  tool: McpTool;
  draft: ToolPermissionDraft;
  attention: string | null;
  autofilling: boolean;
  onEdit: () => void;
}) {
  if (autofilling) {
    return (
      <article {...stylex.props(styles.row, styles.pulse)}>
        <div {...stylex.props(styles.rowContent)}>
          <div {...stylex.props(styles.titleRow)}>
            <h3 {...stylex.props(styles.mutedTitle)}>{tool.name}</h3>
            <div {...stylex.props(styles.badgeGlimmer)} aria-hidden="true" />
          </div>
          <div {...stylex.props(styles.lineGlimmer)} aria-hidden="true" />
        </div>
        <div {...stylex.props(styles.actionGlimmer)} aria-hidden="true" />
      </article>
    );
  }

  return (
    <article {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.rowContent)}>
        <div {...stylex.props(styles.titleRow)}>
          <h3 {...stylex.props(styles.title)}>{tool.name}</h3>
          <ToolStatusBadge disabled={draft.disabled} />
          {attention ? <Badge variant="error" label={attention} /> : null}
        </div>
        <ToolPermissionSummary draft={draft} />
      </div>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        label={`Edit ${tool.name}`}
        icon={<Pencil {...stylex.props(styles.icon)} aria-hidden="true" />}
        isIconOnly
        onClick={onEdit}
      />
    </article>
  );
}

function ToolPermissionSummary({ draft }: { draft: ToolPermissionDraft }) {
  return (
    <div {...stylex.props(styles.summary)} aria-label="Configured tool permissions">
      <ClassificationBadge label="Read" value={draft.readClassification} />
      <ClassificationBadge label="Write" value={draft.writeClassification} />
      <ClassificationBadge label="Export" value={draft.exportClassification} />
    </div>
  );
}

function ToolStatusBadge({ disabled }: { disabled: boolean }) {
  return (
    <Badge
      variant={disabled ? "red" : "green"}
      label={disabled ? "Disabled" : "Enabled"}
    />
  );
}

function ClassificationBadge({ label, value }: { label: string; value: string }) {
  return (
    <Badge
      variant={value === "none" ? "neutral" : "blue"}
      label={`${label}: ${value}`}
          {...stylex.props(styles.monoBadge)}
    />
  );
}

const styles = stylex.create({
  row: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "center",
    gap: 12,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: 12
  },
  pulse: {
    opacity: 0.65
  },
  rowContent: {
    minWidth: 0
  },
  titleRow: {
    display: "flex",
    minWidth: 0,
    flexWrap: "wrap",
    alignItems: "center",
    gap: 8
  },
  title: {
    margin: 0,
    fontSize: 14,
    fontWeight: 600,
    lineHeight: 1.5,
    overflowWrap: "break-word",
    color: "var(--foreground)"
  },
  mutedTitle: {
    margin: 0,
    fontSize: 14,
    fontWeight: 600,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  summary: {
    display: "flex",
    minWidth: 0,
    flexWrap: "wrap",
    alignItems: "center",
    gap: 6,
    marginTop: 4
  },
  monoBadge: {
    fontFamily: "var(--font-mono)",
    fontSize: 12
  },
  badgeGlimmer: {
    width: 96,
    height: 20,
    borderRadius: 6,
    backgroundColor: "var(--muted)"
  },
  lineGlimmer: {
    width: "100%",
    maxWidth: 512,
    height: 16,
    marginTop: 8,
    borderRadius: 6,
    backgroundColor: "var(--muted)"
  },
  actionGlimmer: {
    width: 32,
    height: 32,
    borderRadius: 6,
    backgroundColor: "var(--muted)"
  },
  icon: {
    width: 16,
    height: 16
  }
});
