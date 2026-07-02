import type { ButtonHTMLAttributes, ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";

type TranscriptAttachmentTone = "default" | "success" | "info" | "error";

type TranscriptAttachmentCardProps = {
  id?: string;
  title: ReactNode;
  description?: ReactNode;
  meta?: ReactNode;
  icon?: ReactNode;
  tone?: TranscriptAttachmentTone;
  children?: ReactNode;
  headerButtonProps?: ButtonHTMLAttributes<HTMLButtonElement>;
};

const styles = stylex.create({
  root: {
    display: "grid",
    maxWidth: "100%",
    minWidth: 0,
    gap: 8,
    borderRadius: 8,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    backgroundColor: "var(--noema-surface-card)",
    padding: 12,
    color: "var(--noema-text-primary)"
  },
  header: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    alignItems: "center",
    gap: 10,
    minWidth: 0
  },
  headerWithoutIcon: {
    gridTemplateColumns: "minmax(0, 1fr) auto"
  },
  headerButton: {
    appearance: "none",
    width: "100%",
    borderWidth: 0,
    borderRadius: 8,
    backgroundColor: "transparent",
    color: "inherit",
    font: "inherit",
    margin: -4,
    padding: 4,
    textAlign: "left",
    ":hover": {
      backgroundColor: "var(--noema-surface-hover)"
    },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)"
    }
  },
  icon: {
    display: "inline-flex",
    width: 28,
    height: 28,
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 0,
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-pine-700)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    fontWeight: 700
  },
  info: {
    color: "var(--noema-blue-700)"
  },
  error: {
    color: "var(--noema-red-700)"
  },
  text: {
    minWidth: 0,
    display: "grid",
    gap: 2
  },
  title: {
    minWidth: 0,
    margin: 0,
    fontSize: 14,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--noema-text-primary)",
    overflowWrap: "anywhere"
  },
  description: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 13,
    lineHeight: 1.35,
    overflowWrap: "anywhere"
  },
  meta: {
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 11,
    fontStyle: "normal",
    overflowWrap: "anywhere"
  },
  body: {
    minWidth: 0,
    paddingTop: 4
  }
});

export function TranscriptAttachmentCard({
  id,
  title,
  description,
  meta,
  icon,
  tone = "default",
  children,
  headerButtonProps
}: TranscriptAttachmentCardProps) {
  const header = (
    <div {...stylex.props(styles.header, !icon && styles.headerWithoutIcon)}>
      {icon ? (
        <span {...stylex.props(styles.icon, tone === "info" && styles.info, tone === "error" && styles.error)} aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span {...stylex.props(styles.text)}>
        <span {...stylex.props(styles.title)}>{title}</span>
        {description ? <span {...stylex.props(styles.description)}>{description}</span> : null}
      </span>
      {meta ? <em {...stylex.props(styles.meta)}>{meta}</em> : null}
    </div>
  );

  return (
    <article id={id} {...stylex.props(styles.root)} data-tone={tone}>
      {headerButtonProps ? (
        <button type="button" {...headerButtonProps} {...stylex.props(styles.headerButton)}>
          {header}
        </button>
      ) : (
        header
      )}
      {children ? <div {...stylex.props(styles.body)}>{children}</div> : null}
    </article>
  );
}
