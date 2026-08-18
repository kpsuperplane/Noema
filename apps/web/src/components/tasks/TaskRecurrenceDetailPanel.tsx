import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { AlertTriangle, CircleStop, MoreHorizontal, Pause, Pencil, Play, SkipForward } from "lucide-react";
import { MarkdownContent } from "@/components/MarkdownContent";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
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
import { initialScheduleDraft, scheduleInput, ScheduleFields, type ScheduleDraft } from "./ScheduleFields";
import { recurrenceSummary } from "./tasksModel";
import { normalizeTasksSearch } from "./tasksTypes";

type Recurrence = TasksTaskRecurrenceQuery["taskRecurrence"];

export function TaskRecurrenceDetailPanel({ recurrenceId, onTitleChange }: { recurrenceId: string; onTitleChange: (title: string) => void }) {
  const result = useQuery(TasksTaskRecurrenceDocument, { variables: { recurrenceId } });
  const [pause, pauseState] = useMutation(TasksPauseTaskRecurrenceDocument);
  const [resume, resumeState] = useMutation(TasksResumeTaskRecurrenceDocument);
  const [runNow, runNowState] = useMutation(TasksRunTaskRecurrenceNowDocument);
  const [skip, skipState] = useMutation(TasksSkipTaskRecurrenceNextDocument);
  const [end, endState] = useMutation(TasksEndTaskRecurrenceDocument);
  const [editing, setEditing] = React.useState(false);
  const [confirmingEnd, setConfirmingEnd] = React.useState(false);
  const recurrence = result.data?.taskRecurrence;
  React.useEffect(() => {
    if (recurrence?.title) onTitleChange(recurrence.title);
  }, [onTitleChange, recurrence?.title]);
  if (!recurrence) {
    return <div role={result.error ? "alert" : "status"} {...stylex.props(styles.state)}>{result.error ? "Recurring task details could not be loaded." : "Loading recurring task…"}</div>;
  }
  const busy = pauseState.loading || resumeState.loading || runNowState.loading || skipState.loading || endState.loading;
  const error = pauseState.error ?? resumeState.error ?? runNowState.error ?? skipState.error ?? endState.error;
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
    <VStack gap={4} className={stylex.props(styles.root).className}>
      <VStack as="section" aria-label="Recurring task schedule" gap={2} className={stylex.props(styles.section).className}>
        <HStack justify="between" align="start" gap={3}>
          <VStack gap={1}>
            <span {...stylex.props(styles.eyebrow)}>{ended ? "Ended" : active ? "Active" : "Paused"}</span>
            <strong {...stylex.props(styles.cadence)}>{recurrenceSummary(recurrence.cronExpression)}</strong>
            {active && recurrence.nextRunAt ? <span {...stylex.props(styles.meta)}>Next run · {dateLabel(recurrence.nextRunAt, recurrence.timeZone)}</span> : null}
            <span {...stylex.props(styles.meta)}>{recurrence.timeZone}</span>
          </VStack>
          {!ended ? <DropdownMenu
            button={{ label: "Recurring task actions", icon: <MoreHorizontal aria-hidden="true" size={16} />, isIconOnly: true, size: "sm", variant: "ghost", isDisabled: busy }}
            hasChevron={false}
            placement="below"
            menuWidth={210}
            items={[
              { label: "Run now", icon: <Play aria-hidden="true" size={14} />, onClick: () => void runNow({ variables: { input: commandInput } }).then(() => result.refetch()).catch(() => undefined), isDisabled: busy },
              { label: "Edit schedule", icon: <Pencil aria-hidden="true" size={14} />, onClick: () => setEditing(true), isDisabled: busy },
              { label: active ? "Pause future runs" : "Resume future runs", icon: active ? <Pause aria-hidden="true" size={14} /> : <Play aria-hidden="true" size={14} />, onClick: () => void change(active ? "pause" : "resume").catch(() => undefined), isDisabled: busy },
              { label: "Skip next run", icon: <SkipForward aria-hidden="true" size={14} />, onClick: () => void change("skip").catch(() => undefined), isDisabled: busy || !recurrence.nextRunAt },
              { type: "divider" },
              { label: "End recurring task", icon: <CircleStop aria-hidden="true" size={14} />, onClick: () => setConfirmingEnd(true), isDisabled: busy }
            ]}
          /> : null}
        </HStack>
        {recurrence.description.trim() ? <MarkdownContent className={stylex.props(styles.description).className}>{recurrence.description}</MarkdownContent> : null}
        {error ? <span role="alert" {...stylex.props(styles.error)}>{error.message}</span> : null}
      </VStack>
      <VStack as="section" aria-labelledby="recurrence-history-title" gap={2} className={stylex.props(styles.section).className}>
        <HStack justify="between" align="center" gap={2}>
          <h3 id="recurrence-history-title" {...stylex.props(styles.sectionTitle)}>Occurrences</h3>
          <span aria-label={`${recurrence.occurrences.length} occurrences`} {...stylex.props(styles.count)}>{recurrence.occurrences.length}</span>
        </HStack>
        {recurrence.occurrences.length ? <VStack as="ul" gap={0} className={stylex.props(styles.history).className}>
          {recurrence.occurrences.map((occurrence) => (
            <li key={`${occurrence.localSlot}:${occurrence.recurrenceRevision}`} {...stylex.props(styles.rowItem)}>
              {occurrence.taskId ? <Link to="/tasks/$taskId" params={{ taskId: occurrence.taskId }} search={(current) => normalizeTasksSearch(current)} {...stylex.props(styles.row)}>
                <span>{dateLabel(occurrence.scheduledFor, recurrence.timeZone)}</span>
                <span {...stylex.props(styles.rowState)}>{occurrence.trigger === "MANUAL" ? "Run manually" : "Open task"}</span>
              </Link> : <HStack justify="between" className={stylex.props(styles.row).className}>
                <span>{dateLabel(occurrence.scheduledFor, recurrence.timeZone)}</span>
                <span {...stylex.props(styles.rowState)}>{occurrenceLabel(occurrence.resolution)}</span>
              </HStack>}
            </li>
          ))}
        </VStack> : <p {...stylex.props(styles.empty)}>No occurrences yet.</p>}
      </VStack>
      {editing ? <RecurrenceEditDialog recurrence={recurrence} onClose={() => setEditing(false)} onUpdated={() => result.refetch()} /> : null}
      {confirmingEnd ? <EndRecurrenceDialog submitting={endState.loading} error={endState.error?.message ?? null} onClose={() => setConfirmingEnd(false)} onConfirm={() => void end({ variables: { input: commandInput } }).then(async () => { await result.refetch(); setConfirmingEnd(false); }).catch(() => undefined)} /> : null}
    </VStack>
  );
}

