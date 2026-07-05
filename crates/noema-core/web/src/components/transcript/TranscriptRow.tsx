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
  content: {
    display: "flex",
    flex: 1,
    maxWidth: "calc(100% - 40px)",
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
  return (
    <div {...stylex.props(styles.root, lane === "human" && styles.human)} data-lane={lane}>
      <TranscriptActorAvatar lane={lane} visible={showAvatar} />
      <div {...stylex.props(styles.content, lane === "human" && styles.humanContent)}>{children}</div>
    </div>
  );
}
