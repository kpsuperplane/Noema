import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

const styles = stylex.create({
  label: {
    color: "var(--text-muted)",
    fontSize: 12,
    fontWeight: 600
  },
  technicalLabel: {
    fontFamily: "var(--font-mono)",
    letterSpacing: "0.08em",
    textTransform: "uppercase"
  },
  value: {
    margin: "var(--spacing-0)",
    overflowWrap: "break-word",
    color: "var(--text-secondary)",
    fontSize: 13,
    lineHeight: 1.45,
    whiteSpace: "pre-wrap"
  },
  technicalValue: {
    color: "var(--text-muted)",
    fontFamily: "var(--font-mono)",
    fontSize: 12,
    lineHeight: 1.35
  }
});

export function ToolDetailRow({
  label,
  value,
  technical = false
}: {
  label: string;
  value: string;
  technical?: boolean;
}) {
  return (
    <VStack gap={0.5}>
      <dt {...stylex.props(styles.label, technical && styles.technicalLabel)}>{label}</dt>
      <dd {...stylex.props(styles.value, technical && styles.technicalValue)}>{value}</dd>
    </VStack>
  );
}