function EndRecurrenceDialog({ submitting, error, onClose, onConfirm }: { submitting: boolean; error: string | null; onClose: () => void; onConfirm: () => void }) {
  return <Dialog isOpen onOpenChange={(open) => { if (!open) onClose(); }} purpose="form" width={480} aria-label="End recurring task"><Layout height="auto" header={<DialogHeader title="End recurring task?" onOpenChange={(open) => { if (!open) onClose(); }} />} content={<LayoutContent><VStack gap={3}><HStack as="p" align="start" gap={2} className={stylex.props(styles.warning).className}><AlertTriangle aria-hidden="true" size={16} /><span>No future Tasks will be created. Existing occurrences stay unchanged.</span></HStack>{error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}<HStack justify="end" gap={2}><Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={submitting} onClick={onClose} /><Button type="button" size="sm" variant="destructive" label="End recurring task" isLoading={submitting} isDisabled={submitting} onClick={onConfirm} /></HStack></VStack></LayoutContent>} /></Dialog>;
}

function RecurrenceEditDialog({ recurrence, onClose, onUpdated }: { recurrence: Recurrence; onClose: () => void; onUpdated: () => Promise<unknown> }) {
  const initial = () => ({ ...initialScheduleDraft({ scheduledFor: recurrence.startsAt, timeZone: recurrence.timeZone, missedRunPolicy: recurrence.missedRunPolicy, recurrenceId: recurrence.recurrenceId }), repeat: "custom" as const, cron: recurrence.cronExpression, overlapPolicy: recurrence.overlapPolicy });
  const [draft, setDraft] = React.useState<ScheduleDraft>(initial);
  const [update, state] = useMutation(TasksUpdateTaskRecurrenceDocument);
  React.useEffect(() => { requestAnimationFrame(() => document.querySelector<HTMLElement>('[aria-label="Edit recurring task"] input')?.focus()); }, []);
  return <Dialog isOpen onOpenChange={(next) => { if (!next) onClose(); }} purpose="form" width={500} aria-label="Edit recurring task"><Layout height="auto" header={<DialogHeader title="Edit recurring task" subtitle="Changes apply to future occurrences." onOpenChange={(next) => { if (!next) onClose(); }} />} content={<LayoutContent><VStack as="form" gap={3} onSubmit={(event) => { event.preventDefault(); const next = scheduleInput(draft)?.recurrence; if (!next) return; void update({ variables: { input: { recurrenceId: recurrence.recurrenceId, expectedRevision: recurrence.revision, startsAt: next.startsAt, cronExpression: next.cronExpression, timeZone: draft.timeZone, missedRunPolicy: draft.missedRunPolicy, overlapPolicy: draft.overlapPolicy, clientMutationId: createClientId() } } }).then(async () => { await onUpdated(); onClose(); }).catch(() => undefined); }}><ScheduleFields value={draft} onChange={setDraft} recurringOnly />{state.error ? <span role="alert" {...stylex.props(styles.error)}>{state.error.message}</span> : null}<HStack justify="end" gap={2}><Button type="button" size="sm" variant="ghost" label="Cancel" onClick={onClose} /><Button type="submit" size="sm" variant="primary" label="Save" isLoading={state.loading} isDisabled={state.loading || !scheduleInput(draft)?.recurrence} /></HStack></VStack></LayoutContent>} /></Dialog>;
}

