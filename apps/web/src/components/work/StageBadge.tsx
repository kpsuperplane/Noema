import * as stylex from "@stylexjs/stylex";

export function StageBadge({ name, behavior }: { name: string; behavior: string }) {
  return <span data-behavior={behavior} {...stylex.props(styles.badge)}>{name}</span>;
}

const styles = stylex.create({
  badge: {
    display: "inline-flex",
    width: "fit-content",
    alignItems: "center",
    minHeight: 22,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 999,
    backgroundColor: "var(--surface-sunken)",
    paddingInline: 8,
    fontSize: 11,
    fontWeight: 650,
    lineHeight: 1,
    color: "var(--text-secondary)"
  }
});
