import * as stylex from "@stylexjs/stylex";

export function AuditSettingsPaneContent() {
  return (
    <div {...stylex.props(styles.card)}>
      <p {...stylex.props(styles.mutedText)}>
        MCP audit records will appear here after mediated tool calls run.
      </p>
    </div>
  );
}

const styles = stylex.create({
  card: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: 16
  },
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  }
});
