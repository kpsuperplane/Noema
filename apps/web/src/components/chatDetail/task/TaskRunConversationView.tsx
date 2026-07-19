import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import {
  AlertTriangle,
  ArrowLeft,
  Ban,
  Check,
  CircleHelp,
  Clock,
  Loader2,
  X
} from "lucide-react";
import { TaskRunTranscript } from "./TaskRunTranscript";
import { runDurationLabel, useTaskRunClock } from "./TaskRevisionTimeline";
import type { TaskReview, TaskRun, TaskRunItem } from "./taskTypes";

export function TaskRunConversationView({
  run,
  review,
  liveItems,
  onBack
}: {
  run: TaskRun;
  review?: TaskReview | null;
  liveItems?: readonly TaskRunItem[];
  onBack: () => void;
}) {
  const role = run.role === "reviewer" ? "Review" : run.role === "planner" ? "Planner" : "Executor";
  const presentation = runPresentation(run, review);
  const now = useTaskRunClock(run.status === "running");

  return (
    <section data-task-run-id={run.id} {...stylex.props(styles.root)}>
      <header {...stylex.props(styles.header)}>
        <Button
          type="button"
          icon={<ArrowLeft aria-hidden="true" size={15} />}
          isIconOnly
          label="Back to Timeline"
          size="sm"
          variant="ghost"
          onClick={onBack}
        />
        <h3 {...stylex.props(styles.title)}>{role}</h3>
        <RunStatusIcon presentation={presentation} />
        <span {...stylex.props(styles.duration)}>{runDurationLabel(run, now)}</span>
      </header>
      <TaskRunTranscript liveItems={liveItems} run={run} />
    </section>
  );
}

type RunPresentation = {
  kind: "success" | "error" | "cancelled" | "interrupted" | "running" | "pending" | "needs_input";
  label: string;
};

function runPresentation(run: TaskRun, review?: TaskReview | null): RunPresentation {
  switch (run.status) {
    case "completed":
      if (run.role === "reviewer" && review?.reviewerRunId === run.id) {
        if (review.verdict === "request_changes") {
          return { kind: "error", label: "Changes requested" };
        }
        if (review.verdict === "needs_human") {
          return { kind: "needs_input", label: "Needs human input" };
        }
      }
      return { kind: "success", label: "Passed" };
    case "failed":
      return { kind: "error", label: "Failed" };
    case "cancelled":
      return { kind: "cancelled", label: "Cancelled" };
    case "interrupted":
      return { kind: "interrupted", label: "Interrupted" };
    case "running":
      return { kind: "running", label: "Working" };
    case "queued":
    case "leased":
    case "waiting_for_approval":
      return { kind: "pending", label: "Waiting" };
  }
}

function RunStatusIcon({ presentation }: { presentation: RunPresentation }) {
  const iconProps = { "aria-hidden": true, size: 15, strokeWidth: 2 } as const;
  const icon = (() => {
    switch (presentation.kind) {
      case "success":
        return <Check {...iconProps} />;
      case "error":
        return <X {...iconProps} />;
      case "cancelled":
        return <Ban {...iconProps} />;
      case "interrupted":
        return <AlertTriangle {...iconProps} />;
      case "running":
        return <Loader2 {...iconProps} />;
      case "needs_input":
        return <CircleHelp {...iconProps} />;
      case "pending":
        return <Clock {...iconProps} />;
    }
  })();
  return (
    <span
      aria-label={presentation.label}
      role="img"
      title={presentation.label}
      {...stylex.props(styles.statusIcon, statusIconStyle(presentation.kind))}
    >
      {icon}
    </span>
  );
}

function statusIconStyle(kind: RunPresentation["kind"]) {
  switch (kind) {
    case "success":
      return styles.statusSuccess;
    case "running":
      return styles.statusRunning;
    case "pending":
      return styles.statusPending;
    case "needs_input":
      return styles.statusNeedsInput;
    case "error":
    case "cancelled":
    case "interrupted":
      return styles.statusError;
  }
}

const styles = stylex.create({
  root: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    gap: 6,
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    overflow: "hidden"
  },
  header: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr) auto auto",
    alignItems: "center",
    gap: 8
  },
  title: {
    minWidth: 0,
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 14,
    fontWeight: 700,
    lineHeight: 1.3,
    overflowWrap: "anywhere"
  },
  statusIcon: {
    display: "inline-flex",
    width: 16,
    height: 16,
    alignItems: "center",
    justifyContent: "center"
  },
  statusSuccess: { color: "var(--noema-pine-600)" },
  statusError: { color: "var(--noema-red-700)" },
  statusPending: { color: "var(--noema-text-muted)" },
  statusNeedsInput: { color: "var(--noema-blue-700)" },
  statusRunning: {
    color: "var(--noema-pine-600)",
    animationDuration: "900ms",
    animationIterationCount: "infinite",
    animationName: "tool-marker-spinner-rotate",
    animationTimingFunction: "linear",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none"
    }
  },
  duration: {
    color: "var(--noema-text-secondary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10,
    lineHeight: 1.35,
    fontVariantNumeric: "tabular-nums",
    whiteSpace: "nowrap"
  }
});
