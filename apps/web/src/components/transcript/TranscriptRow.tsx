import * as stylex from "@stylexjs/stylex";
import type { TranscriptLane } from "./renderModel";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";

const styles = stylex.create({
  root: {
    position: "relative",
    display: "flex",
    width: "100%",
    maxWidth: 760,
    minWidth: 0,
    gap: 8,
    fontSize: 14,
    alignItems: "flex-end",
  },
  human: {
    flexDirection: "row-reverse"
  },
  withoutAvatar: {
    gap: 0
  },
  content: {
    display: "flex",
    flex: 1,
    maxWidth: "calc(100% - var(--chat-opposite-avatar-gutter, 40px))",
    minWidth: 0,
    flexDirection: "column",
    gap: 10,
    overflowWrap: "anywhere"
  },
  humanContent: {
    alignItems: "flex-end"
  },
  contentWithoutAvatar: {
    maxWidth: "100%"
  }
});

export function TranscriptRow({
  lane,
  showAvatar = true,
  reserveAvatarSpace = true,
  children
}: {
  lane: TranscriptLane;
  showAvatar?: boolean;
  reserveAvatarSpace?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div {...stylex.props(styles.root, lane === "human" && styles.human, !reserveAvatarSpace && styles.withoutAvatar)} data-lane={lane}>
      {reserveAvatarSpace ? <TranscriptActorAvatar lane={lane} visible={showAvatar} /> : null}
      <div {...stylex.props(
        styles.content,
        lane === "human" && styles.humanContent,
        !reserveAvatarSpace && styles.contentWithoutAvatar
      )}>{children}</div>
    </div>
  );
}
