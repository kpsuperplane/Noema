import * as stylex from "@stylexjs/stylex";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID, LOCAL_HUMAN_AVATAR_ID } from "../IdentityAvatar";
import type { TranscriptLane } from "./renderModel";

const styles = stylex.create({
  root: {
    display: "flex",
    width: "fit-content",
    minWidth: 32,
    alignItems: "center",
    justifyContent: "center",
    alignSelf: "flex-end",
    flexShrink: 0,
    overflow: "hidden"
  },
  hidden: {
    visibility: "hidden"
  }
});

export function TranscriptActorAvatar({
  lane,
  visible = true
}: {
  lane: TranscriptLane;
  visible?: boolean;
}) {
  const actorId = lane === "human" ? LOCAL_HUMAN_AVATAR_ID : LOCAL_AGENT_AVATAR_ID;
  const actorType = lane === "human" ? "human" : "agent";

  return (
    <span {...stylex.props(styles.root, !visible && styles.hidden)} aria-hidden={!visible}>
      <IdentityAvatar actorId={actorId} actorType={actorType} size="sm" />
    </span>
  );
}
