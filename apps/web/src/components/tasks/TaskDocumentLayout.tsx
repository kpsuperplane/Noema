import type { ReactNode } from "react";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

export function TaskTitleHeader({ children, close }: { children: ReactNode; close: ReactNode }) {
  return <HStack as="header" align="start" gap={3} xstyle={[styles.column, styles.header]}>
    <VStack gap={0} xstyle={styles.title}>{children}</VStack>
    <HStack xstyle={styles.mobileClose}>{close}</HStack>
  </HStack>;
}

export function TaskDocumentLayout({ children }: { children: ReactNode }) {
  return <VStack gap={3} data-slot="task-document" xstyle={[styles.column, styles.document]}>{children}</VStack>;
}

const styles = stylex.create({
  // Match the task workspace's reading width, including room for field highlights.
  column: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, minWidth: 0, marginInline: "auto" },
  header: { paddingBlockStart: "var(--spacing-3)", paddingBlockEnd: "var(--spacing-1)", pointerEvents: "auto" },
  title: { flexGrow: 1, minWidth: 0 },
  mobileClose: { display: { default: "inline-flex", "@media (min-width: 980px)": "none" } },
  document: { flexGrow: 1, paddingBlockStart: "var(--spacing-4)", paddingBlockEnd: "var(--spacing-6)" }
});
