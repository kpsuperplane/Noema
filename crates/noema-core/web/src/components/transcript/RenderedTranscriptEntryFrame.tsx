import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import type { TranscriptLane } from "./renderModel";

const styles = stylex.create({
  content: {
    display: "grid",
    width: "100%",
    maxWidth: 760,
    minWidth: 0
  },
  inner: {
    display: "grid",
    alignItems: "end",
    minHeight: 0
  },
  innerHuman: {
    justifyItems: "end"
  }
});

export function RenderedTranscriptEntryFrame({
  children,
  lane
}: {
  children: React.ReactNode;
  lane: TranscriptLane;
}) {
  return (
    <div {...stylex.props(styles.content)} data-lane={lane} data-slot="message-arrival-content">
      <div {...stylex.props(styles.inner, lane === "human" && styles.innerHuman)} data-slot="message-arrival-inner">
        {children}
      </div>
    </div>
  );
}
