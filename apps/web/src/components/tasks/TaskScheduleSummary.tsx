import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Collapsible } from "@astryxdesign/core/Collapsible";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { AlertTriangle, CircleStop, MoreHorizontal, Pause, Pencil, Play, SkipForward } from "lucide-react";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import type { TaskDetail } from "@/components/chatDetail/task/taskTypes";
import {
  TasksEndTaskRecurrenceDocument,
  TasksPauseTaskRecurrenceDocument,
  TasksResumeTaskRecurrenceDocument,
  TasksRunTaskRecurrenceNowDocument,
  TasksSkipTaskRecurrenceNextDocument,
  TasksTaskRecurrenceDocument,
  TasksUpdateTaskRecurrenceDocument,
  type TasksTaskRecurrenceQuery
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { normalizeTasksSearch } from "./tasksTypes";
import { initialScheduleDraft, scheduleInput, ScheduleFields, type ScheduleDraft } from "./ScheduleFields";
import { recurrenceSummary } from "./tasksModel";

type Schedule = NonNullable<TaskDetail["schedule"]>;
type Recurrence = TasksTaskRecurrenceQuery["taskRecurrence"];

export function TaskScheduleSummary({ schedule, canRunRecurrenceNow = false }: { schedule: Schedule; canRunRecurrenceNow?: boolean }) {
  if (!schedule.recurrenceId) {
    return <p {...stylex.props(styles.oneTime)}>Scheduled for <strong>{dateLabel(schedule.scheduledFor, schedule.timeZone)}</strong> · {schedule.timeZone}</p>;
  }
  return <RecurrenceSummary schedule={schedule} recurrenceId={schedule.recurrenceId} canRunNow={canRunRecurrenceNow} />;
}

function RecurrenceSummary({ schedule, recurrenceId, canRunNow }: { schedule: Schedule; recurrenceId: string; canRunNow: boolean }) {
  const result = useQuery(TasksTaskRecurrenceDocument, { variables: { recurrenceId } });
  const [pause, pauseState] = useMutation(TasksPauseTaskRecurrenceDocument);
  const [resume, resumeState] = useMutation(TasksResumeTaskRecurrenceDocument);
  const [skip, skipState] = useMutation(TasksSkipTaskRecurrenceNextDocument);
  const [end, endState] = useMutation(TasksEndTaskRecurrenceDocument);
  const [runNow, runNowState] = useMutation(TasksRunTaskRecurrenceNowDocument);
  const [editing, setEditing] = React.useState(false);
  const [confirmingEnd, setConfirmingEnd] = React.useState(false);
  const recurrence = result.data?.taskRecurrence;
  const busy = pauseState.loading || resumeState.loading || skipState.loading || endState.loading || runNowState.loading;
  const error = result.error ?? pauseState.error ?? resumeState.error ?? skipState.error ?? endState.error ?? runNowState.error;
  if (!recurrence) return <p role={result.error ? "alert" : undefined} {...stylex.props(styles.oneTime, result.error && styles.error)}>{result.error ? "Schedule details could not be loaded." : `Repeating from ${dateLabel(schedule.scheduledFor, schedule.timeZone)}`}</p>;
  const commandInput = { recurrenceId, expectedRevision: recurrence.revision, clientMutationId: createClientId() };
  const change = async (kind: "pause" | "resume" | "skip") => {
    if (kind === "pause") await pause({ variables: { input: commandInput } });
    if (kind === "resume") await resume({ variables: { input: commandInput } });
    if (kind === "skip") await skip({ variables: { input: commandInput } });
    await result.refetch();
  };
  const active = recurrence.lifecycle === "ACTIVE";
  const ended = recurrence.lifecycle === "ENDED";
  return (
    <VStack as="section" aria-label="Recurring schedule" gap={2} className={stylex.props(styles.root).className}>
      <HStack justify="between" align="start" gap={2}>
        <VStack gap={0.5} className={stylex.props(styles.scheduleCopy).className}>
          <strong {...stylex.props(styles.scheduleTitle)}>{recurrenceSummary(recurrence.cronExpression)}</strong>
          <span {...stylex.props(styles.scheduleMeta)}>
            {ended ? "Ended · Occurrence · " : recurrence.lifecycle === "PAUSED" ? "Paused · Occurrence · " : "This occurrence · "}
            {dateLabel(schedule.scheduledFor, recurrence.timeZone)}
          </span>
          {active && recurrence.nextRunAt ? <span {...stylex.props(styles.nextRun)}>Following run · {dateLabel(recurrence.nextRunAt, recurrence.timeZone)}</span> : null}
        </VStack>
        {!ended ? (
          <DropdownMenu
            button={{ label: "Recurring schedule actions", icon: <MoreHorizontal aria-hidden="true" size={15} />, isIconOnly: true, size: "sm", variant: "ghost", isDisabled: busy, xstyle: styles.menuButton }}
            hasChevron={false}
            placement="above"
            menuWidth={210}
            items={[
              ...(canRunNow ? [{ label: "Run now", icon: <Play aria-hidden="true" size={14} />, onClick: () => void runNow({ variables: { input: commandInput } }).then(() => result.refetch()).catch(() => undefined), isDisabled: busy }] : []),
              { label: "Edit schedule", icon: <Pencil aria-hidden="true" size={14} />, onClick: () => setEditing(true), isDisabled: busy },
              { label: active ? "Pause future runs" : "Resume future runs", icon: active ? <Pause aria-hidden="true" size={14} /> : <Play aria-hidden="true" size={14} />, onClick: () => void change(active ? "pause" : "resume").catch(() => undefined), isDisabled: busy },
              { label: "Skip next run", icon: <SkipForward aria-hidden="true" size={14} />, onClick: () => void change("skip").catch(() => undefined), isDisabled: busy || !recurrence.nextRunAt },
              { type: "divider" },
              { label: "End recurring schedule", icon: <CircleStop aria-hidden="true" size={14} />, onClick: () => setConfirmingEnd(true), isDisabled: busy }
            ]}
          />
        ) : null}
      </HStack>
      {error ? <span role="alert" {...stylex.props(styles.error)}>{error.message}</span> : null}
      {recurrence.occurrences.length ? (
        <Collapsible trigger={<span {...stylex.props(styles.historyTrigger)}>Schedule history <span {...stylex.props(styles.historyCount)}>{recurrence.occurrences.length}</span></span>} defaultIsOpen={false}>
          <VStack gap={0} className={stylex.props(styles.historyList).className}>
            {recurrence.occurrences.map((occurrence) => occurrence.taskId ? (
              <Link key={`${occurrence.localSlot}:${occurrence.recurrenceRevision}`} to="/tasks/$taskId" params={{ taskId: occurrence.taskId }} search={(current) => normalizeTasksSearch(current)} {...stylex.props(styles.occurrence)}>
                <span>{dateLabel(occurrence.scheduledFor, recurrence.timeZone)}</span><span {...stylex.props(styles.occurrenceState)}>{occurrence.trigger === "MANUAL" ? "Run manually" : "Open task"}</span>
              </Link>
            ) : <HStack key={`${occurrence.localSlot}:${occurrence.recurrenceRevision}`} justify="between" className={stylex.props(styles.occurrence).className}><span>{dateLabel(occurrence.scheduledFor, recurrence.timeZone)}</span><span {...stylex.props(styles.occurrenceState)}>{occurrenceLabel(occurrence.resolution)}</span></HStack>)}
          </VStack>
        </Collapsible>
      ) : null}
      {editing ? <RecurrenceEditDialog recurrence={recurrence} onClose={() => setEditing(false)} onUpdated={() => result.refetch()} /> : null}
      {confirmingEnd ? <EndRecurrenceDialog submitting={endState.loading} error={endState.error?.message ?? null} onClose={() => setConfirmingEnd(false)} onConfirm={() => void end({ variables: { input: commandInput } }).then(async () => { await result.refetch(); setConfirmingEnd(false); }).catch(() => undefined)} /> : null}
    </VStack>
  );
}

function EndRecurrenceDialog({ submitting, error, onClose, onConfirm }: { submitting: boolean; error: string | null; onClose: () => void; onConfirm: () => void }) {
  return (
    <Dialog isOpen onOpenChange={(open) => { if (!open) onClose(); }} purpose="form" width={480} aria-label="End recurring schedule">
      <Layout height="auto" header={<DialogHeader title="End recurring schedule?" onOpenChange={(open) => { if (!open) onClose(); }} />} content={<LayoutContent>
        <VStack gap={3}>
          <HStack as="p" align="start" gap={2} className={stylex.props(styles.warning).className}><AlertTriangle aria-hidden="true" size={16} /><span>No future tasks will be created. This task and earlier runs stay unchanged.</span></HStack>
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
          <HStack justify="end" gap={2}><Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={submitting} onClick={onClose} /><Button type="button" size="sm" variant="destructive" label="End schedule" isLoading={submitting} isDisabled={submitting} onClick={onConfirm} /></HStack>
        </VStack>
      </LayoutContent>} />
    </Dialog>
  );
}

function RecurrenceEditDialog({ recurrence, onClose, onUpdated }: { recurrence: Recurrence; onClose: () => void; onUpdated: () => Promise<unknown> }) {
  const initial = () => ({ ...initialScheduleDraft({ scheduledFor: recurrence.startsAt, timeZone: recurrence.timeZone, missedRunPolicy: recurrence.missedRunPolicy, recurrenceId: recurrence.recurrenceId }), repeat: "custom" as const, cron: recurrence.cronExpression, overlapPolicy: recurrence.overlapPolicy });
  const [draft, setDraft] = React.useState<ScheduleDraft>(initial);
  const [update, state] = useMutation(TasksUpdateTaskRecurrenceDocument);
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

function dateLabel(value: string, timeZone: string) { return new Intl.DateTimeFormat(undefined, { timeZone, weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZoneName: "short" }).format(new Date(value)); }
function occurrenceLabel(resolution: Recurrence["occurrences"][number]["resolution"]) { return resolution === "SKIPPED" ? "Skipped" : resolution === "COALESCED" ? "Combined" : "Task created"; }
const styles = stylex.create({
  root: { minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)" },
  oneTime: { margin: 0, paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", color: "var(--noema-text-secondary)", fontSize: 12 },
  scheduleCopy: { minWidth: 0 },
  scheduleTitle: { minWidth: 0, color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 700, lineHeight: 1.35 },
  scheduleMeta: { minWidth: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.4, fontVariantNumeric: "tabular-nums", overflowWrap: "anywhere" },
  nextRun: { minWidth: 0, color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.4, fontVariantNumeric: "tabular-nums", overflowWrap: "anywhere" },
  menuButton: { width: 28, height: 28, flexShrink: 0 },
  historyTrigger: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)", color: "var(--noema-text-secondary)", fontSize: 12, fontWeight: 650 },
  historyCount: { color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 12, fontWeight: 500, fontVariantNumeric: "tabular-nums" },
  historyList: { paddingBlockStart: "var(--spacing-1)" },
  occurrence: { display: "flex", minWidth: 0, justifyContent: "space-between", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-1)", color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.4, fontVariantNumeric: "tabular-nums", textDecoration: "none" },
  occurrenceState: { flexShrink: 0, color: "var(--noema-text-muted)" },
  warning: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 },
  error: { color: "var(--destructive)", fontSize: 12 }
});
