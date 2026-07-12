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
        errorMessage={verdict === "fail" ? criterion.evidence ?? undefined : undefined}
        id={criterion.id}
        input={hasDetails ? {
          expected_evidence: criterion.expectedEvidence,
          ...((verdict === "pending" || verdict === "uncertain")
            ? { observed_evidence: criterion.evidence }
            : {})
        } : undefined}
        name={criterion.text}
        output={hasDetails && (verdict === "pass" || verdict === "fail")
          ? { evidence: criterion.evidence }
          : undefined}
        status={criterionToolCallStatus(verdict)}
        target={`Criterion ${criterion.position} · ${criterionVerdictLabel(verdict)}`}
      />
    </li>
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
  empty: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45
  }
});
