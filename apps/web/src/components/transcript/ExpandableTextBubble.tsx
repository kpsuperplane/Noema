import * as React from "react";
import * as stylex from "@stylexjs/stylex";

const interactiveContentSelector = [
  "a[href]",
  "audio[controls]",
  "button",
  "input",
  "label",
  "select",
  "summary",
  "textarea",
  "video[controls]",
  "[contenteditable]:not([contenteditable='false'])",
  "[role='button']",
  "[role='link']",
  "[tabindex]:not([tabindex='-1'])"
].join(",");

const styles = stylex.create({
  root: {
    position: "relative",
    minWidth: 0
  },
  preview: {
    maxHeight: "10.2em",
    overflow: "hidden"
  },
  expanded: {
    maxHeight: "none",
    overflow: "visible"
  },
  faded: {
    maskImage: "linear-gradient(to bottom, black 0, black calc(100% - 2em), transparent 100%)",
    WebkitMaskImage: "linear-gradient(to bottom, black 0, black calc(100% - 2em), transparent 100%)"
  },
  trigger: {
    position: "absolute",
    insetBlock: "calc(-1 * var(--spacing-2))",
    insetInline: "calc(-1 * var(--spacing-4))",
    padding: 0,
    appearance: "none",
    backgroundColor: "transparent",
    borderWidth: 0,
    cursor: "pointer",
    outline: "none"
  },
  expandedTrigger: {
    pointerEvents: "none"
  }
});

export function ExpandableTextBubbleContent({
  children,
  onOverflowChange
}: {
  children: React.ReactNode;
  onOverflowChange: (overflowing: boolean) => void;
}) {
  const previewRef = React.useRef<HTMLDivElement>(null);
  const contentRef = React.useRef<HTMLDivElement>(null);
  const overflowRef = React.useRef(false);
  const [expanded, setExpanded] = React.useState(false);
  const [overflowing, setOverflowing] = React.useState(false);

  const measure = React.useCallback(() => {
    if (expanded) {
      return;
    }
    const preview = previewRef.current;
    const content = contentRef.current;
    if (!preview || !content) {
      return;
    }
    const nextOverflowing = content.scrollHeight > preview.clientHeight + 1;
    if (overflowRef.current === nextOverflowing) {
      return;
    }
    overflowRef.current = nextOverflowing;
    setOverflowing(nextOverflowing);
    onOverflowChange(nextOverflowing);
  }, [expanded, onOverflowChange]);

  React.useLayoutEffect(() => {
    measure();
  }, [children, measure]);

  React.useLayoutEffect(() => {
    if (typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(measure);
    if (previewRef.current) {
      observer.observe(previewRef.current);
    }
    if (contentRef.current) {
      observer.observe(contentRef.current);
    }
    return () => observer.disconnect();
  }, [measure]);

  const toggleExpanded = () => setExpanded((current) => !current);
  const collapseFromContent = (event: React.MouseEvent<HTMLDivElement>) => {
    if (!expanded || !(event.target instanceof Element)) {
      return;
    }
    if (!event.target.closest(interactiveContentSelector)) {
      setExpanded(false);
    }
  };

  return (
    <div onClick={collapseFromContent} {...stylex.props(styles.root)}>
      <div
        ref={previewRef}
        {...stylex.props(styles.preview, expanded && styles.expanded, overflowing && !expanded && styles.faded)}
      >
        <div ref={contentRef}>{children}</div>
      </div>
      {overflowing ? (
        <button
          aria-expanded={expanded}
          aria-label={expanded ? "Collapse message" : "Expand message"}
          onClick={toggleExpanded}
          type="button"
          {...stylex.props(styles.trigger, expanded && styles.expandedTrigger)}
        />
      ) : null}
    </div>
  );
}
