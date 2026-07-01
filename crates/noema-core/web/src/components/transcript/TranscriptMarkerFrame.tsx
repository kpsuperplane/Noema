import type { AriaRole, ButtonHTMLAttributes, ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";

type TranscriptMarkerTone = "default" | "muted" | "success" | "warning" | "error";

type TranscriptMarkerFrameProps = {
  tone?: TranscriptMarkerTone;
  pending?: boolean;
  icon?: ReactNode;
  children: ReactNode;
  role?: AriaRole;
  buttonProps?: ButtonHTMLAttributes<HTMLButtonElement>;
};

const styles = stylex.create({
  root: {
    display: "inline-flex",
    maxWidth: "100%",
    alignItems: "center",
    gap: 8,
    borderRadius: 8,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "transparent",
    backgroundColor: "transparent",
    paddingBlock: 4,
    paddingInline: 8,
    fontSize: 14,
    lineHeight: 1.35,
    color: "var(--noema-text-secondary)",
    textAlign: "left"
  },
  button: {
    appearance: "none",
    cursor: "pointer",
    font: "inherit",
    ":hover": {
      backgroundColor: "var(--noema-surface-hover)"
    },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)"
    }
  },
  muted: {
    color: "var(--noema-text-faint)"
  },
  success: {
    color: "var(--noema-pine-700)"
  },
  warning: {
    color: "var(--noema-clay-600)"
  },
  error: {
    color: "var(--noema-red-700)"
  },
  icon: {
    display: "inline-flex",
    width: 16,
    height: 16,
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 0
  },
  content: {
    display: "inline-flex",
    minWidth: 0,
    flexWrap: "wrap",
    gap: "0 6px",
    overflowWrap: "anywhere"
  },
  pending: {
    opacity: 0.76
  }
});

export function TranscriptMarkerFrame({
  tone = "default",
  pending = false,
  icon,
  children,
  role,
  buttonProps
}: TranscriptMarkerFrameProps) {
  const styleProps = stylex.props(
    styles.root,
    buttonProps && styles.button,
    tone === "muted" && styles.muted,
    tone === "success" && styles.success,
    tone === "warning" && styles.warning,
    tone === "error" && styles.error,
    pending && styles.pending
  );
  const content = (
    <>
      {icon ? (
        <span {...stylex.props(styles.icon)} aria-hidden="true">
          {icon}
        </span>
      ) : null}
      <span {...stylex.props(styles.content)}>{children}</span>
    </>
  );

  if (buttonProps) {
    return (
      <button type="button" {...buttonProps} {...styleProps} data-pending={pending ? "true" : undefined}>
        {content}
      </button>
    );
  }

  return (
    <span role={role} {...styleProps} data-pending={pending ? "true" : undefined}>
      {content}
    </span>
  );
}
