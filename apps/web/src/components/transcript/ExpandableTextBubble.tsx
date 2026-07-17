import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog } from "@astryxdesign/core/Dialog";
import { Icon } from "@astryxdesign/core/Icon";
import * as stylex from "@stylexjs/stylex";

const styles = stylex.create({
  root: {
    position: "relative",
    minWidth: 0
  },
  preview: {
    maxHeight: "10.2em",
    overflow: "hidden"
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
  closeRow: {
    display: "flex",
    justifyContent: "flex-end",
    paddingBlock: 8,
    paddingInline: 10
  },
  dialogBody: {
    minHeight: 0,
    maxHeight: "min(70vh, 720px)",
    padding: 18,
    overflow: "auto",
    color: "var(--noema-text-primary)",
    fontSize: 14,
    lineHeight: 1.7,
    overflowWrap: "break-word",
    wordBreak: "break-word"
  }
});

export function ExpandableTextBubbleContent({
  children,
  onOpen,
  onOverflowChange
}: {
  children: React.ReactNode;
  onOpen: () => void;
  onOverflowChange: (overflowing: boolean) => void;
}) {
  const previewRef = React.useRef<HTMLDivElement>(null);
  const contentRef = React.useRef<HTMLDivElement>(null);
  const overflowRef = React.useRef(false);
  const [overflowing, setOverflowing] = React.useState(false);

  const measure = React.useCallback(() => {
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
  }, [onOverflowChange]);

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

  return (
    <div {...stylex.props(styles.root)}>
      <div ref={previewRef} {...stylex.props(styles.preview, overflowing && styles.faded)}>
        <div ref={contentRef}>{children}</div>
      </div>
      {overflowing ? (
        <button
          aria-haspopup="dialog"
          aria-label="View full message"
          onClick={onOpen}
          type="button"
          {...stylex.props(styles.trigger)}
        />
      ) : null}
    </div>
  );
}

export function TextBubbleDialog({
  children,
  open,
  title,
  onOpenChange
}: {
  children: React.ReactNode;
  open: boolean;
  title: string;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog
      aria-label={title}
      isOpen={open}
      maxHeight="80vh"
      onOpenChange={onOpenChange}
      padding={0}
      purpose="info"
      width={760}
    >
      <div {...stylex.props(styles.closeRow)}>
        <Button
          icon={<Icon color="inherit" icon="close" />}
          isIconOnly
          label="Close"
          onClick={() => onOpenChange(false)}
          tooltip="Close"
          variant="ghost"
        />
      </div>
      <div {...stylex.props(styles.dialogBody)}>{children}</div>
    </Dialog>
  );
}
