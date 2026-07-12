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
        <div {...stylex.props(styles.headingCopy)}>
          <span {...stylex.props(styles.eyebrow)}>{displayLabel} conversation</span>
          <h3 {...stylex.props(styles.title)}>Run history</h3>
        </div>
        <div {...stylex.props(styles.meta)}>
          <span>{label ?? runStatusLabel(run.status)}</span>
          <span aria-hidden="true">·</span>
          <span>Revision {run.revision ?? 0} · Attempt {run.attemptIndex + 1}</span>
          <span aria-hidden="true">·</span>
          <span>{runDurationLabel(run, now)}</span>
        </div>
      </header>
      <TaskRunTranscript liveItems={liveItems} run={run} />
    </section>
  );
}

function runStatusLabel(status: TaskRun["status"]): string {
  switch (status) {
    case "completed":
      return "Passed";
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
  root: { display: "grid", gap: 14, minWidth: 0 },
  backButton: { justifySelf: "start", width: "fit-content" },
  header: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)",
    alignItems: "end",
    justifyContent: "space-between",
    gap: 12,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)",
    paddingBottom: 12,
    "@media (max-width: 520px)": {
      gridTemplateColumns: "1fr",
      alignItems: "start"
    }
  },
  headingCopy: { display: "grid", minWidth: 0, gap: 3 },
  eyebrow: { color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650, letterSpacing: "0.06em", textTransform: "uppercase" },
  title: { margin: 0, color: "var(--noema-text-primary)", fontSize: 16, fontWeight: 700, lineHeight: 1.25 },
  meta: {
    display: "flex",
    minWidth: 0,
    maxWidth: "100%",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "end",
    gap: 5,
    color: "var(--noema-text-secondary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10,
    lineHeight: 1.35,
    fontVariantNumeric: "tabular-nums",
    overflowWrap: "anywhere",
    "@media (max-width: 520px)": {
      justifySelf: "start",
      justifyContent: "start"
    }
  }
});
