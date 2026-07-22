import * as stylex from "@stylexjs/stylex";
import type { IdentityAvatarActivity } from "../IdentityAvatar";
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
    "@container chat-transcript (width < 600px)": {
      gap: 0
    }
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
    overflowWrap: "anywhere",
    "@container chat-transcript (width < 600px)": {
      maxWidth: "100%"
    }
  },
  humanContent: {
    alignItems: "flex-end"
  },
  contentWithoutAvatar: {
    maxWidth: "100%"
  }
});

export function TranscriptRow({
  avatarActivity = "idle",
  avatarAnimated = false,
  lane,
  showAvatar = true,
  reserveAvatarSpace = true,
  children
}: {
  avatarActivity?: IdentityAvatarActivity;
  avatarAnimated?: boolean;
  lane: TranscriptLane;
  showAvatar?: boolean;
  reserveAvatarSpace?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div {...stylex.props(styles.root, lane === "human" && styles.human, !reserveAvatarSpace && styles.withoutAvatar)} data-lane={lane}>
      {reserveAvatarSpace ? (
        <TranscriptActorAvatar
          activity={avatarActivity}
          animated={avatarAnimated}
          lane={lane}
          visible={showAvatar}
        />
      ) : null}
      <div {...stylex.props(
        styles.content,
        lane === "human" && styles.humanContent,
        !reserveAvatarSpace && styles.contentWithoutAvatar
      )}>{children}</div>
    </div>
  );
}
