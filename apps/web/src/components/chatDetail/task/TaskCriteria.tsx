import * as stylex from "@stylexjs/stylex";
import { TaskStaticSection } from "./TaskSection";
import { TaskToolMarker, type TaskToolMarkerStatus } from "./TaskToolMarker";
import type { TaskCriterion, TaskCriterionVerdict } from "./taskTypes";

export function TaskCriteria({
  criteria,
  embedded = false,
  showTitle = true
}: {
  criteria: readonly TaskCriterion[];
  embedded?: boolean;
  showTitle?: boolean;
}) {
  if (criteria.length === 0) {
    return null;
  }

  const content = (
    <ol {...stylex.props(styles.list)}>
      {criteria.map((criterion) => (
        <CriterionRow key={criterion.id} criterion={criterion} />
      ))}
    </ol>
  );

  if (embedded) {
    return (
      <div {...stylex.props(styles.embedded)}>
        {showTitle ? <h4 {...stylex.props(styles.embeddedTitle)}>Validation criteria</h4> : null}
        {content}
      </div>
    );
  }

  return (
    <TaskStaticSection count={criteria.length} id="task-criteria-title" title="Validation criteria">
      {content}
    </TaskStaticSection>
  );
}

function CriterionRow({ criterion }: { criterion: TaskCriterion }) {
  const verdict = criterion.verdict ?? "pending";
  const hasDetails = Boolean(criterion.evidence || criterion.expectedEvidence);
  return (
    <li {...stylex.props(styles.item)}>
      <TaskToolMarker
        detail={hasDetails ? <CriterionEvidence criterion={criterion} /> : undefined}
        id={criterion.id}
        name={criterion.text}
        presentation="content"
        status={criterionToolCallStatus(verdict)}
      />
    </li>
  );
}

function CriterionEvidence({ criterion }: { criterion: TaskCriterion }) {
  return (
    <dl {...stylex.props(styles.evidence)}>
      {criterion.expectedEvidence ? (
        <div {...stylex.props(styles.evidenceRow)}>
          <dt {...stylex.props(styles.evidenceLabel)}>Expected</dt>
          <dd {...stylex.props(styles.evidenceValue)}>{criterion.expectedEvidence}</dd>
        </div>
      ) : null}
      {criterion.evidence ? (
        <div {...stylex.props(styles.evidenceRow)}>
          <dt {...stylex.props(styles.evidenceLabel)}>Observed</dt>
          <dd {...stylex.props(styles.evidenceValue)}>{criterion.evidence}</dd>
        </div>
      ) : null}
    </dl>
  );
}

function criterionToolCallStatus(verdict: TaskCriterionVerdict): TaskToolMarkerStatus {
  switch (verdict) {
    case "pass":
      return "complete";
    case "fail":
      return "error";
    case "uncertain":
    case "pending":
      return "pending";
  }
}

const styles = stylex.create({
  embedded: {
    display: "grid",
    gap: "var(--spacing-1-5)",
    minWidth: 0,
    paddingTop: "var(--spacing-1)"
  },
  embeddedTitle: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 11,
    fontWeight: 650
  },
  list: {
    display: "grid",
    gap: "var(--spacing-1-5)",
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  item: {
    minWidth: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 7,
    backgroundColor: "var(--noema-surface-sunken)",
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)"
  },
  evidence: {
    display: "grid",
    gap: "var(--spacing-2)",
    maxWidth: 520,
    margin: 0,
    paddingBlock: "var(--spacing-0-5)"
  },
  evidenceRow: {
    display: "grid",
    gap: "var(--spacing-0-5)"
  },
  evidenceLabel: {
    color: "var(--noema-text-muted)",
    fontSize: 10,
    fontWeight: 650
  },
  evidenceValue: {
    minWidth: 0,
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45,
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap"
  }
});
