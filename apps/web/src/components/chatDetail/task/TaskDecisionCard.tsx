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
  return (
    <section aria-labelledby="task-decision-title" {...stylex.props(styles.card)}>
      <header {...stylex.props(styles.header)}>
        <span {...stylex.props(styles.eyebrow)}>Action required</span>
        <h3 id="task-decision-title" {...stylex.props(styles.title)}>
          {attention.title}
        </h3>
      </header>
      {question ? <p {...stylex.props(styles.question)}>{question}</p> : null}
      <p {...stylex.props(styles.summary)}>{attention.summary}</p>
      {attention.context ? (
        <div {...stylex.props(styles.context)}>
          <Markdown
            autolink="gfm"
            contentWidth="100%"
            density="default"
            headingLevelStart={4}
            xstyle={styles.markdown as unknown as MarkdownXStyle}
          >
            {attention.context}
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
    gap: 10,
    minWidth: 0,
    marginBlock: 8,
    marginInline: 8,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--color-border-orange)",
    borderRadius: 10,
    backgroundColor: "color-mix(in srgb, var(--noema-clay-50) 62%, var(--noema-surface-card))",
    paddingBlock: 13,
    paddingInline: 13,
    boxShadow: "var(--shadow-inset-warning)"
  },
  header: {
    display: "grid",
    gap: 2
  },
  eyebrow: {
    color: "var(--noema-clay-600)",
    fontSize: 10,
    fontWeight: 700,
    letterSpacing: "0.025em"
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
  question: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 13,
    fontWeight: 650,
    lineHeight: 1.45,
    textWrap: "pretty"
  },
  summary: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.5,
    textWrap: "pretty"
  },
  context: {
    minWidth: 0,
    borderLeftWidth: 2,
    borderLeftStyle: "solid",
    borderLeftColor: "color-mix(in srgb, var(--noema-clay-600) 42%, transparent)",
    paddingInlineStart: 9
  },
  markdown: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.5
  },
  response: {
    minWidth: 0,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "color-mix(in srgb, var(--noema-clay-600) 18%, transparent)",
    paddingTop: 11
  }
});
