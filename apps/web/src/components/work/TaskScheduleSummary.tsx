import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import type { TaskDetail } from "@/components/chatDetail/task/taskTypes";
import {
  WorkEndTaskRecurrenceDocument,
  WorkPauseTaskRecurrenceDocument,
  WorkResumeTaskRecurrenceDocument,
  WorkSkipTaskRecurrenceNextDocument,
  WorkTaskRecurrenceDocument,
  WorkUpdateTaskRecurrenceDocument,
  type WorkTaskRecurrenceQuery
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { normalizeWorkSearch } from "./workTypes";
import { initialScheduleDraft, scheduleInput, ScheduleFields, type ScheduleDraft } from "./ScheduleFields";

type Schedule = NonNullable<TaskDetail["schedule"]>;
type Recurrence = WorkTaskRecurrenceQuery["taskRecurrence"];

export function TaskScheduleSummary({ schedule }: { schedule: Schedule }) {
  if (!schedule.recurrenceId) {
    return <p {...stylex.props(styles.oneTime)}>Scheduled for <strong>{dateLabel(schedule.scheduledFor, schedule.timeZone)}</strong> · {schedule.timeZone}</p>;
  }
  return <RecurrenceSummary schedule={schedule} recurrenceId={schedule.recurrenceId} />;
}

function RecurrenceSummary({ schedule, recurrenceId }: { schedule: Schedule; recurrenceId: string }) {
  const result = useQuery(WorkTaskRecurrenceDocument, { variables: { recurrenceId } });
  const [pause, pauseState] = useMutation(WorkPauseTaskRecurrenceDocument);
  const [resume, resumeState] = useMutation(WorkResumeTaskRecurrenceDocument);
  const [skip, skipState] = useMutation(WorkSkipTaskRecurrenceNextDocument);
  const [end, endState] = useMutation(WorkEndTaskRecurrenceDocument);
  const [editing, setEditing] = React.useState(false);
  const recurrence = result.data?.taskRecurrence;
  const busy = pauseState.loading || resumeState.loading || skipState.loading || endState.loading;
  const error = result.error ?? pauseState.error ?? resumeState.error ?? skipState.error ?? endState.error;
  if (!recurrence) return <p {...stylex.props(styles.oneTime)}>Repeating from {dateLabel(schedule.scheduledFor, schedule.timeZone)}</p>;
  const commandInput = { recurrenceId, expectedRevision: recurrence.revision, clientMutationId: createClientId() };
  const change = async (kind: "pause" | "resume" | "skip" | "end") => {
    if (kind === "pause") await pause({ variables: { input: commandInput } });
    if (kind === "resume") await resume({ variables: { input: commandInput } });
    if (kind === "skip") await skip({ variables: { input: commandInput } });
    if (kind === "end") await end({ variables: { input: commandInput } });
    await result.refetch();
  };
  return (
    <VStack gap={2} className={stylex.props(styles.root).className}>
      <HStack justify="between" align="center" gap={2}>
        <VStack gap={0.5}>
          <strong>{recurrence.nextRunAt ? `Next ${dateLabel(recurrence.nextRunAt, recurrence.timeZone)}` : recurrence.lifecycle.toLowerCase()}</strong>
          <span {...stylex.props(styles.meta)}>{recurrence.cronExpression} · {recurrence.timeZone}</span>
        </VStack>
        {recurrence.lifecycle !== "ENDED" ? <Button size="sm" variant="ghost" label="Edit" onClick={() => setEditing(true)} /> : null}
      </HStack>
      {recurrence.lifecycle !== "ENDED" ? (
        <HStack gap={1}>
          <Button size="sm" variant="secondary" label={recurrence.lifecycle === "PAUSED" ? "Resume" : "Pause"} isDisabled={busy} onClick={() => void change(recurrence.lifecycle === "PAUSED" ? "resume" : "pause")} />
          <Button size="sm" variant="ghost" label="Skip next" isDisabled={busy} onClick={() => void change("skip")} />
          <Button size="sm" variant="ghost" label="End" isDisabled={busy} onClick={() => void change("end")} />
        </HStack>
      ) : null}
      {error ? <span role="alert" {...stylex.props(styles.error)}>{error.message}</span> : null}
      {recurrence.occurrences.length ? (
        <VStack gap={1}>
          <strong {...stylex.props(styles.historyTitle)}>Occurrence history</strong>
          {recurrence.occurrences.map((occurrence) => occurrence.taskId ? (
            <Link key={`${occurrence.localSlot}:${occurrence.recurrenceRevision}`} to="/work/tasks/$taskId" params={{ taskId: occurrence.taskId }} search={(current) => normalizeWorkSearch(current)} {...stylex.props(styles.occurrence)}>
              <span>{dateLabel(occurrence.scheduledFor, recurrence.timeZone)}</span><span>{occurrence.resolution.toLowerCase().replaceAll("_", " ")}</span>
            </Link>
          ) : <HStack key={`${occurrence.localSlot}:${occurrence.recurrenceRevision}`} justify="between" {...stylex.props(styles.occurrence)}><span>{dateLabel(occurrence.scheduledFor, recurrence.timeZone)}</span><span>{occurrence.resolution.toLowerCase().replaceAll("_", " ")}</span></HStack>)}
        </VStack>
      ) : null}
      {editing ? <RecurrenceEditDialog recurrence={recurrence} onClose={() => setEditing(false)} onUpdated={() => result.refetch()} /> : null}
    </VStack>
  );
}

function RecurrenceEditDialog({ recurrence, onClose, onUpdated }: { recurrence: Recurrence; onClose: () => void; onUpdated: () => Promise<unknown> }) {
  const initial = () => ({ ...initialScheduleDraft({ scheduledFor: recurrence.startsAt, timeZone: recurrence.timeZone, missedRunPolicy: recurrence.missedRunPolicy, recurrenceId: recurrence.recurrenceId }), repeat: "custom" as const, cron: recurrence.cronExpression, overlapPolicy: recurrence.overlapPolicy });
  const [draft, setDraft] = React.useState<ScheduleDraft>(initial);
  const [update, state] = useMutation(WorkUpdateTaskRecurrenceDocument);
  React.useEffect(() => {
    requestAnimationFrame(() => document.querySelector<HTMLElement>('[aria-label="Edit recurring schedule"] input')?.focus());
  }, []);
  return (
    <Dialog isOpen onOpenChange={(next) => { if (!next) onClose(); }} purpose="form" width={500} aria-label="Edit recurring schedule">
      <Layout height="auto" header={<DialogHeader title="Edit recurring schedule" subtitle="Changes apply to future occurrences." onOpenChange={(next) => { if (!next) onClose(); }} />} content={<LayoutContent>
        <VStack as="form" gap={3} onSubmit={(event) => {
          event.preventDefault();
          const next = scheduleInput(draft)?.recurrence;
          if (!next) return;
          void update({ variables: { input: { recurrenceId: recurrence.recurrenceId, expectedRevision: recurrence.revision, startsAt: next.startsAt, cronExpression: next.cronExpression, timeZone: draft.timeZone, missedRunPolicy: draft.missedRunPolicy, overlapPolicy: draft.overlapPolicy, clientMutationId: createClientId() } } })
            .then(async () => { await onUpdated(); onClose(); }).catch(() => undefined);
        }}>
          <ScheduleFields value={draft} onChange={setDraft} recurringOnly />
          {state.error ? <span role="alert" {...stylex.props(styles.error)}>{state.error.message}</span> : null}
          <HStack justify="end" gap={2}><Button type="button" size="sm" variant="ghost" label="Cancel" onClick={onClose} /><Button type="submit" size="sm" variant="primary" label="Save" isLoading={state.loading} isDisabled={state.loading || !scheduleInput(draft)?.recurrence} /></HStack>
        </VStack>
      </LayoutContent>} />
    </Dialog>
  );
}

function dateLabel(value: string, timeZone: string) { return new Intl.DateTimeFormat(undefined, { timeZone, dateStyle: "medium", timeStyle: "short" }).format(new Date(value)); }

const styles = stylex.create({
  root: { borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-2)" },
  oneTime: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 11 },
  meta: { color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 9 },
  historyTitle: { color: "var(--noema-text-secondary)", fontSize: 10 },
  occurrence: { display: "flex", justifyContent: "space-between", gap: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 10, textDecoration: "none" },
  error: { color: "var(--destructive)", fontSize: 11 }
});
