import * as React from "react";
import { ChatSystemMessage } from "@astryxdesign/core/Chat";
import { CodeBlock } from "@astryxdesign/core/CodeBlock";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";

const styles = stylex.create({
  trigger: {
    maxWidth: "min(100%, 680px)",
    padding: 0,
    overflow: "hidden",
    color: "inherit",
    font: "inherit",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    appearance: "none",
    backgroundColor: "transparent",
    borderWidth: 0,
    cursor: "pointer",
    textDecoration: {
      default: "none",
      ":hover": "underline"
    },
    textUnderlineOffset: 3,
    outline: {
      default: "none",
      ":focus-visible": "2px solid var(--color-accent)"
    },
    outlineOffset: 3,
    borderRadius: 2
  },
  body: {
    minHeight: 0,
    padding: 16,
    overflow: "auto"
  },
  text: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    lineHeight: 1.55,
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap",
    wordBreak: "break-word"
  }
});

export function TranscriptSystemMessage({ label, text }: { label?: string; text: string }) {
  const [open, setOpen] = React.useState(false);
  const content = React.useMemo(() => formatSystemMessageContent(text), [text]);

  return (
    <>
      <ChatSystemMessage>
        <button
          aria-haspopup="dialog"
          onClick={() => setOpen(true)}
          type="button"
          {...stylex.props(styles.trigger)}
        >
          {label || systemMessagePreview(content.value, content.isJson)}
        </button>
      </ChatSystemMessage>
      <Dialog
        aria-label="System message"
        isOpen={open}
        maxHeight="80vh"
        onOpenChange={setOpen}
        padding={0}
        purpose="info"
        width={760}
      >
        <DialogHeader
          hasDivider
          onOpenChange={setOpen}
          subtitle={content.isJson ? "JSON" : "Task conversation"}
          title={label || "System message"}
        />
        <div {...stylex.props(styles.body)}>
          {content.isJson ? (
            <CodeBlock
              code={content.value}
              container="section"
              hasCopyButton
              language="json"
              maxHeight="60vh"
              width="100%"
            />
          ) : (
            <pre {...stylex.props(styles.text)}>{content.value}</pre>
          )}
        </div>
      </Dialog>
    </>
  );
}

function formatSystemMessageContent(text: string): { isJson: boolean; value: string } {
  const value = text.trim();
  try {
    return { isJson: true, value: JSON.stringify(JSON.parse(value), null, 2) };
  } catch {
    return { isJson: false, value };
  }
}

function systemMessagePreview(value: string, isJson: boolean): string {
  if (isJson) {
    return "System message · JSON";
  }
  return value.split("\n").find((line) => line.trim())?.trim() || "System message";
}
