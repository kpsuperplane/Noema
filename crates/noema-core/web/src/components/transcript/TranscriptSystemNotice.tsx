import type { AriaRole, ReactNode } from "react";
import { ChatSystemMessage, type ChatSystemMessageProps } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";

type TranscriptSystemNoticeTone = "default" | "success" | "warning" | "error";
type ChatSystemMessageXStyle = ChatSystemMessageProps["xstyle"];

const styles = stylex.create({
  root: {
    width: "100%",
    maxWidth: 760,
    minWidth: 0
  },
  content: {
    display: "inline-flex",
    maxWidth: "100%",
    alignItems: "baseline",
    justifyContent: "center",
    flexWrap: "wrap",
    gap: "0 6px",
    overflowWrap: "anywhere",
    textAlign: "center"
  },
  label: {
    fontWeight: 650
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
  label,
  children,
  role,
  tone = "default"
}: {
  label?: ReactNode;
  children: ReactNode;
  role?: AriaRole;
  tone?: TranscriptSystemNoticeTone;
}) {
  return (
    <ChatSystemMessage role={role} xstyle={chatSystemMessageXStyle(styles.root)}>
      <span
        {...stylex.props(
          styles.content,
          tone === "default" && styles.defaultTone,
          tone === "success" && styles.successTone,
          tone === "warning" && styles.warningTone,
          tone === "error" && styles.errorTone
        )}
      >
        {label ? <strong {...stylex.props(styles.label)}>{label}</strong> : null}
        <span>{children}</span>
      </span>
    </ChatSystemMessage>
  );
}

function chatSystemMessageXStyle(xstyle: unknown): ChatSystemMessageXStyle {
  return xstyle as unknown as ChatSystemMessageXStyle;
}
