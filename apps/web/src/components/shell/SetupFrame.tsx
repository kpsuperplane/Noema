import type { ReactNode } from "react";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";

export function SetupFrame({
  children,
  subtitle = "Local setup"
}: {
  children: ReactNode;
  subtitle?: string;
}) {
  return (
    <VStack as="main" height="100dvh" {...stylex.props(styles.root)}>
      <HStack gap={3} vAlign="center" {...stylex.props(styles.header)}>
        <img src="/assets/noema-mark.svg" width="32" height="32" alt="" />
        <VStack gap={0} {...stylex.props(styles.headerText)}>
          <strong {...stylex.props(styles.title)}>Noema</strong>
          <span {...stylex.props(styles.subtitle)}>{subtitle}</span>
        </VStack>
      </HStack>
      <VStack as="div" {...stylex.props(styles.body)}>{children}</VStack>
    </VStack>
  );
}

const truncatedText = {
  display: "block",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap"
} as const;

const styles = stylex.create({
  root: {
    overflow: "hidden",
    backgroundColor: "var(--background)"
  },
  header: {
    minWidth: 0,
    flexShrink: 0,
    height: 64,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)",
    backgroundColor: "rgba(255, 255, 255, 0.95)",
    paddingInline: "var(--spacing-5)"
  },
  headerText: {
    minWidth: 0
  },
  title: {
    ...truncatedText,
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    letterSpacing: 0
  },
  subtitle: {
    ...truncatedText,
    fontSize: 12,
    color: "var(--muted-foreground)"
  },
  body: {
    flex: 1,
    minHeight: 0,
    overflow: "auto"
  }
});
