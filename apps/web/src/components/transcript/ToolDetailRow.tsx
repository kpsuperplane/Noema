import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

const styles = stylex.create({
  label: {
    color: "var(--text-faint)",
    fontFamily: "var(--font-mono)",
    fontSize: 9,
    letterSpacing: "0.08em",
    textTransform: "uppercase"
  },
  value: {
    maxHeight: 88,
    margin: "var(--spacing-0)",
    overflow: "auto",
    overflowWrap: "break-word",
    color: "var(--text-muted)",
    fontSize: 12,
    lineHeight: 1.35,
    whiteSpace: "pre-wrap"
  }
});

export function ToolDetailRow({ label, value }: { label: string; value: string }) {
  return (
    <VStack gap={0.5}>
      <dt {...stylex.props(styles.label)}>{label}</dt>
      <dd {...stylex.props(styles.value)}>{value}</dd>
    </VStack>
  );
}
