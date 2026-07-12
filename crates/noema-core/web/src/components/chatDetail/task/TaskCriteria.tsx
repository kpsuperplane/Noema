import * as stylex from "@stylexjs/stylex";
import { TaskStaticSection } from "./TaskSection";
import { TaskToolMarker, type TaskToolMarkerStatus } from "./TaskToolMarker";
import type { TaskCriterion, TaskCriterionVerdict } from "./taskTypes";

export function TaskCriteria({ criteria }: { criteria: readonly TaskCriterion[] }) {
  return (
    <TaskStaticSection count={criteria.length} id="task-criteria-title" title="Validation criteria">
      {criteria.length === 0 ? (
        <p {...stylex.props(styles.empty)}>No validation criteria were recorded.</p>
      ) : (
        <ol {...stylex.props(styles.list)}>
          {criteria.map((criterion) => (
            <CriterionRow key={criterion.id} criterion={criterion} />
          ))}
        </ol>
      )}
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
        status={criterionToolCallStatus(verdict)}
        target={criterionVerdictLabel(verdict)}
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

function criterionVerdictLabel(verdict: TaskCriterionVerdict): string {
  switch (verdict) {
    case "pass":
      return "Passed";
    case "fail":
      return "Failed";
    case "uncertain":
      return "Uncertain";
    case "pending":
      return "Pending";
  }
}

const styles = stylex.create({
  list: {
    display: "grid",
    gap: 4,
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  item: { minWidth: 0 },
  evidence: {
    display: "grid",
    gap: 7,
    maxWidth: 520,
    margin: 0,
    paddingBlock: 2
  },
  evidenceRow: {
    display: "grid",
    gridTemplateColumns: "80px minmax(0, 1fr)",
    alignItems: "baseline",
    gap: 10,
    "@media (max-width: 520px)": {
      gridTemplateColumns: "1fr",
      gap: 2
    }
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
  },
  empty: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45
  }
});
