import { IconButton, type IconButtonProps } from "@astryxdesign/core/IconButton";
import { HStack, VStack } from "@astryxdesign/core/Stack";
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
  navigationLabel,
  title,
  titleId,
  titleFont = "sans"
}: {
  actions?: ReactNode;
  children?: ReactNode;
  navigationLabel: ShellNavigationLabel;
  title: string;
  titleId?: string;
  titleFont?: "sans" | "serif";
}) {
  return (
    <header {...stylex.props(styles.header)}>
      <ShellPageTrack>
        <VStack gap={1.5} {...stylex.props(styles.content)}>
          <HStack gap={2} vAlign="center" {...stylex.props(styles.titleRow)}>
            <ShellSidebarTrigger navigationLabel={navigationLabel} />
            <h1
              id={titleId}
              {...stylex.props(styles.title, titleFont === "serif" && styles.serifTitle)}
            >
              {title}
            </h1>
            {actions ? <HStack gap={1} vAlign="center" {...stylex.props(styles.actions)}>{actions}</HStack> : null}
          </HStack>
          {children}
        </VStack>
      </ShellPageTrack>
    </header>
  );
}

const styles = stylex.create({
  header: {
    position: "sticky",
    top: 0,
    zIndex: 3,
    width: "100%",
    flexShrink: 0,
    backgroundColor: "var(--background)",
    "::after": {
      content: "''",
      position: "absolute",
      top: "100%",
      right: 0,
      left: 0,
      height: "var(--spacing-4)",
      pointerEvents: "none",
      backgroundImage: "linear-gradient(to bottom, var(--background), rgb(255 255 255 / 0))"
    }
  },
  content: {
    paddingBlock: "var(--spacing-3) var(--spacing-2)",
    "@media (max-width: 760px)": {
      paddingBlock: "var(--spacing-2)"
    }
  },
  titleRow: {
    minWidth: 0,
  },
  actions: {
    flexShrink: 0,
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
    margin: "var(--spacing-0)",
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
  }
});

function iconButtonXStyle(...xstyle: unknown[]): IconButtonProps["xstyle"] {
  return xstyle as unknown as IconButtonProps["xstyle"];
}
