import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import type { TaskDetail } from "./taskTypes";

type MarkdownXStyle = MarkdownProps["xstyle"];

export function TaskDecisionCard({
  attention,
  question,
  children
}: {
  attention: NonNullable<TaskDetail["attention"]>;
  question?: string | null;
  children?: ReactNode;
}) {
  const prompt = question?.trim() || null;
  const context = attention.context?.trim() || null;
  const contextIsPrimary = Boolean(context)
    && (attention.kind === "RECOVERY_REQUIRED" || !prompt);
  const primaryText = contextIsPrimary ? null : prompt || (context ? null : attention.summary);

  return (
    <section aria-labelledby="task-decision-title" {...stylex.props(styles.card)}>
      <h3 id="task-decision-title" {...stylex.props(styles.title)}>
        {attention.title}
      </h3>
      {primaryText ? <p {...stylex.props(styles.primaryText)}>{primaryText}</p> : null}
      {context ? (
        <div {...stylex.props(styles.context)}>
          <Markdown
            autolink="gfm"
            contentWidth="100%"
            density="default"
            headingLevelStart={4}
            xstyle={(contextIsPrimary
              ? styles.primaryMarkdown
              : styles.supportingMarkdown) as unknown as MarkdownXStyle}
          >
            {context}
          </Markdown>
        </div>
      ) : null}
      {children ? <div {...stylex.props(styles.response)}>{children}</div> : null}
    </section>
  );
}

const styles = stylex.create({
  card: {
    display: "grid",
    gap: "var(--spacing-2)",
    minWidth: 0,
    marginBlock: "var(--spacing-2)",
    marginInline: "var(--spacing-2)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--color-border-orange)",
    borderRadius: 10,
    backgroundColor: "color-mix(in srgb, var(--noema-clay-50) 62%, var(--noema-surface-card))",
    paddingBlock: "var(--spacing-3)",
    paddingInline: "var(--spacing-3)",
    boxShadow: "var(--shadow-inset-warning)"
  },
  title: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontFamily: "var(--noema-font-display)",
    fontSize: 15,
    fontWeight: 700,
    lineHeight: 1.3,
    textWrap: "balance"
  },
  primaryText: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 14,
    fontWeight: 600,
    lineHeight: 1.5,
    textWrap: "pretty"
  },
  context: { minWidth: 0 },
  primaryMarkdown: {
    color: "var(--noema-text-primary)",
    fontSize: 14,
    fontWeight: 600,
    lineHeight: 1.5
  },
  supportingMarkdown: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.5
  },
  response: {
    minWidth: 0,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "color-mix(in srgb, var(--noema-clay-600) 18%, transparent)",
    paddingTop: "var(--spacing-3)"
  }
});
