import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { AlertCircle, Check, Circle, HelpCircle } from "lucide-react";
import type { ReactNode } from "react";
import type { TaskCriterion, TaskCriterionVerdict } from "./taskTypes";

export function TaskCriteria({ criteria }: { criteria: readonly TaskCriterion[] }) {
  return (
    <section aria-labelledby="task-criteria-title" {...stylex.props(styles.section)}>
      <SectionHeading id="task-criteria-title" title="Validation criteria" count={criteria.length} />
      {criteria.length === 0 ? (
        <p {...stylex.props(styles.empty)}>No validation criteria were recorded.</p>
      ) : (
        <ol {...stylex.props(styles.list)}>
          {criteria.map((criterion) => (
            <CriterionRow key={criterion.id} criterion={criterion} />
          ))}
        </ol>
      )}
    </section>
  );
}

function CriterionRow({ criterion }: { criterion: TaskCriterion }) {
  const verdict = criterion.verdict ?? "pending";
  const meta = criterionMeta(verdict);
  return (
    <li {...stylex.props(styles.item)}>
      <span {...stylex.props(styles.index)} aria-hidden="true">
        {criterion.position}
      </span>
      <div {...stylex.props(styles.content)}>
        <div {...stylex.props(styles.textRow)}>
          <p {...stylex.props(styles.text)}>{criterion.text}</p>
          <Badge variant={meta.variant} icon={meta.icon} label={meta.label} {...stylex.props(styles.badge)} />
        </div>
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
    </li>
  );
}

export function SectionHeading({ id, title, count, tabIndex }: { id: string; title: string; count?: number; tabIndex?: number }) {
  return (
    <div {...stylex.props(styles.heading)}>
      <h3 id={id} tabIndex={tabIndex} {...stylex.props(styles.headingTitle)}>
        {title}
      </h3>
      {typeof count === "number" ? <span {...stylex.props(styles.headingCount)}>{count}</span> : null}
    </div>
  );
}

function criterionMeta(verdict: TaskCriterionVerdict): {
  label: string;
  variant: "neutral" | "success" | "error" | "warning";
  icon: ReactNode;
} {
  const iconProps = { "aria-hidden": true, size: 12, strokeWidth: 2 } as const;
  switch (verdict) {
    case "pass":
      return { label: "Pass", variant: "success", icon: <Check {...iconProps} /> };
    case "fail":
      return { label: "Fail", variant: "error", icon: <AlertCircle {...iconProps} /> };
    case "uncertain":
      return { label: "Uncertain", variant: "warning", icon: <HelpCircle {...iconProps} /> };
    case "pending":
      return { label: "Pending", variant: "neutral", icon: <Circle {...iconProps} /> };
  }
}

const styles = stylex.create({
  section: {
    display: "grid",
    gap: 11,
    paddingBlock: 2
  },
  heading: {
    display: "flex",
    alignItems: "center",
    gap: 8
  },
  headingTitle: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 13,
    fontWeight: 700,
    lineHeight: 1.35
  },
  headingCount: {
    display: "inline-flex",
    minWidth: 20,
    height: 20,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 999,
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10
  },
  list: {
    display: "grid",
    gap: 8,
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  item: {
    display: "grid",
    gridTemplateColumns: "24px minmax(0, 1fr)",
    gap: 10,
    minWidth: 0,
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-sunken)",
    padding: 10
  },
  index: {
    display: "inline-flex",
    width: 24,
    height: 24,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 7,
    backgroundColor: "var(--noema-surface-card)",
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 11
  },
  content: {
    display: "grid",
    gap: 6,
    minWidth: 0
  },
  textRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "start",
    justifyContent: "space-between",
    gap: 8,
    minWidth: 0
  },
  text: {
    flex: "1 1 180px",
    minWidth: 0,
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 13,
    lineHeight: 1.45,
    overflowWrap: "anywhere"
  },
  badge: {
    flexShrink: 0,
    fontSize: 10
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
