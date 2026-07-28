import type { ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";

export function SetupFrame({
  children,
  subtitle = "Local setup"
}: {
  children: ReactNode;
  subtitle?: string;
}) {
  return (
    <main {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}>
        <img src="/assets/noema-mark.svg" width="32" height="32" alt="" />
        <div {...stylex.props(styles.headerText)}>
          <strong {...stylex.props(styles.title)}>Noema</strong>
          <span {...stylex.props(styles.subtitle)}>{subtitle}</span>
        </div>
      </div>
      <div {...stylex.props(styles.body)}>{children}</div>
    </main>
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
    display: "grid",
    height: "100dvh",
    gridTemplateRows: "64px minmax(0, 1fr)",
    overflow: "hidden",
    backgroundColor: "var(--background)"
  },
  header: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "calc(var(--spacing-2) + var(--spacing-0-5))",
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
    minHeight: 0,
    overflow: "auto"
  }
});
