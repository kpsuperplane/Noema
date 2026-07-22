import * as stylex from "@stylexjs/stylex";
import {
  IdentityAvatar,
  LOCAL_AGENT_AVATAR_ID,
  LOCAL_HUMAN_AVATAR_ID,
  type IdentityAvatarActivity
} from "../IdentityAvatar";
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
    overflow: "hidden",
    "@container chat-transcript (width < 600px)": {
      display: "none"
    }
  },
  hidden: {
    visibility: "hidden"
  }
});

export function TranscriptActorAvatar({
  activity = "idle",
  animated = false,
  lane,
  visible = true
}: {
  activity?: IdentityAvatarActivity;
  animated?: boolean;
  lane: TranscriptLane;
  visible?: boolean;
}) {
  const actorId = lane === "human" ? LOCAL_HUMAN_AVATAR_ID : LOCAL_AGENT_AVATAR_ID;
  const actorType = lane === "human" ? "human" : "agent";

  return (
    <span {...stylex.props(styles.root, !visible && styles.hidden)} aria-hidden={!visible}>
      <IdentityAvatar
        activity={activity}
        actorId={actorId}
        actorType={actorType}
        animated={animated && visible}
        size="sm"
      />
    </span>
  );
}
