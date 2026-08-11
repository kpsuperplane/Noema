import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Banner } from "@astryxdesign/core/Banner";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Collapsible } from "@astryxdesign/core/Collapsible";
import { DateTimeInput, type ISODateTimeString } from "@astryxdesign/core/DateTimeInput";
import { HStack } from "@astryxdesign/core/HStack";
import { Selector } from "@astryxdesign/core/Selector";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { TasksTaskSchedulePreviewDocument, type NewTaskScheduleInput } from "@/generated/graphql";

export type RepeatChoice = "never" | "daily" | "weekdays" | "selected" | "weekly" | "monthly" | "custom";
export type ScheduleDraft = {
  localStart: string;
  timeZone: string;
  repeat: RepeatChoice;
  selectedDays: number[];
  cron: string;
  firstOccurrence?: string;
  resolvedFor?: string;
  missedRunPolicy: "RUN_ONCE" | "SKIP";
  overlapPolicy: "SKIP" | "QUEUE_ONE" | "ALLOW";
};

const repeatOptions = [
  { value: "never", label: "Never" }, { value: "daily", label: "Daily" },
  { value: "weekdays", label: "Weekdays" }, { value: "selected", label: "Selected weekdays" },
  { value: "weekly", label: "Weekly" }, { value: "monthly", label: "Monthly" },
  { value: "custom", label: "Custom cron" }
];
const weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

export function initialScheduleDraft(schedule?: { scheduledFor: string; timeZone: string; missedRunPolicy: "RUN_ONCE" | "SKIP"; recurrenceId?: string | null }): ScheduleDraft {
  const timeZone = schedule?.timeZone || Intl.DateTimeFormat().resolvedOptions().timeZone;
  const start = schedule?.scheduledFor ? localDateTime(schedule.scheduledFor, timeZone) : defaultLocalStart();
  return {
    localStart: start,
    timeZone,
    repeat: schedule?.recurrenceId ? "custom" : "never",
    selectedDays: [new Date(`${start}:00`).getDay()],
    cron: schedule?.recurrenceId ? "" : cronFor("daily", start, []),
    missedRunPolicy: schedule?.missedRunPolicy ?? "RUN_ONCE",
    overlapPolicy: "SKIP"
  };
}

export function scheduleInput(draft: ScheduleDraft): NewTaskScheduleInput | null {
  const startsAt = localToUtc(draft.localStart, draft.timeZone);
  if (!startsAt) return null;
  const cron = draft.repeat === "never" ? undefined : draft.repeat === "custom"
    ? draft.cron.trim() : cronFor(draft.repeat, draft.localStart, draft.selectedDays);
  if (draft.repeat !== "never" && !cron) return null;
  const resolutionKey = cron ? `${startsAt}|${draft.timeZone}|${cron}` : undefined;
  const scheduledFor = cron
    ? draft.resolvedFor === resolutionKey ? draft.firstOccurrence : undefined
    : startsAt;
  if (!scheduledFor) return null;
  return {
    scheduledFor,
    timeZone: draft.timeZone,
    missedRunPolicy: draft.missedRunPolicy,
    recurrence: cron ? { startsAt, cronExpression: cron, overlapPolicy: draft.overlapPolicy } : null
  };
}

