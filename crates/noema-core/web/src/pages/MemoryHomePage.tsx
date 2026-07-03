import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import * as stylex from "@stylexjs/stylex";
import { Network } from "lucide-react";

export function MemoryHomePage({ onOpenGraph }: { onOpenGraph: () => void }) {
  return (
    <section {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}>
        <p {...stylex.props(styles.eyebrow)}>Memory</p>
        <h1 {...stylex.props(styles.title)}>
          Memory management
        </h1>
        <p {...stylex.props(styles.description)}>
          Inspect what Noema remembers and how those memories connect.
        </p>
      </div>

      <Card {...stylex.props(styles.graphCard)} padding={4}>
        <div {...stylex.props(styles.graphEntry)}>
          <Network {...stylex.props(styles.graphIcon)} aria-hidden="true" />
          <div {...stylex.props(styles.graphText)}>
            <strong {...stylex.props(styles.graphTitle)}>Owner inspection graph</strong>
            <span {...stylex.props(styles.graphDescription)}>
              Inspect full graph claims, entity links, and evidence.
            </span>
          </div>
        </div>
        <Button
          {...stylex.props(styles.openButton)}
          type="button"
          label="Open inspection"
          onClick={onOpenGraph}
        >
          Open inspection
        </Button>
      </Card>
    </section>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    width: "min(960px, 100%)",
    minHeight: 0,
    gap: 20,
    marginInline: "auto",
    padding: "28px 24px",
    "@media (max-width: 760px)": {
      paddingInline: 20
    }
  },
  header: {
    display: "grid",
    gap: 8
  },
  eyebrow: {
    margin: 0,
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    letterSpacing: "0.12em",
    color: "var(--text-accent)",
    textTransform: "uppercase"
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 32,
    lineHeight: 1.1,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  description: {
    margin: 0,
    maxWidth: 640,
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  graphCard: {
    display: "grid",
    gap: 12
  },
  graphEntry: {
    display: "flex",
    alignItems: "center",
    gap: 12,
    minWidth: 0
  },
  graphIcon: {
    width: 20,
    height: 20,
    flexShrink: 0,
    color: "var(--text-accent)"
  },
  graphText: {
    minWidth: 0
  },
  graphTitle: {
    display: "block",
    fontSize: 14,
    fontWeight: 600
  },
  graphDescription: {
    display: "block",
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  openButton: {
    width: "fit-content"
  }
});
