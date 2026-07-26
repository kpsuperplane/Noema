import { IconButton, type IconButtonProps } from "@astryxdesign/core/IconButton";
import * as stylex from "@stylexjs/stylex";
import { Menu } from "lucide-react";
import type { ReactNode } from "react";
import { ShellPageTrack } from "./ShellPageLayout";
import { useShellSurface } from "./ShellSurfaceContext";

export type ShellNavigationLabel = "Memory" | "Settings" | "Tasks";

export function ShellSidebarTrigger({
  navigationLabel
}: {
  navigationLabel: ShellNavigationLabel;
}) {
  const {
    openSidebar,
    sidebarAvailable,
    sidebarOpen,
    sidebarTriggerRef
  } = useShellSurface();

  if (!sidebarAvailable) {
    return null;
  }

  return (
    <IconButton
      ref={sidebarTriggerRef}
      type="button"
      variant="ghost"
      size="md"
      label={`Open ${navigationLabel} navigation`}
      icon={<Menu aria-hidden="true" size={18} />}
      aria-controls="noema-shell-sidebar"
      aria-expanded={sidebarOpen}
      xstyle={iconButtonXStyle(styles.menuButton)}
      onClick={openSidebar}
    />
  );
}

export function ShellSectionHeader({
  actions,
  children,
  description,
  navigationLabel,
  title,
  titleId,
  titleFont = "sans"
}: {
  actions?: ReactNode;
  children?: ReactNode;
  description?: string;
  navigationLabel: ShellNavigationLabel;
  title: string;
  titleId?: string;
  titleFont?: "sans" | "serif";
}) {
  return (
    <header {...stylex.props(styles.header)}>
      <ShellPageTrack>
        <div {...stylex.props(styles.content)}>
          <div {...stylex.props(styles.titleRow)}>
            <ShellSidebarTrigger navigationLabel={navigationLabel} />
            <h1
              id={titleId}
              {...stylex.props(styles.title, titleFont === "serif" && styles.serifTitle)}
            >
              {title}
            </h1>
            {actions ? <div {...stylex.props(styles.actions)}>{actions}</div> : null}
          </div>
          {description ? <p {...stylex.props(styles.description)}>{description}</p> : null}
          {children}
        </div>
      </ShellPageTrack>
    </header>
  );
}

const styles = stylex.create({
  header: {
    width: "100%",
    flexShrink: 0,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)",
    backgroundColor: "var(--background)"
  },
  content: {
    display: "grid",
    gap: "var(--spacing-2)",
    paddingBlock: "var(--spacing-6)",
    "@media (max-width: 760px)": {
      paddingBlock: "var(--spacing-4)"
    }
  },
  titleRow: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-2)"
  },
  actions: {
    display: "flex",
    flexShrink: 0,
    alignItems: "center",
    gap: "var(--spacing-1)",
    marginInlineStart: "auto"
  },
  menuButton: {
    display: "none",
    flexShrink: 0,
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    color: "var(--pine-700)",
    "@media (max-width: 760px)": {
      display: "inline-flex"
    }
  },
  title: {
    minWidth: 0,
    margin: 0,
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 24,
    lineHeight: 1.25,
    letterSpacing: 0,
    overflowWrap: "anywhere"
  },
  serifTitle: {
    fontFamily: "Georgia, 'Times New Roman', serif",
    fontWeight: 400
  },
  description: {
    maxWidth: 620,
    margin: 0,
    color: "var(--muted-foreground)",
    fontSize: 14,
    lineHeight: 1.5,
    overflowWrap: "anywhere"
  }
});

function iconButtonXStyle(...xstyle: unknown[]): IconButtonProps["xstyle"] {
  return xstyle as unknown as IconButtonProps["xstyle"];
}
