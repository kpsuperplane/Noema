import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import * as React from "react";

export function TaskDisclosureSection({
  id,
  title,
  summary,
  count,
  children,
  defaultExpanded = false,
  expanded,
  headingTabIndex,
  onExpandedChange
}: {
  id: string;
  title: string;
  summary?: React.ReactNode;
  count?: number;
  children: React.ReactNode;
  defaultExpanded?: boolean;
  expanded?: boolean;
  headingTabIndex?: number;
  onExpandedChange?: (expanded: boolean) => void;
}) {
  const [internalState, setInternalState] = React.useState({
    defaultExpanded,
    expanded: defaultExpanded
  });
  const internalExpanded = internalState.defaultExpanded === defaultExpanded
    ? internalState.expanded
    : defaultExpanded;
  const isExpanded = expanded ?? internalExpanded;
  const contentId = `${id}-content`;

  const toggle = () => {
    const nextExpanded = !isExpanded;
    if (expanded === undefined) {
      setInternalState({ defaultExpanded, expanded: nextExpanded });
    }
    onExpandedChange?.(nextExpanded);
  };

  return (
    <section aria-labelledby={id} {...stylex.props(styles.section)}>
      <button
        type="button"
        aria-controls={contentId}
        aria-expanded={isExpanded}
        onClick={toggle}
        {...stylex.props(styles.trigger)}
      >
        <span {...stylex.props(styles.copy)}>
          <span {...stylex.props(styles.titleRow)}>
            <span id={id} role="heading" aria-level={3} tabIndex={headingTabIndex} {...stylex.props(styles.title)}>
              {title}
            </span>
            {typeof count === "number" ? <span {...stylex.props(styles.count)}>{count}</span> : null}
          </span>
          {summary ? <span {...stylex.props(styles.summary)}>{summary}</span> : null}
        </span>
        <span {...stylex.props(styles.toggle)}>
          <span {...stylex.props(styles.toggleLabel)}>{isExpanded ? "Show less" : "Show more"}</span>
          <ChevronDown
            aria-hidden="true"
            size={14}
            {...stylex.props(styles.chevron, isExpanded && styles.chevronExpanded)}
          />
        </span>
      </button>
      <div id={contentId} hidden={!isExpanded} {...stylex.props(styles.body)}>
        {children}
      </div>
    </section>
  );
}

const styles = stylex.create({
  section: {
    minWidth: 0,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)"
  },
  trigger: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    width: "100%",
    minWidth: 0,
    alignItems: "center",
    gap: 10,
    borderWidth: 0,
    borderRadius: 8,
    backgroundColor: "transparent",
    paddingBlock: 9,
    paddingInline: 8,
    color: "var(--noema-text-primary)",
    font: "inherit",
    textAlign: "left",
    cursor: "pointer",
    transitionDuration: "140ms",
    transitionProperty: "background-color, transform",
    ":hover": {
      backgroundColor: "var(--noema-surface-hover)"
    },
    ":active": {
      transform: "translateY(1px)"
    },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)",
      outlineOffset: -2
    }
  },
  copy: {
    display: "grid",
    minWidth: 0,
    gap: 3
  },
  titleRow: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: 7
  },
  title: {
    minWidth: 0,
    color: "var(--noema-text-primary)",
    fontSize: 13,
    fontWeight: 700,
    lineHeight: 1.3,
    overflowWrap: "anywhere"
  },
  count: {
    display: "inline-flex",
    minWidth: 18,
    height: 18,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 999,
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 9
  },
  summary: {
    display: "-webkit-box",
    minWidth: 0,
    overflow: "hidden",
    color: "var(--noema-text-secondary)",
    fontSize: 11,
    lineHeight: 1.35,
    overflowWrap: "anywhere",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: 2
  },
  toggle: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "end",
    gap: 4,
    color: "var(--noema-text-muted)",
    whiteSpace: "nowrap"
  },
  toggleLabel: {
    fontSize: 10,
    fontWeight: 600
  },
  chevron: {
    transition: "transform 140ms ease"
  },
  chevronExpanded: {
    transform: "rotate(180deg)"
  },
  body: {
    minWidth: 0,
    paddingBlockEnd: 11,
    paddingInline: 8
  }
});
