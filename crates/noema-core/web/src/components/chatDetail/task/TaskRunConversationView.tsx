import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ArrowLeft } from "lucide-react";
import { TaskRunTranscript } from "./TaskRunTranscript";
import { runDurationLabel, useTaskRunClock } from "./TaskRevisionTimeline";
import type { TaskRun, TaskRunItem } from "./taskTypes";

export function TaskRunConversationView({
  run,
  label,
  liveItems,
  onBack
}: {
  run: TaskRun;
  label?: string;
  liveItems?: readonly TaskRunItem[];
  onBack: () => void;
}) {
  const role = run.role === "reviewer" ? "Review" : "Executor";
  const displayLabel = label ?? role;
  const title = displayLabel === role
    ? `${role} · ${runStatusLabel(run.status)}`
    : displayLabel;
  const now = useTaskRunClock(run.status === "running");

  return (
    <section data-task-run-id={run.id} {...stylex.props(styles.root)}>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        label="Back to Timeline"
        icon={<ArrowLeft aria-hidden="true" size={15} />}
        {...stylex.props(styles.backButton)}
        onClick={onBack}
      />
      <header {...stylex.props(styles.header)}>
        <h3 {...stylex.props(styles.title)}>{title}</h3>
        <span {...stylex.props(styles.duration)}>{runDurationLabel(run, now)}</span>
      </header>
      <TaskRunTranscript liveItems={liveItems} run={run} />
    </section>
  );
}

function runStatusLabel(status: TaskRun["status"]): string {
  switch (status) {
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    case "interrupted":
      return "Interrupted";
    case "running":
      return "Working";
    case "queued":
      return "Queued";
    case "leased":
      return "Starting";
    case "waiting_for_approval":
      return "Waiting";
  }
}

const styles = stylex.create({
  root: { display: "grid", gap: 10, minWidth: 0 },
  backButton: { justifySelf: "start", width: "fit-content" },
  header: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "baseline",
    justifyContent: "space-between",
    gap: 10,
    paddingBottom: 4
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
  duration: {
    color: "var(--noema-text-secondary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10,
    lineHeight: 1.35,
    fontVariantNumeric: "tabular-nums",
    whiteSpace: "nowrap"
  }
});
