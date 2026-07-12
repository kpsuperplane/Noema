import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { TaskDisclosureSection } from "./TaskDisclosureSection";
import type { TaskModelSnapshot } from "./taskTypes";

export function TaskModelSnapshots({
  executor,
  reviewer,
  reviewerInherited
}: {
  executor?: TaskModelSnapshot | null;
  reviewer?: TaskModelSnapshot | null;
  reviewerInherited?: boolean;
}) {
  if (!executor && !reviewer) {
    return null;
  }

  return (
    <TaskDisclosureSection
      id="task-models-title"
      summary={modelSummary(executor, reviewer)}
      title="Agent models"
    >
      <p {...stylex.props(styles.hint)}>Captured when the task was created</p>
      <div {...stylex.props(styles.grid)}>
        {executor ? <ModelCard label="Executor" snapshot={executor} /> : null}
        {reviewer ? (
          <ModelCard
            label="Reviewer"
            snapshot={reviewer}
            inherited={reviewerInherited || reviewer.inherited}
          />
        ) : null}
      </div>
    </TaskDisclosureSection>
  );
}

function modelSummary(
  executor?: TaskModelSnapshot | null,
  reviewer?: TaskModelSnapshot | null
): string {
  return [
    executor ? `Executor · ${executor.modelLabel || executor.modelProfile}` : null,
    reviewer ? `Reviewer · ${reviewer.modelLabel || reviewer.modelProfile}` : null
  ].filter(Boolean).join(" · ");
}

function ModelCard({
  label,
  snapshot,
  inherited
}: {
  label: string;
  snapshot: TaskModelSnapshot;
  inherited?: boolean;
}) {
  const modelName = snapshot.modelLabel || snapshot.modelProfile;
  const provider = snapshot.providerDisplayName;
  return (
    <article {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.cardHeader)}>
        <h4 {...stylex.props(styles.cardTitle)}>{label}</h4>
        {inherited ? <Badge variant="neutral" label="Uses primary model" {...stylex.props(styles.badge)} /> : null}
      </div>
      <p {...stylex.props(styles.model)}>{modelName}</p>
      <dl {...stylex.props(styles.metadata)}>
        {provider ? <MetadataRow label="Provider" value={provider} /> : null}
        {snapshot.reasoningEffort ? <MetadataRow label="Reasoning" value={snapshot.reasoningEffort} /> : null}
      </dl>
    </article>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div {...stylex.props(styles.metadataRow)}>
      <dt {...stylex.props(styles.metadataLabel)}>{label}</dt>
      <dd {...stylex.props(styles.metadataValue)}>{value}</dd>
    </div>
  );
}

const styles = stylex.create({
  hint: {
    margin: 0,
    color: "var(--noema-text-muted)",
    fontSize: 10,
    lineHeight: 1.35,
    paddingBottom: 7
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))",
    gap: 8
  },
  card: {
    display: "grid",
    gap: 7,
    minWidth: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-card)",
    padding: 10
  },
  cardHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 6
  },
  cardTitle: {
    margin: 0,
    color: "var(--noema-text-muted)",
    fontSize: 10,
    fontWeight: 700,
    letterSpacing: "0.07em",
    textTransform: "uppercase"
  },
  badge: {
    fontSize: 9
  },
  model: {
    minWidth: 0,
    margin: 0,
    color: "var(--noema-text-primary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    lineHeight: 1.35,
    overflowWrap: "anywhere"
  },
  metadata: {
    display: "grid",
    gap: 4,
    margin: 0
  },
  metadataRow: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr)",
    gap: 8
  },
  metadataLabel: {
    color: "var(--noema-text-muted)",
    fontSize: 10
  },
  metadataValue: {
    minWidth: 0,
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 10,
    overflowWrap: "anywhere"
  }
});
