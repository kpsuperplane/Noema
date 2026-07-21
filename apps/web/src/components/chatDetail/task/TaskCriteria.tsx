import { Popover } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { Check, CircleAlert, Clock3, X } from "lucide-react";
import * as React from "react";
import { TaskStaticSection } from "./TaskSection";
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
  const hasDetails = Boolean(criterion.expectedEvidence?.trim() || criterion.evidence?.trim());
  return (
    <li {...stylex.props(styles.item)}>
      {hasDetails ? <CriterionDetailsPopover criterion={criterion} /> : <CriterionStaticRow criterion={criterion} />}
    </li>
  );
}

function CriterionDetailsPopover({ criterion }: { criterion: TaskCriterion }) {
  const [open, setOpen] = React.useState(false);
  const pinnedRef = React.useRef(false);
  const hoveringRef = React.useRef(false);
  const closeTimeoutRef = React.useRef<ReturnType<typeof setTimeout> | null>(null);
  const clearCloseTimeout = React.useCallback(() => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current);
      closeTimeoutRef.current = null;
    }
  }, []);
  const scheduleClose = React.useCallback(() => {
    clearCloseTimeout();
    if (pinnedRef.current || hoveringRef.current) {
      return;
    }
    closeTimeoutRef.current = setTimeout(() => {
      closeTimeoutRef.current = null;
      setOpen(false);
    }, 180);
  }, [clearCloseTimeout]);
  React.useEffect(() => () => clearCloseTimeout(), [clearCloseTimeout]);

  return (
    <Popover
      alignment="start"
      content={(
        <div
          onMouseEnter={() => { hoveringRef.current = true; clearCloseTimeout(); }}
          onMouseLeave={() => { hoveringRef.current = false; scheduleClose(); }}
        >
          <CriterionEvidence criterion={criterion} />
        </div>
      )}
      hasAutoFocus={false}
      isOpen={open}
      label={`Validation details: ${criterion.text}`}
      onOpenChange={(next) => {
        if (!next) {
          pinnedRef.current = false;
          setOpen(false);
        }
      }}
      placement="above"
      width="min(360px, calc(100vw - var(--spacing-6)))"
      xstyle={popoverXStyle(styles.criterionPopover)}
    >
      {(trigger) => (
        <button
          ref={(element) => trigger.ref(element)}
          type="button"
          aria-controls={trigger["aria-controls"]}
          aria-expanded={trigger["aria-expanded"]}
          aria-haspopup={trigger["aria-haspopup"]}
          aria-label={`Show details for validation: ${criterion.text}`}
          onBlur={scheduleClose}
          onClick={() => {
            clearCloseTimeout();
            pinnedRef.current = !pinnedRef.current;
            setOpen(pinnedRef.current);
          }}
          onFocus={() => { clearCloseTimeout(); setOpen(true); }}
          onMouseEnter={() => { hoveringRef.current = true; clearCloseTimeout(); setOpen(true); }}
          onMouseLeave={() => { hoveringRef.current = false; scheduleClose(); }}
          {...stylex.props(styles.criterionRow, styles.criterionInteractive)}
        >
          <TaskCriterionStatusIcon verdict={criterion.verdict} />
          <span {...stylex.props(styles.criterionText)}>{criterion.text}</span>
        </button>
      )}
    </Popover>
  );
}

function CriterionStaticRow({ criterion }: { criterion: TaskCriterion }) {
  return (
    <div {...stylex.props(styles.criterionRow)}>
      <TaskCriterionStatusIcon verdict={criterion.verdict} />
      <span {...stylex.props(styles.criterionText)}>{criterion.text}</span>
    </div>
  );
}

export function TaskCriterionStatusIcon({ verdict, size = 15 }: { verdict?: TaskCriterionVerdict | null; size?: number }) {
  if (verdict === "pass") {
    return <Check aria-hidden="true" size={size} strokeWidth={2.2} {...stylex.props(styles.statusIcon, styles.complete)} />;
  }
  if (verdict === "fail") {
    return <X aria-hidden="true" size={size} strokeWidth={2.2} {...stylex.props(styles.statusIcon, styles.error)} />;
  }
  if (verdict === "uncertain") {
    return <CircleAlert aria-hidden="true" size={size} strokeWidth={2} {...stylex.props(styles.statusIcon, styles.uncertain)} />;
  }
  return <Clock3 aria-hidden="true" size={size} strokeWidth={2} {...stylex.props(styles.statusIcon, styles.pending)} />;
}

function CriterionEvidence({ criterion }: { criterion: TaskCriterion }) {
  return (
    <dl {...stylex.props(styles.evidence)}>
      {criterion.expectedEvidence?.trim() ? (
        <div {...stylex.props(styles.evidenceRow)}>
          <dt {...stylex.props(styles.evidenceLabel)}>Expected</dt>
          <dd {...stylex.props(styles.evidenceValue)}>{criterion.expectedEvidence}</dd>
        </div>
      ) : null}
      {criterion.evidence?.trim() ? (
        <div {...stylex.props(styles.evidenceRow)}>
          <dt {...stylex.props(styles.evidenceLabel)}>Seen</dt>
          <dd {...stylex.props(styles.evidenceValue)}>{criterion.evidence}</dd>
        </div>
      ) : null}
    </dl>
  );
}

function popoverXStyle(...xstyle: unknown[]): React.ComponentProps<typeof Popover>["xstyle"] {
  return xstyle as React.ComponentProps<typeof Popover>["xstyle"];
}

const styles = stylex.create({
  embedded: {
    display: "grid",
    gap: "var(--spacing-1-5)",
    minWidth: 0,
    paddingBlockEnd: "var(--spacing-2)",
    paddingInline: "var(--spacing-4)"
  },
  embeddedTitle: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 11,
    fontWeight: 650
  },
  list: {
    display: "grid",
    gap: 0,
    margin: 0,
    padding: 0,
    listStyle: "none"
  },
  item: {
    minWidth: 0
  },
  criterionRow: {
    display: "flex",
    width: "100%",
    minWidth: 0,
    alignItems: "flex-start",
    gap: "var(--spacing-1-5)",
    borderWidth: 0,
    backgroundColor: "transparent",
    paddingBlock: "var(--spacing-1)",
    paddingInline: 0,
    color: "inherit",
    font: "inherit",
    textAlign: "left"
  },
  criterionInteractive: {
    cursor: "pointer",
    ":hover": {
      color: "var(--noema-text-primary)"
    },
    ":focus-visible": {
      borderRadius: 5,
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 40%, transparent)",
      outlineOffset: 2
    }
  },
  criterionText: {
    minWidth: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.4,
    overflowWrap: "anywhere"
  },
  statusIcon: {
    flexShrink: 0,
    marginBlockStart: 1
  },
  pending: {
    color: "var(--noema-text-faint)"
  },
  uncertain: {
    color: "var(--noema-clay-600)"
  },
  complete: {
    color: "var(--noema-pine-600)"
  },
  error: {
    color: "var(--noema-red-700)"
  },
  criterionPopover: {
    maxHeight: "min(50vh, 360px)",
    overflowX: "hidden",
    overflowY: "auto"
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
