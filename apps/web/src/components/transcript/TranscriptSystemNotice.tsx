import type { AriaRole, ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";
import { Settings } from "lucide-react";

type TranscriptSystemNoticeTone = "default" | "success" | "warning" | "error";

const styles = stylex.create({
  root: {
    display: "grid",
    width: "100%",
    maxWidth: 760,
    minWidth: 0,
    justifyItems: "center",
    paddingBlock: 4,
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
    alignItems: "baseline",
    justifyContent: "center",
    flexWrap: "wrap",
    gap: "0 6px",
    overflowWrap: "anywhere",
    whiteSpace: "normal",
    wordBreak: "break-word",
    textAlign: "center"
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
  tone = "default"
}: {
  avatar?: ReactNode;
  children: ReactNode;
  role?: AriaRole;
  tone?: TranscriptSystemNoticeTone;
}) {
  const leading = avatar ?? <Settings aria-hidden="true" size={14} strokeWidth={2} />;
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
      <span
        {...stylex.props(styles.content)}
      >
        <span {...stylex.props(styles.avatar)} aria-hidden="true">{leading}</span>
        <span {...stylex.props(styles.message)}>{children}</span>
      </span>
    </div>
  );
}
