import * as stylex from "@stylexjs/stylex";
import { AlertTriangle } from "lucide-react";
import type { ShellAttention } from "./AppShell";

export function ShellAttentionItem({
  attention,
  compact = false
}: {
  attention: ShellAttention;
  compact?: boolean;
}) {
  return (
    <div
      {...stylex.props(styles.root, compact && styles.compact)}
      role="status"
      title={compact ? attention.message : undefined}
    >
      <span {...stylex.props(styles.title)}>
        <AlertTriangle size={14} aria-hidden="true" />
        {attention.title}
      </span>
      <span {...stylex.props(styles.message, compact && styles.compactMessage)}>
        {attention.message}
      </span>
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    gap: "var(--spacing-1)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--clay-600) 32%, transparent)",
    borderRadius: 6,
    backgroundColor: "var(--clay-50)",
    paddingBlock: "calc(var(--spacing-2) + var(--spacing-0-5))",
    paddingInline: "var(--spacing-3)",
    color: "var(--red-700)"
  },
  title: {
    display: "flex",
    alignItems: "center",
    gap: "var(--spacing-2)",
    fontSize: 12,
    fontWeight: 600
  },
  message: {
    fontSize: 12,
    lineHeight: 1.375
  },
  compact: {
    display: "flex",
    maxWidth: 360,
    alignItems: "center",
    gap: "var(--spacing-2)",
    borderWidth: 0,
    backgroundColor: "transparent",
    paddingBlock: "var(--spacing-0)",
    paddingInline: "var(--spacing-1-5)"
  },
  compactMessage: {
    minWidth: 0,
    overflow: "hidden",
    color: "var(--muted-foreground)",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    "@media (max-width: 760px)": { display: "none" }
  }
});
