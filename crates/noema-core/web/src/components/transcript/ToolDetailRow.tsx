import * as stylex from "@stylexjs/stylex";

const styles = stylex.create({
  root: {
    display: "grid",
    gap: 1
  },
  label: {
    color: "var(--text-faint)",
    fontFamily: "var(--font-mono)",
    fontSize: 9,
    letterSpacing: "0.08em",
    textTransform: "uppercase"
  },
  value: {
    maxHeight: 88,
    margin: 0,
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
    <div {...stylex.props(styles.root)}>
      <dt {...stylex.props(styles.label)}>{label}</dt>
      <dd {...stylex.props(styles.value)}>{value}</dd>
    </div>
  );
}
