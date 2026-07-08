import * as stylex from "@stylexjs/stylex";

export function MemoryGraphPage() {
  return (
    <section {...stylex.props(styles.messageState)} aria-label="Memory Graph">
      <h1 {...stylex.props(styles.title)}>Memory graph moved</h1>
      <p {...stylex.props(styles.messageText)}>
        Memory is now configured through Settings.
      </p>
    </section>
  );
}

const styles = stylex.create({
  messageState: {
    display: "grid",
    width: "min(760px, 100%)",
    minHeight: 0,
    alignContent: "center",
    gap: 12,
    marginInline: "auto",
    padding: "28px 24px"
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 32,
    lineHeight: 1.1,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  messageText: {
    margin: 0,
    maxWidth: 560,
    fontSize: 14,
    color: "var(--muted-foreground)"
  }
});
