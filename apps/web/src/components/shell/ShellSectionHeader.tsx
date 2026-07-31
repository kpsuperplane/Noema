import { HStack, VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import { ShellPageTrack } from "./ShellPageLayout";
import { useShellSurface } from "./ShellSurfaceContext";

export function ShellSectionHeader({
  actions,
  children,
  title,
  titleId,
  titleFont = "sans"
}: {
  actions?: ReactNode;
  children?: ReactNode;
  title: string;
  titleId?: string;
  titleFont?: "sans" | "serif";
}) {
  const { sidebarAvailable } = useShellSurface();
  const shellOwnsMobileTitle = sidebarAvailable;
  const headerOnlyContainsTitle = !actions && !children;

  return (
    <header
      {...stylex.props(
        styles.header,
        shellOwnsMobileTitle && headerOnlyContainsTitle && styles.mobileTitleOnlyHeader
      )}
    >
      <ShellPageTrack>
        <VStack gap={1.5} {...stylex.props(styles.content)}>
          <HStack gap={2} vAlign="center" {...stylex.props(styles.titleRow)}>
            <h1
              id={titleId}
              {...stylex.props(
                styles.title,
                titleFont === "serif" && styles.serifTitle,
                shellOwnsMobileTitle && styles.mobileTitleInShell
              )}
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
  mobileTitleOnlyHeader: {
    "@media (max-width: 760px)": {
      display: "none"
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
  mobileTitleInShell: {
    "@media (max-width: 760px)": {
      display: "none"
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
