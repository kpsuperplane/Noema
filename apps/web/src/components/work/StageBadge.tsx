import * as stylex from "@stylexjs/stylex";

export function StageBadge({ name }: { name: string; behavior?: string }) {
  return <span {...stylex.props(styles.label)}>{name}</span>;
}

const styles = stylex.create({
  label: { color: "var(--noema-text-secondary)", fontSize: 10, fontWeight: 650 }
});
