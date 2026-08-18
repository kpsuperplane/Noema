import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import type { TaskDetail } from "@/components/chatDetail/task/taskTypes";
import { normalizeTasksSearch } from "./tasksTypes";

type Schedule = NonNullable<TaskDetail["schedule"]>;

export function TaskScheduleSummary({ schedule }: { schedule: Schedule }) {
  if (!schedule.recurrenceId) {
    return <p {...stylex.props(styles.summary)}>Scheduled for <strong>{dateLabel(schedule.scheduledFor, schedule.timeZone)}</strong> · {schedule.timeZone}</p>;
  }
  return (
    <p {...stylex.props(styles.summary)}>
      <Link
        to="/tasks/recurrences/$recurrenceId"
        params={{ recurrenceId: schedule.recurrenceId }}
        search={(current) => normalizeTasksSearch(current)}
        {...stylex.props(styles.link)}
      >
        View recurring task
      </Link>
      <span> · This occurrence · {dateLabel(schedule.scheduledFor, schedule.timeZone)}</span>
    </p>
  );
}

function dateLabel(value: string, timeZone: string) {
  return new Intl.DateTimeFormat(undefined, {
    timeZone,
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    timeZoneName: "short"
  }).format(new Date(value));
}

const styles = stylex.create({
  summary: { margin: 0, paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", color: "var(--noema-text-secondary)", fontSize: 12 },
  link: { color: "var(--noema-pine-700)", fontWeight: 650, textDecoration: "none", ":hover": { textDecoration: "underline" } }
});
