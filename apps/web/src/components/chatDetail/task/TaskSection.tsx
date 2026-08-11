import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

export function TaskStaticSection({
  id,
  title,
  count,
  tabIndex,
  children
}: {
  id: string;
  title: string;
  count?: number;
  tabIndex?: number;
  children: React.ReactNode;
}) {
  return (
    <VStack as="section" aria-labelledby={id} gap={1.5} className={stylex.props(styles.section).className}>
      <HStack align="center" gap={1.5} className={stylex.props(styles.heading).className}>
        <h3 id={id} tabIndex={tabIndex} {...stylex.props(styles.title)}>{title}</h3>
        {typeof count === "number" ? <span {...stylex.props(styles.count)}>{count}</span> : null}
      </HStack>
      {children}
    </VStack>
  );
}

const styles = stylex.create({
  section: {
    minWidth: 0,
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)"
  },
  heading: {
    minWidth: 0,
  },
  title: {
    margin: "var(--spacing-0)",
    minWidth: 0,
    color: "var(--noema-text-primary)",
    fontSize: 13,
    fontWeight: 700,
    lineHeight: 1.3,
    overflowWrap: "anywhere"
  },
  count: {
    display: "inline-flex",
    minWidth: 18,
    height: 18,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12
  }
});
