import type { AriaRole, ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";
import { Activity } from "lucide-react";

type TranscriptSystemNoticeTone = "default" | "success" | "warning" | "error";

const styles = stylex.create({
  root: {
    display: "flex",
    width: "100%",
    maxWidth: "none",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-3)",
    paddingBlock: "var(--spacing-1)",
    fontFamily: "var(--noema-font-body)",
    fontSize: 13,
    fontWeight: 400,
    lineHeight: 1.5,
    overflowWrap: "anywhere",
    textAlign: "center",
    wordBreak: "break-word"
  },
  content: {
    display: "inline-flex",
    maxWidth: "100%",
    minWidth: 0,
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 1,
    flexWrap: "wrap",
    gap: "0 var(--spacing-1-5)",
    overflowWrap: "anywhere",
    whiteSpace: "normal",
    wordBreak: "break-word",
    textAlign: "center"
  },
  rule: {
    height: 1,
    minWidth: "var(--spacing-4)",
    flexBasis: 0,
    flexGrow: 1,
    backgroundColor: "var(--noema-border-subtle)"
  },
  avatar: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 0,
    alignSelf: "center"
  },
  message: {
    minWidth: 0,
    maxWidth: "100%",
    overflowWrap: "anywhere",
    whiteSpace: "normal",
    wordBreak: "break-word"
  },
  singleLineContent: {
    flexWrap: "nowrap"
  },
  singleLineMessage: {
    overflow: "hidden",
    overflowWrap: "normal",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    wordBreak: "normal"
  },
  defaultTone: {
    color: "var(--noema-text-secondary)"
  },
  successTone: {
    color: "var(--noema-pine-700)"
  },
  warningTone: {
    color: "var(--noema-clay-600)"
  },
  errorTone: {
    color: "var(--noema-red-700)"
  }
});

export function TranscriptSystemNotice({
  avatar,
  children,
  role,
  singleLine = false,
  tone = "default"
}: {
  avatar?: ReactNode;
  children: ReactNode;
  role?: AriaRole;
  singleLine?: boolean;
  tone?: TranscriptSystemNoticeTone;
}) {
  const leading = avatar ?? <Activity aria-hidden="true" size={14} strokeWidth={2} />;
  return (
    <div
      {...stylex.props(
        styles.root,
        tone === "default" && styles.defaultTone,
        tone === "success" && styles.successTone,
        tone === "warning" && styles.warningTone,
        tone === "error" && styles.errorTone
      )}
      role={role ?? "status"}
    >
      <span {...stylex.props(styles.rule)} aria-hidden="true" />
      <span {...stylex.props(styles.content, singleLine && styles.singleLineContent)}>
        <span {...stylex.props(styles.avatar)} aria-hidden="true">{leading}</span>
        <span {...stylex.props(styles.message, singleLine && styles.singleLineMessage)}>{children}</span>
      </span>
      <span {...stylex.props(styles.rule)} aria-hidden="true" />
    </div>
  );
}