function dateLabel(value: string, timeZone: string) { return new Intl.DateTimeFormat(undefined, { timeZone, weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZoneName: "short" }).format(new Date(value)); }
function occurrenceLabel(resolution: Recurrence["occurrences"][number]["resolution"]) { return resolution === "SKIPPED" ? "Skipped" : resolution === "COALESCED" ? "Combined" : "Pending"; }

const styles = stylex.create({
  root: { minWidth: 0, maxWidth: 760, marginInline: "auto", padding: "var(--spacing-4)" },
  section: { minWidth: 0, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, padding: "var(--spacing-4)", backgroundColor: "var(--noema-surface-card)" },
  eyebrow: { color: "var(--noema-text-muted)", fontSize: 12, fontWeight: 650, textTransform: "uppercase", letterSpacing: "0.04em" },
  cadence: { color: "var(--noema-text-primary)", fontSize: 15, lineHeight: 1.35 },
  meta: { color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.4, fontVariantNumeric: "tabular-nums" },
  description: { marginBlockStart: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 13 },
  sectionTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 650 },
  count: { color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 12 },
  history: { margin: 0, padding: 0, listStyle: "none", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  rowItem: { minWidth: 0, listStyle: "none", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)" },
  row: { display: "flex", minWidth: 0, justifyContent: "space-between", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 12, fontVariantNumeric: "tabular-nums", textDecoration: "none" },
  rowState: { flexShrink: 0, color: "var(--noema-text-muted)" },
  empty: { margin: 0, color: "var(--noema-text-muted)", fontSize: 12 },
  state: { display: "grid", minHeight: "100%", placeItems: "center", padding: "var(--spacing-4)", color: "var(--noema-text-muted)", fontSize: 13 },
  warning: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 },
  error: { color: "var(--destructive)", fontSize: 12 }
});
