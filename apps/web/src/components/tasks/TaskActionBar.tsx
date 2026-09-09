import type { ReactNode } from "react";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

export function TaskActionBar({ label, children, attention }: {
  label: string;
  children: ReactNode;
  attention?: ReactNode;
}) {
  return (
    <VStack as="aside" aria-label={label} gap={0} xstyle={styles.contextDock}>
      {attention}
      <VStack gap={0} xstyle={styles.contextCard}>{children}</VStack>
    </VStack>
  );
}

const styles = stylex.create({
  contextDock: {
    display: "flex",
    flexDirection: "column",
    position: "relative",
    zIndex: 2,
    minWidth: 0,
    minHeight: 0,
    width: "calc(100% - var(--spacing-6) - var(--spacing-6))",
    maxWidth: 760,
    marginInline: "auto",
    marginBlockEnd: "var(--spacing-4)",
    marginBlockStart: "calc(-1 * var(--spacing-3))",
    "--human-intervention-card-radius": "24px",
    "--human-intervention-card-bottom-radius": "0px",
    "--human-intervention-card-overlap": "var(--human-intervention-card-radius)"
  },
  contextCard: {
    display: "flex",
    flexDirection: "column",
    position: "relative",
    zIndex: 2,
    minWidth: 0,
    minHeight: "calc(2 * var(--spacing-6) + var(--spacing-2) + 2 * var(--border-width))",
    justifyContent: "center",
    flex: "0 0 auto",
    marginBlockStart: "var(--spacing-0)",
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: "var(--human-intervention-card-radius)",
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "0 10px 28px color-mix(in srgb, var(--noema-text-primary) 13%, transparent)"
  },
});
