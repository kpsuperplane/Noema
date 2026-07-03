import { Card } from "@astryxdesign/core/Card";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@apollo/client/react";

import { MemoryGraphClaimDetailDocument } from "@/generated/graphql";
import type { NormalizedMemoryGraphEdge } from "@/memory/graph";

export function MemoryGraphDetailPanel({ selectedEdge }: { selectedEdge: NormalizedMemoryGraphEdge | null }) {
  const detail = useQuery(MemoryGraphClaimDetailDocument, {
    variables: { claimId: selectedEdge?.claimId ?? "" },
    skip: !selectedEdge,
    fetchPolicy: "cache-and-network",
  });

  if (!selectedEdge) {
    return (
      <aside {...stylex.props(styles.root)}>
        <h2 {...stylex.props(styles.panelTitle)}>Memory detail</h2>
        <p {...stylex.props(styles.mutedText)}>Select a claim edge to inspect evidence.</p>
      </aside>
    );
  }

  const claim = detail.data?.memoryClaim;
  const claimUnavailable = !detail.loading && !detail.error && detail.data && !claim;

  return (
    <aside {...stylex.props(styles.root, styles.selectedRoot)}>
      <div {...stylex.props(styles.header)}>
        <p {...stylex.props(styles.eyebrow)}>
          {claim?.status ?? selectedEdge.status}
        </p>
        <h2 {...stylex.props(styles.panelTitle)}>
          {claim?.predicateLabel ?? selectedEdge.predicateLabel}
        </h2>
        <p {...stylex.props(styles.mutedText)}>{claim?.fact ?? selectedEdge.fact}</p>
      </div>

      <dl {...stylex.props(styles.metadata)}>
        <div>
          <dt {...stylex.props(styles.metadataLabel)}>Sensitivity</dt>
          <dd {...stylex.props(styles.metadataValue)}>{claim?.sensitivity ?? selectedEdge.sensitivity}</dd>
        </div>
        <div>
          <dt {...stylex.props(styles.metadataLabel)}>Evidence</dt>
          <dd {...stylex.props(styles.metadataValue)}>{claim?.evidenceCount ?? selectedEdge.evidenceCount}</dd>
        </div>
      </dl>

      {detail.loading ? <p {...stylex.props(styles.mutedText)}>Loading evidence...</p> : null}
      {detail.error ? <p {...stylex.props(styles.errorText)}>{detail.error.message}</p> : null}
      {claimUnavailable ? (
        <p {...stylex.props(styles.mutedText)}>
          Claim detail is unavailable. The graph edge summary remains visible.
        </p>
      ) : null}

      {claim?.evidence.length ? (
        <div {...stylex.props(styles.evidenceList)}>
          <h3 {...stylex.props(styles.evidenceTitle)}>Evidence</h3>
          {claim.evidence.map((evidence, index) => {
            const key =
              evidence.evidenceId ??
              `${evidence.sourceItemId ?? "source"}:${evidence.observedAt ?? evidence.createdAt}:${index}`;

            return (
              <Card key={key} padding={3}>
                <article {...stylex.props(styles.evidenceCard)}>
                  <strong {...stylex.props(styles.evidenceAuthority)}>{evidence.authority}</strong>
                  {evidence.excerpt ? (
                    <p {...stylex.props(styles.evidenceExcerpt)}>{evidence.excerpt}</p>
                  ) : null}
                  {evidence.sourceItemId ? (
                    <code {...stylex.props(styles.sourceCode)}>{evidence.sourceItemId}</code>
                  ) : null}
                </article>
              </Card>
            );
          })}
        </div>
      ) : null}
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
  },
  errorText: {
    margin: 0,
    fontSize: 14,
    color: "var(--destructive)"
  },
  evidenceList: {
    display: "grid",
    gap: 8
  },
  evidenceTitle: {
    margin: 0,
    fontSize: 14,
    fontWeight: 600
  },
  evidenceCard: {
    display: "grid",
    gap: 8
  },
  evidenceAuthority: {
    display: "block",
    fontSize: 12
  },
  evidenceExcerpt: {
    margin: 0,
    fontSize: 14,
    color: "var(--muted-foreground)"
  },
  sourceCode: {
    display: "block",
    marginTop: 0,
    fontSize: 11,
    color: "var(--muted-foreground)",
    overflowWrap: "anywhere"
  }
});
