import * as stylex from "@stylexjs/stylex";
import type { NormalizedMemoryGraphEdge } from "@/memory/graph";

export function MemoryGraphDetailPanel({
  selectedEdge
}: {
  selectedEdge: NormalizedMemoryGraphEdge | null;
}) {
  if (!selectedEdge) {
    return (
      <aside {...stylex.props(styles.root)}>
        <h2 {...stylex.props(styles.panelTitle)}>Memory detail</h2>
        <p {...stylex.props(styles.mutedText)}>Select a memory link to inspect evidence.</p>
      </aside>
    );
  }

  return (
    <aside {...stylex.props(styles.root, styles.selectedRoot)}>
      <div {...stylex.props(styles.header)}>
        <p {...stylex.props(styles.eyebrow)}>{selectedEdge.status}</p>
        <h2 {...stylex.props(styles.panelTitle)}>{selectedEdge.predicateLabel}</h2>
        <p {...stylex.props(styles.mutedText)}>{selectedEdge.fact}</p>
      </div>
      <dl {...stylex.props(styles.metadata)}>
        <div>
          <dt {...stylex.props(styles.metadataLabel)}>Sensitivity</dt>
          <dd {...stylex.props(styles.metadataValue)}>{selectedEdge.sensitivity}</dd>
        </div>
        <div>
          <dt {...stylex.props(styles.metadataLabel)}>Evidence</dt>
          <dd {...stylex.props(styles.metadataValue)}>{selectedEdge.evidenceCount}</dd>
        </div>
      </dl>
    </aside>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    minHeight: 0,
    alignContent: "start",
    gap: 8,
    overflowY: "auto",
    borderLeft: "1px solid var(--border-subtle)",
    backgroundColor: "white",
    padding: 20,
    "@media (max-width: 900px)": {
      borderLeftWidth: 0,
      borderTop: "1px solid var(--border-subtle)"
    }
  },
  selectedRoot: {
    gap: 16
  },
  header: {
    display: "grid",
    gap: 4
  },
  eyebrow: {
    margin: 0,
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    letterSpacing: "0.12em",
    color: "var(--text-accent)",
    textTransform: "uppercase"
  },
  panelTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    lineHeight: 1.35,
    letterSpacing: 0
  },
  mutedText: {
    margin: 0,
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  metadata: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: 12,
    fontSize: 14
  },
  metadataLabel: {
    fontSize: 12,
    color: "var(--muted-foreground)"
  },
  metadataValue: {
    margin: 0
  }
});
