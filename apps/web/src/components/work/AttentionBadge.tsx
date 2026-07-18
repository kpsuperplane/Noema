import * as stylex from "@stylexjs/stylex";
import { CircleAlert } from "lucide-react";
import { sentenceCase } from "./workModel";

export function AttentionBadge({ kind, label }: { kind: string; label?: string }) {
  return (
    <span {...stylex.props(styles.badge)}>
      <CircleAlert aria-hidden="true" size={12} />
      {label ?? sentenceCase(kind)}
    </span>
  );
}

const styles = stylex.create({
  badge: {
    display: "inline-flex",
    width: "fit-content",
    alignItems: "center",
    gap: 5,
    borderRadius: 999,
    backgroundColor: "var(--clay-50)",
    paddingBlock: 4,
    paddingInline: 8,
    color: "var(--clay-600)",
    fontSize: 11,
    fontWeight: 650,
    lineHeight: 1.2
  }
});
