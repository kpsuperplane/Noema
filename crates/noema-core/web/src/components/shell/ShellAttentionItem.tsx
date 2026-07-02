import * as stylex from "@stylexjs/stylex";
import { AlertTriangle } from "lucide-react";
import type { ShellAttention } from "./AppShell";

export function ShellAttentionItem({ attention }: { attention: ShellAttention }) {
  return (
    <div
      {...stylex.props(styles.root)}
      role="status"
    >
      <span {...stylex.props(styles.title)}>
        <AlertTriangle size={14} aria-hidden="true" />
        {attention.title}
      </span>
      <span {...stylex.props(styles.message)}>{attention.message}</span>
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    gap: 4,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--clay-600) 32%, transparent)",
    borderRadius: 6,
    backgroundColor: "var(--clay-50)",
    paddingBlock: 10,
    paddingInline: 12,
    color: "var(--red-700)"
  },
  title: {
    display: "flex",
    alignItems: "center",
    gap: 8,
    fontSize: 12,
    fontWeight: 600
  },
  message: {
    fontSize: 12,
    lineHeight: 1.375
  }
});
