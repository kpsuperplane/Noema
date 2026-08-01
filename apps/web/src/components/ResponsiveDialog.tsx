import { Dialog as AstryxDialog, DialogHeader } from "@astryxdesign/core/Dialog";
import type { DialogProps } from "@astryxdesign/core/Dialog";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import * as stylex from "@stylexjs/stylex";
import { useRef, useState } from "react";
import { Drawer } from "vaul";

const mobileViewport = "(max-width: 760px)";

function dimension(value: number | string | undefined): string {
  if (value == null) {
    return "calc(100dvh - var(--spacing-6))";
  }
  return typeof value === "number" ? `${value}px` : value;
}

export function Dialog(props: DialogProps) {
  const isMobile = useMediaQuery(mobileViewport);
  const [presentation, setPresentation] = useState({ open: props.isOpen, mobile: isMobile });
  const contentRef = useRef<HTMLDivElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);

  if (presentation.open !== props.isOpen) {
    setPresentation({ open: props.isOpen, mobile: isMobile });
  }

  const renderAsMobile = props.isOpen ? presentation.mobile : isMobile;
  if (!renderAsMobile) {
    return <AstryxDialog {...props} />;
  }

  const {
    isOpen,
    onOpenChange,
    purpose = "info",
    maxHeight,
    padding,
    children,
  } = props;
  const label = props["aria-label"] ?? "Dialog";
  const automaticDismissal = purpose === "info";
  const drawerMaxHeight = dimension(maxHeight);
  const dialogMaxHeight = `calc(${drawerMaxHeight} - var(--spacing-5) - env(safe-area-inset-bottom, 0px))`;

  return (
    <Drawer.Root
      open={isOpen}
      onOpenChange={onOpenChange}
      dismissible={automaticDismissal}
      handleOnly={!automaticDismissal}
      autoFocus
    >
      <Drawer.Portal>
        <Drawer.Overlay {...stylex.props(styles.overlay)} />
        <Drawer.Content
          ref={contentRef}
          aria-label={label}
          aria-describedby={undefined}
          role={purpose === "required" ? "alertdialog" : "dialog"}
          onFocusCapture={(event) => {
            const previous = event.relatedTarget;
            if (
              openerRef.current == null &&
              previous instanceof HTMLElement &&
              !event.currentTarget.contains(previous)
            ) {
              openerRef.current = previous;
            }
          }}
          onOpenAutoFocus={(event) => {
            const active = document.activeElement as HTMLElement | null;
            if (active && !contentRef.current?.contains(active)) openerRef.current = active;
            const target = contentRef.current?.querySelector<HTMLElement>("[data-autofocus]");
            if (!target) return;
            event.preventDefault();
            target.focus();
          }}
          onCloseAutoFocus={(event) => {
            const opener = openerRef.current;
            openerRef.current = null;
            if (!opener?.isConnected) return;
            event.preventDefault();
            opener.focus();
          }}
          onEscapeKeyDown={(event) => {
            if (automaticDismissal) return;
            event.preventDefault();
            if (purpose === "form") onOpenChange(false);
          }}
          onPointerDownOutside={(event) => {
            if (!automaticDismissal) event.preventDefault();
          }}
          {...stylex.props(styles.content)}
          style={{ maxHeight: drawerMaxHeight }}
        >
          <Drawer.Title {...stylex.props(styles.visuallyHidden)}>{label}</Drawer.Title>
          {automaticDismissal ? <Drawer.Handle {...stylex.props(styles.handle)} /> : null}
          <AstryxDialog
            isOpen
            isInline
            onOpenChange={onOpenChange}
            purpose={purpose}
            padding={padding}
            width="100%"
            maxHeight={dialogMaxHeight}
            aria-label={label}
            className={stylex.props(styles.inlineDialog).className}
          >
            {children}
          </AstryxDialog>
        </Drawer.Content>
      </Drawer.Portal>
    </Drawer.Root>
  );
}

export { DialogHeader };
export type { DialogProps };

const styles = stylex.create({
  overlay: {
    position: "fixed",
    inset: 0,
    zIndex: 1000,
    backgroundColor: "var(--color-overlay)",
    backdropFilter: "blur(2px)",
    animationDuration: "var(--motion-spring-surface-duration)",
    animationTimingFunction: "var(--motion-spring-critical-easing)",
  },
  content: {
    position: "fixed",
    right: 0,
    bottom: 0,
    left: 0,
    zIndex: 1001,
    display: "flex",
    flexDirection: "column",
    minHeight: 0,
    overflow: "hidden",
    outline: "none",
    borderTopLeftRadius: "var(--radius-container)",
    borderTopRightRadius: "var(--radius-container)",
    backgroundColor: "var(--color-background-surface)",
    boxShadow: "var(--shadow-high)",
    paddingBottom: "max(var(--spacing-2), env(safe-area-inset-bottom))",
    animationDuration: "var(--motion-spring-surface-duration)",
    animationTimingFunction: "var(--motion-spring-critical-easing)",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
  },
  handle: {
    width: "var(--spacing-8)",
    height: "var(--spacing-1)",
    flexShrink: 0,
    alignSelf: "center",
    marginBlock: "var(--spacing-2) var(--spacing-1)",
    borderRadius: "var(--radius-pill)",
    backgroundColor: "var(--border-default)",
  },
  inlineDialog: {
    width: "100%",
    maxWidth: "none",
    minHeight: 0,
    overflowY: "auto",
    borderRadius: 0,
    boxShadow: "none",
  },
  visuallyHidden: {
    position: "absolute",
    width: 1,
    height: 1,
    margin: -1,
    padding: 0,
    overflow: "hidden",
    clip: "rect(0 0 0 0)",
    whiteSpace: "nowrap",
    borderWidth: 0,
  },
});
