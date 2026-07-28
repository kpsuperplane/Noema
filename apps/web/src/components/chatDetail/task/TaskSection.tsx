import * as stylex from "@stylexjs/stylex";
import * as React from "react";

export function TaskStaticSection({
  id,
  title,
  count,
  tabIndex,
  children
}: {
  id: string;
  title: string;
  count?: number;
  tabIndex?: number;
  children: React.ReactNode;
}) {
  return (
    <section aria-labelledby={id} {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.heading)}>
        <h3 id={id} tabIndex={tabIndex} {...stylex.props(styles.title)}>{title}</h3>
        {typeof count === "number" ? <span {...stylex.props(styles.count)}>{count}</span> : null}
      </div>
      {children}
    </section>
  );
}

export function TaskExpandableContent({
  id,
  children
}: {
  id: string;
  children: React.ReactNode;
}) {
  const [expanded, setExpanded] = React.useState(false);
  const [truncated, setTruncated] = React.useState(false);
  const contentRef = React.useRef<HTMLDivElement>(null);

  React.useLayoutEffect(() => {
    const content = contentRef.current;
    if (!content || expanded) {
      return;
    }
    const updateTruncation = () => {
      setTruncated(content.scrollHeight > content.clientHeight + 1);
    };
    updateTruncation();
    if (typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(updateTruncation);
    observer.observe(content);
    return () => observer.disconnect();
  }, [children, expanded]);

  return (
    <div {...stylex.props(styles.expandable)}>
      <div
        ref={contentRef}
        id={id}
        {...stylex.props(styles.expandableContent, expanded && styles.expandableContentExpanded)}
      >
        {children}
      </div>
      {expanded || truncated ? (
        <button
          type="button"
          aria-controls={id}
          aria-expanded={expanded}
          onClick={() => setExpanded((current) => !current)}
          {...stylex.props(styles.readMore, expanded && styles.readMoreExpanded)}
        >
          {expanded ? "Show Less" : "Read More"}
        </button>
      ) : null}
    </div>
  );
}

const styles = stylex.create({
  section: {
    display: "grid",
    gap: "var(--spacing-1-5)",
    minWidth: 0,
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)"
  },
  heading: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-1-5)"
  },
  title: {
    margin: "var(--spacing-0)",
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
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 9
  },
  expandableContent: {
    maxHeight: "4.65em",
    minWidth: 0,
    overflow: "hidden",
    fontSize: 13
  },
  expandable: {
    position: "relative",
    minWidth: 0
  },
  expandableContentExpanded: {
    maxHeight: "none",
    overflow: "visible"
  },
  readMore: {
    position: "absolute",
    right: 0,
    bottom: 0,
    width: "fit-content",
    borderWidth: 0,
    borderRadius: 0,
    backgroundColor: "transparent",
    backgroundImage: "linear-gradient(90deg, transparent, var(--noema-surface-card) 18px)",
    paddingBlock: "calc(var(--spacing-0-5) - 1px)",
    paddingInlineEnd: 2,
    paddingInlineStart: 22,
    color: "var(--noema-text-muted)",
    font: "inherit",
    fontSize: 10,
    fontWeight: 650,
    cursor: "pointer",
    ":hover": {
      color: "var(--noema-text-secondary)"
    },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)"
    }
  },
  readMoreExpanded: {
    position: "static",
    display: "block",
    marginTop: "var(--spacing-1)",
    borderRadius: 5,
    backgroundImage: "none",
    paddingBlockEnd: 3,
    paddingBlockStart: 3,
    paddingInlineEnd: 4,
    paddingInlineStart: 4,
    ":hover": {
      backgroundColor: "var(--noema-surface-hover)"
    }
  }
});
