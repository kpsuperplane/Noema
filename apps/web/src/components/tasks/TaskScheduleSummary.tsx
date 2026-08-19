import * as stylex from "@stylexjs/stylex";
import type { TaskDetail } from "@/components/chatDetail/task/taskTypes";
import { taskScheduleTimestampLabel } from "./tasksModel";

type Schedule = NonNullable<TaskDetail["schedule"]>;

export function TaskScheduleSummary({ schedule }: { schedule: Schedule }) {
  if (schedule.recurrenceId) return null;
  return <p {...stylex.props(styles.summary)}>Scheduled for <strong>{taskScheduleTimestampLabel(schedule.scheduledFor, schedule.timeZone)}</strong> · {schedule.timeZone}</p>;
}

const styles = stylex.create({
  summary: { margin: 0, paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", color: "var(--noema-text-secondary)", fontSize: 12 }
});
