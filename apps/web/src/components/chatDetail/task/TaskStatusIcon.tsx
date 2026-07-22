import * as stylex from "@stylexjs/stylex";
import {
  AlertCircle,
  CircleCheck,
  CircleDot,
  Clock3,
  LoaderCircle,
  RotateCcw,
  UserRound,
  X
} from "lucide-react";
import type { TaskStatus } from "./taskTypes";

export function TaskStatusIcon({ status, size = 12 }: { status: TaskStatus; size?: number }) {
  const props = { "aria-hidden": true, size, strokeWidth: 2 } as const;
  const icon = status === "queued" ? <Clock3 {...props} />
    : status === "executing" ? <LoaderCircle {...props} />
    : status === "reviewing" ? <CircleDot {...props} />
    : status === "revision_requested" ? <RotateCcw {...props} />
    : status === "waiting_for_human" ? <UserRound {...props} />
    : status === "done" ? <CircleCheck {...props} />
    : status === "failed" ? <AlertCircle {...props} />
    : <X {...props} />;
  return status === "executing" ? <span {...stylex.props(styles.spinner)}>{icon}</span> : icon;
}

const rotate = stylex.keyframes({
  to: { transform: "rotate(360deg)" }
});

const styles = stylex.create({
  spinner: {
    display: "inline-flex",
    animationDuration: "900ms",
    animationIterationCount: "infinite",
    animationName: rotate,
    animationTimingFunction: "linear",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none"
    }
  }
});
