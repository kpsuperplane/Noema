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
    <>
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
          {...stylex.props(styles.readMore)}
        >
          {expanded ? "Show Less" : "Read More"}
        </button>
      ) : null}
    </>
  );
}

const styles = stylex.create({
  section: {
    display: "grid",
    gap: 9,
    minWidth: 0,
    paddingBlock: 10,
    paddingInline: 8
  },
  heading: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: 7
  },
  title: {
    margin: 0,
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
  expandableContent: {
    maxHeight: "4.65em",
    minWidth: 0,
    overflow: "hidden",
    fontSize: 13
  },
  expandableContentExpanded: {
    maxHeight: "none",
    overflow: "visible"
  },
  readMore: {
    width: "fit-content",
    borderWidth: 0,
    borderRadius: 5,
    backgroundColor: "transparent",
    paddingBlock: 3,
    paddingInline: 4,
    color: "var(--noema-text-muted)",
    font: "inherit",
    fontSize: 10,
    fontWeight: 650,
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
  }
});