export function ScheduleFields({ value, onChange, recurringOnly = false }: { value: ScheduleDraft; onChange: (value: ScheduleDraft) => void; recurringOnly?: boolean }) {
  const startsAt = localToUtc(value.localStart, value.timeZone);
  const cron = value.repeat === "never" ? undefined : value.repeat === "custom"
    ? value.cron.trim() : cronFor(value.repeat, value.localStart, value.selectedDays);
  const resolutionKey = startsAt && cron ? `${startsAt}|${value.timeZone}|${cron}` : undefined;
  const preview = useQuery(TasksTaskSchedulePreviewDocument, {
    variables: { input: { startsAt: startsAt ?? "", timeZone: value.timeZone, cronExpression: cron } },
    skip: !startsAt || (value.repeat !== "never" && !cron),
    fetchPolicy: "cache-first"
  });
  const firstOccurrence = preview.data?.taskSchedulePreview.occurrences[0];
  React.useEffect(() => {
    if (!resolutionKey || !firstOccurrence || (value.resolvedFor === resolutionKey && value.firstOccurrence === firstOccurrence)) return;
    onChange({ ...value, firstOccurrence, resolvedFor: resolutionKey });
  }, [firstOccurrence, onChange, resolutionKey, value]);
  const set = <K extends keyof ScheduleDraft>(key: K, next: ScheduleDraft[K]) => onChange({ ...value, [key]: next });

  return (
    <VStack gap={3}>
      <DateTimeInput
        label="Starts"
        value={value.localStart as ISODateTimeString}
        onChange={(next) => next && set("localStart", next)}
        min={recurringOnly ? undefined : defaultLocalStart(0) as ISODateTimeString}
        size="sm"
        isRequired
      />
      <TextInput
        label="Timezone"
        description="IANA timezone used for wall-clock recurrence."
        value={value.timeZone}
        onChange={(next) => set("timeZone", next)}
        size="sm"
        isRequired
      />
      <Selector
        label="Repeat"
        options={recurringOnly ? repeatOptions.filter((option) => option.value !== "never") : repeatOptions}
        value={value.repeat}
        onChange={(next) => set("repeat", next as RepeatChoice)}
        size="sm"
      />
      {value.repeat === "selected" ? (
        <VStack as="fieldset" gap={1} {...stylex.props(styles.weekdayGroup)}>
          <legend {...stylex.props(styles.weekdayLegend)}>Days</legend>
          <HStack gap={1} wrap="wrap">
            {weekdays.map((day, index) => (
              <CheckboxInput
                key={day}
                label={day}
                size="sm"
                value={value.selectedDays.includes(index)}
                onChange={() => set("selectedDays", value.selectedDays.includes(index) ? value.selectedDays.filter((item) => item !== index) : [...value.selectedDays, index].sort())}
              />
            ))}
          </HStack>
        </VStack>
      ) : null}
      {value.repeat === "custom" ? (
        <TextInput label="Cron expression" description="Five fields: minute hour day month weekday." value={value.cron} onChange={(next) => set("cron", next)} size="sm" isRequired />
      ) : null}
      {cron && highFrequency(cron) ? <Banner status="warning" title="This schedule may run more than once an hour." /> : null}
      <Collapsible trigger="Advanced" defaultIsOpen={false}>
        <VStack gap={3}>
          <Selector label="If a run was missed" options={[{ value: "RUN_ONCE", label: "Run once" }, { value: "SKIP", label: "Skip" }]} value={value.missedRunPolicy} onChange={(next) => set("missedRunPolicy", next as ScheduleDraft["missedRunPolicy"])} size="sm" />
          {value.repeat !== "never" ? <Selector label="If another run is active" options={[{ value: "SKIP", label: "Skip" }, { value: "QUEUE_ONE", label: "Queue one" }, { value: "ALLOW", label: "Allow overlap" }]} value={value.overlapPolicy} onChange={(next) => set("overlapPolicy", next as ScheduleDraft["overlapPolicy"])} size="sm" /> : null}
        </VStack>
      </Collapsible>
      {startsAt && !preview.loading && preview.data ? (
        <VStack gap={1} aria-live="polite">
          <strong>Next runs</strong>
          {preview.data.taskSchedulePreview.occurrences.map((occurrence) => <span key={occurrence}>{dateLabel(occurrence, value.timeZone)}</span>)}
        </VStack>
      ) : null}
      {!startsAt || (value.repeat !== "never" && !cron) || preview.error ? <Banner status="error" title="Check the start time, timezone, and repeat settings." /> : null}
    </VStack>
  );
}

const styles = stylex.create({
  weekdayGroup: {
    minWidth: 0,
    margin: "var(--spacing-0)",
    padding: "var(--spacing-0)",
    borderWidth: 0
  },
  weekdayLegend: {
    marginBlockEnd: "var(--spacing-1)"
  }
});

function cronFor(repeat: Exclude<RepeatChoice, "never" | "custom"> | "daily", local: string, days: number[]) {
  const date = new Date(`${local}:00`);
  const [hour, minute] = local.slice(11, 16).split(":").map(Number);
  if (!Number.isFinite(hour) || !Number.isFinite(minute)) return "";
  if (repeat === "daily") return `${minute} ${hour} * * *`;
  if (repeat === "weekdays") return `${minute} ${hour} * * 1-5`;
  if (repeat === "selected") return days.length ? `${minute} ${hour} * * ${days.join(",")}` : "";
  if (repeat === "weekly") return `${minute} ${hour} * * ${date.getDay()}`;
  return `${minute} ${hour} ${date.getDate()} * *`;
}

function defaultLocalStart(offsetMinutes = 60) {
  const date = new Date(Date.now() + offsetMinutes * 60_000);
  date.setSeconds(0, 0);
  return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}T${two(date.getHours())}:${two(date.getMinutes())}`;
}

function localDateTime(instant: string, timeZone: string) {
  const parts = dateParts(new Date(instant), timeZone);
  return `${parts.year}-${parts.month}-${parts.day}T${parts.hour}:${parts.minute}`;
}

function localToUtc(local: string, timeZone: string): string | null {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(local)) return null;
  const naive = Date.parse(`${local}:00Z`);
  try {
    for (let offset = -14 * 60; offset <= 14 * 60; offset += 15) {
      const candidate = new Date(naive + offset * 60_000);
      if (localDateTime(candidate.toISOString(), timeZone) === local) return candidate.toISOString();
    }
  } catch { return null; }
  return null;
}

function dateParts(date: Date, timeZone: string) {
  const parts = new Intl.DateTimeFormat("en-CA", { timeZone, year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).formatToParts(date);
  return Object.fromEntries(parts.map((part) => [part.type, part.value])) as Record<"year" | "month" | "day" | "hour" | "minute", string>;
}

function dateLabel(value: string, timeZone: string) {
  return new Intl.DateTimeFormat(undefined, { timeZone, dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}
function highFrequency(cron: string) { return cron.trim().split(/\s+/)[0].includes("*"); }
function two(value: number) { return String(value).padStart(2, "0"); }
