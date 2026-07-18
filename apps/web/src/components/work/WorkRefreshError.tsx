import * as stylex from "@stylexjs/stylex";

export function WorkRefreshError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div role="status" aria-live="polite" {...stylex.props(styles.root)}>
      <span>{message}</span>
      <button type="button" {...stylex.props(styles.retry)} onClick={onRetry}>Retry</button>
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 6,
    borderRadius: 8,
    backgroundColor: "var(--clay-50)",
    paddingBlock: 7,
    paddingInline: 9,
    color: "var(--clay-600)",
    fontSize: 12
  },
  retry: {
    borderWidth: 0,
    backgroundColor: "transparent",
    padding: 0,
    color: "inherit",
    font: "inherit",
    fontWeight: 700,
    textDecoration: "underline",
    cursor: "pointer",
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 2
    }
  }
});
