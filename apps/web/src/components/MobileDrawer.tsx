import * as stylex from "@stylexjs/stylex";
import { useRef, useState, type ReactNode } from "react";
import { Drawer } from "vaul";

export function drawerDimension(value: number | string | undefined): string {
  if (value == null) return "calc(var(--noema-mobile-viewport-height) - var(--spacing-6))";
  return typeof value === "number" ? `${value}px` : value;
}

export function useLatchedDrawerPresentation(open: boolean, drawerViewport: boolean): boolean {
  const [presentation, setPresentation] = useState({ open, drawer: drawerViewport });

  if (presentation.open !== open) {
    setPresentation({
      open,
      drawer: open ? drawerViewport : presentation.drawer
    });
  }

  return presentation.drawer;
}

export function MobileDrawer({
  isOpen,
  onOpenChange,
  label,
  children,
  dismissible = true,
  closeOnEscape = dismissible,
  height,
  maxHeight,
  role = "dialog"
}: {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  label: string;
  children: ReactNode;
  dismissible?: boolean;
  closeOnEscape?: boolean;
  height?: number | string;
  maxHeight?: number | string;
  role?: "dialog" | "alertdialog";
}) {
  const contentRef = useRef<HTMLDivElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);

  return (
    <Drawer.Root
      open={isOpen}
      onOpenChange={onOpenChange}
      dismissible={dismissible}
      handleOnly={!dismissible}
      autoFocus
    >
      <Drawer.Portal>
        <Drawer.Overlay {...stylex.props(styles.overlay)} />
        <Drawer.Content
          ref={contentRef}
          aria-label={label}
          aria-describedby={undefined}
          role={role}
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
            opener.focus({ preventScroll: true });
          }}
          onEscapeKeyDown={(event) => {
            if (dismissible) return;
            event.preventDefault();
            if (closeOnEscape) onOpenChange(false);
          }}
          onPointerDownOutside={(event) => {
            if (!dismissible) event.preventDefault();
          }}
          {...stylex.props(styles.content)}
          style={{
            height: height == null ? undefined : drawerDimension(height),
            maxHeight: drawerDimension(maxHeight ?? height)
          }}
        >
          <Drawer.Title {...stylex.props(styles.visuallyHidden)}>{label}</Drawer.Title>
          {dismissible ? <Drawer.Handle {...stylex.props(styles.handle)} /> : null}
          <div data-slot="mobile-drawer-body" {...stylex.props(styles.body)}>
            {children}
          </div>
        </Drawer.Content>
      </Drawer.Portal>
    </Drawer.Root>
  );
}

const styles = stylex.create({
  overlay: {
    position: "fixed",
    inset: 0,
    zIndex: 1000,
    backgroundColor: "var(--color-overlay)",
    backdropFilter: "blur(2px)",
    animationDuration: "var(--motion-spring-surface-duration)",
    animationTimingFunction: "var(--motion-spring-critical-easing)"
  },
  content: {
    position: "fixed",
    right: 0,
    bottom: {
      default: 0,
      "@media (max-width: 760px) and (display-mode: standalone)":
        "calc(-1 * env(safe-area-inset-bottom, 0px))"
    },
    left: 0,
    zIndex: 1001,
    display: "flex",
    flexDirection: "column",
    minHeight: 0,
    overflow: "hidden",
    outline: "none",
    boxSizing: "border-box",
    borderTopLeftRadius: "var(--radius-container)",
    borderTopRightRadius: "var(--radius-container)",
    backgroundColor: "var(--color-background-surface)",
    boxShadow: "var(--shadow-high)",
    paddingBottom: "max(var(--spacing-2), env(safe-area-inset-bottom))",
    animationDuration: "var(--motion-spring-surface-duration)",
    animationTimingFunction: "var(--motion-spring-critical-easing)",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)"
  },
  handle: {
    width: "var(--spacing-8)",
    height: "var(--spacing-1)",
    flexShrink: 0,
    alignSelf: "center",
    marginBlock: "var(--spacing-2) var(--spacing-1)",
    borderRadius: "var(--radius-pill)",
    backgroundColor: "var(--border-default)"
  },
  body: {
    position: "relative",
    display: "flex",
    flex: "1 1 auto",
    minHeight: 0,
    overflow: "hidden"
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
    borderWidth: 0
  }
});
