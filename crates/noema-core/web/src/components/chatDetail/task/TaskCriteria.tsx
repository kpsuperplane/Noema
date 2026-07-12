import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { AlertCircle, Check, ChevronDown, Circle, HelpCircle } from "lucide-react";
import * as React from "react";
import type { ReactNode } from "react";
import { TaskStaticSection } from "./TaskDisclosureSection";
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
  const meta = criterionMeta(verdict);
  const hasDetails = Boolean(criterion.evidence || criterion.expectedEvidence);
  const [expanded, setExpanded] = React.useState(false);
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
        {hasDetails ? (
          <button
            type="button"
            aria-expanded={expanded}
            onClick={() => setExpanded((current) => !current)}
            {...stylex.props(styles.detailsButton)}
          >
            <span>{expanded ? "Hide details" : "Show details"}</span>
            <ChevronDown
              aria-hidden="true"
              size={13}
              {...stylex.props(styles.detailsChevron, expanded && styles.detailsChevronExpanded)}
            />
          </button>
        ) : null}
        {expanded ? (
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
        ) : null}
      </div>
    </li>
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
    gap: 5,
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
  detailsButton: {
    display: "inline-flex",
    width: "fit-content",
    alignItems: "center",
    gap: 3,
    borderWidth: 0,
    borderRadius: 5,
    backgroundColor: "transparent",
    paddingBlock: 3,
    paddingInline: 4,
    color: "var(--noema-text-muted)",
    font: "inherit",
    fontSize: 10,
    fontWeight: 600,
    cursor: "pointer",
    ":hover": {
      backgroundColor: "var(--noema-surface-hover)",
      color: "var(--noema-text-secondary)"
    },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)"
    }
  },
  detailsChevron: {
    transition: "transform 140ms ease"
  },
  detailsChevronExpanded: {
    transform: "rotate(180deg)"
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
