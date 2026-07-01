import * as stylex from "@stylexjs/stylex";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID, LOCAL_HUMAN_AVATAR_ID } from "../IdentityAvatar";
import type { TranscriptLane } from "./renderModel";

const styles = stylex.create({
  root: {
    position: "relative",
    display: "flex",
    width: "100%",
    maxWidth: 760,
    minWidth: 0,
    gap: 8,
    fontSize: 14
  },
  human: {
    flexDirection: "row-reverse"
  },
  avatar: {
    display: "flex",
    width: "fit-content",
    minWidth: 32,
    alignItems: "center",
    justifyContent: "center",
    alignSelf: "flex-end",
    flexShrink: 0,
    overflow: "hidden"
  },
  hiddenAvatar: {
    visibility: "hidden"
  },
  content: {
    display: "flex",
    width: "100%",
    minWidth: 0,
    flexDirection: "column",
    gap: 10,
    overflowWrap: "anywhere"
  },
  humanContent: {
    alignItems: "flex-end"
  }
});

export function TranscriptRow({
  lane,
  showAvatar = true,
  children
}: {
  lane: TranscriptLane;
  showAvatar?: boolean;
  children: React.ReactNode;
}) {
  const actorId = lane === "human" ? LOCAL_HUMAN_AVATAR_ID : LOCAL_AGENT_AVATAR_ID;
  const actorType = lane === "human" ? "human" : "agent";

  return (
    <div {...stylex.props(styles.root, lane === "human" && styles.human)} data-lane={lane}>
      <div {...stylex.props(styles.avatar, !showAvatar && styles.hiddenAvatar)} aria-hidden={!showAvatar}>
        <IdentityAvatar actorId={actorId} actorType={actorType} size="sm" />
      </div>
      <div {...stylex.props(styles.content, lane === "human" && styles.humanContent)}>{children}</div>
    </div>
  );
}
