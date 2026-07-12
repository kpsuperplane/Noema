import { ChatToolCalls, type ChatToolCallStatus } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { TaskStaticSection } from "./TaskSection";
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
      <ChatToolCalls
        calls={[{
          key: criterion.id,
          name: criterion.text,
          target: `Criterion ${criterion.position} · ${criterionVerdictLabel(verdict)}`,
          status: criterionToolCallStatus(verdict),
          errorMessage: verdict === "fail" ? criterion.evidence ?? undefined : undefined,
          resultDetail: hasDetails ? (
            <div {...stylex.props(styles.evidenceGroup)}>
              {criterion.evidence ? (
                <p {...stylex.props(styles.evidence)}>
                  <span {...stylex.props(styles.evidenceLabel)}>Evidence</span>
                  {criterion.evidence}
                </p>
              ) : null}
              {criterion.expectedEvidence ? (
                <p {...stylex.props(styles.evidence)}>
                  <span {...stylex.props(styles.evidenceLabel)}>Expected evidence</span>
                  {criterion.expectedEvidence}
                </p>
              ) : null}
            </div>
          ) : undefined
        }]}
        {...stylex.props(styles.toolCall)}
      />
    </li>
  );
}

function criterionToolCallStatus(verdict: TaskCriterionVerdict): ChatToolCallStatus {
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
  toolCall: {
    width: "100%",
    borderRadius: 8,
    ":hover": {
      backgroundColor: "var(--noema-surface-hover)"
    }
  },
  evidenceGroup: {
    display: "grid",
    gap: 6,
    paddingTop: 2
  },
  evidence: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.4,
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap"
  },
  evidenceLabel: {
    marginInlineEnd: 5,
    color: "var(--noema-text-muted)",
    fontSize: 10,
    fontWeight: 650,
    letterSpacing: "0.06em",
    textTransform: "uppercase"
  },
  empty: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.45
  }
});
