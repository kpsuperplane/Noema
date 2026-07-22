import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { springs } from "@/motion/springs";
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
  animateArrival,
  children,
  lane
}: {
  animateArrival: boolean;
  children: React.ReactNode;
  lane: TranscriptLane;
}) {
  const reduceMotion = useReducedMotion();

  return (
    <m.div
      {...stylex.props(styles.content)}
      data-lane={lane}
      data-slot="message-arrival-content"
      initial={
        animateArrival && !reduceMotion
          ? { opacity: 0, y: 10, scale: 0.985 }
          : false
      }
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={reduceMotion ? { duration: 0 } : springs.standard}
      style={{ transformOrigin: lane === "human" ? "bottom right" : "bottom center" }}
    >
      <div {...stylex.props(styles.inner, lane === "human" && styles.innerHuman)} data-slot="message-arrival-inner">
        {children}
      </div>
    </m.div>
  );
}
