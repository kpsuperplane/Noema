import * as stylex from "@stylexjs/stylex";
import { readableKind, statusLabel } from "@/shared/format";
import type { TurnTranscriptItem } from "@/shared/types";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
import { TranscriptSystemNotice } from "./TranscriptSystemNotice";

const styles = stylex.create({
  details: {
    display: "grid",
    gap: 9,
    margin: 0,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    paddingTop: 10
  },
  row: {
    display: "grid",
    gap: 2
  },
  label: {
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10,
    letterSpacing: "0.08em",
    color: "var(--noema-text-faint)",
    textTransform: "uppercase"
  },
  value: {
    margin: 0,
    fontSize: 13,
    color: "var(--noema-text-secondary)"
  }
});

export function ActivityRow({
  item,
  open,
  onToggle
}: {
  item: Extract<TurnTranscriptItem, { kind: "activity" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const isMemorySave = item.activity_kind === "memory_save";
  const title = isMemorySave ? "Memory saved" : item.title;
  const status = statusLabel(item.status);
  const tone = item.status === "FAILED" ? "error" : item.status === "STARTED" ? "info" : "success";
  const noticeTone = item.status === "FAILED" ? "error" : item.status === "COMPLETED" ? "success" : "default";

  if (!isMemorySave) {
    return (
      <TranscriptSystemNotice label={status} role={item.status === "FAILED" ? "alert" : "status"} tone={noticeTone}>
        {item.summary || title}
      </TranscriptSystemNotice>
    );
  }

  return (
    <TranscriptAttachmentCard
      title={title}
      description={item.summary}
      meta={status}
      icon={isMemorySave ? "M" : "A"}
      tone={tone}
      headerButtonProps={{
        onClick: onToggle,
        "aria-expanded": open
      }}
    >
      {open ? (
          <dl {...stylex.props(styles.details)}>
            <div {...stylex.props(styles.row)}>
              <dt {...stylex.props(styles.label)}>What happened</dt>
              <dd {...stylex.props(styles.value)}>{item.summary || title}</dd>
            </div>
            <div {...stylex.props(styles.row)}>
              <dt {...stylex.props(styles.label)}>Source</dt>
              <dd {...stylex.props(styles.value)}>
                {isMemorySave ? "You used an explicit remember request in this chat." : readableKind(item.activity_kind)}
              </dd>
            </div>
            <div {...stylex.props(styles.row)}>
              <dt {...stylex.props(styles.label)}>Next step</dt>
              <dd {...stylex.props(styles.value)}>
                {isMemorySave ? "Continue chatting. Deeper memory settings come in the next slice." : "No action needed."}
              </dd>
            </div>
          </dl>
      ) : null}
    </TranscriptAttachmentCard>
  );
}
