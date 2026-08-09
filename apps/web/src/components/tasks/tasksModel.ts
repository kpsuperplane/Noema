import type { TasksTask } from "./tasksTypes";

export function relativeTime(value: string, now = Date.now()): string {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "recently";
  const minutes = Math.max(0, Math.floor((now - timestamp) / 60_000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  return `${days}d`;
}

export function timestampLabel(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp)
    ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp)
    : value;
}

export function recurrenceSummary(cron: string): string {
  const [minute, hour, day, month, weekday, ...extra] = cron.trim().split(/\s+/);
  if (extra.length || !numericTime(hour, minute) || month !== "*") return "Custom recurring schedule";
  const time = new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
    timeZone: "UTC"
  }).format(new Date(Date.UTC(2020, 0, 1, Number(hour), Number(minute))));
  if (day !== "*" && weekday === "*" && /^\d{1,2}$/.test(day)) return `Monthly on day ${Number(day)} at ${time}`;
  if (day !== "*") return "Custom recurring schedule";
  if (weekday === "*") return `Every day at ${time}`;
  if (weekday === "1-5") return `Weekdays at ${time}`;
  const dayNames = weekday.split(",").map((value) => WEEKDAYS[Number(value)]).filter(Boolean);
  return dayNames.length === weekday.split(",").length
    ? `${dayNames.join(", ")} at ${time}`
    : "Custom recurring schedule";
}

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;

function numericTime(hour?: string, minute?: string): boolean {
  return Boolean(hour && minute && /^\d{1,2}$/.test(hour) && /^\d{1,2}$/.test(minute)
    && Number(hour) <= 23 && Number(minute) <= 59);
}

export function taskRunLabel(task: Pick<TasksTask, "currentRun">): string | null {
  return task.currentRun?.activityLabel ?? null;
}
