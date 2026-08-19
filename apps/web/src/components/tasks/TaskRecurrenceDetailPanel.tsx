import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { StatusDot } from "@astryxdesign/core/StatusDot";
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
import { TaskDocumentInlineEditor } from "./TaskMarkdownEditor";
import { isStaleCommandError } from "./semanticCommand";

type Recurrence = NonNullable<TasksTaskRecurrenceQuery["taskRecurrence"]>;

type RecurrenceEditField = "TITLE" | "DOCUMENT";

export type RecurrenceInlineEditController = {
  field: RecurrenceEditField | null;
  recurrence: Recurrence;
  canEdit: boolean;
  canStart: boolean;
  busy: boolean;
  error: string | null;
  requiresAcknowledgement: boolean;
  actionUnavailable: boolean;
  start: (field: RecurrenceEditField) => Promise<void>;
  cancel: () => void;
  saveTitle: (title: string) => Promise<void>;
  saveDocument: (taskDocument: string) => Promise<void>;
  acknowledge: () => Promise<void>;
};

export function TaskRecurrenceDetailPanel({ recurrenceId, onTitleChange }: { recurrenceId: string; onTitleChange: (title: string, edit: RecurrenceInlineEditController) => void }) {
  const result = useQuery(TasksTaskRecurrenceDocument, { variables: { recurrenceId } });
  const refetch = result.refetch;
  const reload = React.useCallback(async () => {
    const recurrence = (await refetch()).data?.taskRecurrence;
    if (!recurrence) throw new Error("Recurring task unavailable");
    return recurrence;
  }, [refetch]);
  if (!result.data?.taskRecurrence) {
    return (
      <VStack
        role={result.error ? "alert" : "status"}
        gap={2}
        align="center"
        justify="center"
        className={stylex.props(styles.state).className}
      >
        <span>{result.error ? "Recurring task details could not be loaded." : "Loading recurring task…"}</span>
        {result.error ? <Button type="button" size="sm" variant="secondary" label="Retry" onClick={() => void refetch().catch(() => undefined)} /> : null}
      </VStack>
    );
  }
  return <LoadedRecurrenceDetail recurrence={result.data.taskRecurrence} onReload={reload} onTitleChange={onTitleChange} />;
}

function LoadedRecurrenceDetail({ recurrence, onReload, onTitleChange }: { recurrence: Recurrence; onReload: () => Promise<Recurrence>; onTitleChange: (title: string, edit: RecurrenceInlineEditController) => void }) {
  const [pause, pauseState] = useMutation(TasksPauseTaskRecurrenceDocument);
  const [resume, resumeState] = useMutation(TasksResumeTaskRecurrenceDocument);
  const [runNow, runNowState] = useMutation(TasksRunTaskRecurrenceNowDocument);
  const [skip, skipState] = useMutation(TasksSkipTaskRecurrenceNextDocument);
  const [end, endState] = useMutation(TasksEndTaskRecurrenceDocument);
  const [update, updateState] = useMutation(TasksUpdateTaskRecurrenceDocument);
  const [editingSchedule, setEditingSchedule] = React.useState(false);
  const [confirmingEnd, setConfirmingEnd] = React.useState(false);
  const [activeEdit, setActiveEdit] = React.useState<{ field: RecurrenceEditField; subject: Recurrence } | null>(null);
  const commandBusy = pauseState.loading || resumeState.loading || runNowState.loading || skipState.loading || endState.loading;
  const busy = commandBusy || updateState.loading;
  const error = pauseState.error ?? resumeState.error ?? runNowState.error ?? skipState.error ?? endState.error;
  const editSubject = activeEdit?.subject ?? recurrence;
  const stale = updateState.error ? isStaleCommandError(updateState.error) : false;
  const liveEditChanged = Boolean(activeEdit && (
    recurrence.revision !== activeEdit.subject.revision ||
    recurrence.taskDocumentDigest !== activeEdit.subject.taskDocumentDigest
  ));
  const requiresAcknowledgement = liveEditChanged || stale;
  const actionUnavailable = Boolean(activeEdit && recurrence.lifecycle === "ENDED");
  const commandInput = { recurrenceId: recurrence.recurrenceId, expectedRevision: recurrence.revision, clientMutationId: createClientId() };
  const startEdit = React.useCallback(async (field: RecurrenceEditField) => {
    if (recurrence.lifecycle === "ENDED" || commandBusy || activeEdit || editingSchedule || confirmingEnd) return;
    updateState.reset();
    setActiveEdit({ field, subject: recurrence });
  }, [activeEdit, commandBusy, confirmingEnd, editingSchedule, recurrence, updateState]);
  const cancelEdit = React.useCallback(() => {
    updateState.reset();
    setActiveEdit(null);
  }, [updateState]);
  const saveEdit = React.useCallback(async (draft: { title?: string; taskDocument?: string }) => {
    if (!activeEdit || requiresAcknowledgement || actionUnavailable) return;
    const subject = activeEdit.subject;
    await update({ variables: { input: {
      recurrenceId: subject.recurrenceId,
      expectedRevision: subject.revision,
      title: draft.title ?? subject.title,
      taskDocument: draft.taskDocument ?? subject.taskDocument,
      expectedTaskDocumentDigest: subject.taskDocumentDigest,
      clientMutationId: createClientId()
    } } });
    await onReload();
    updateState.reset();
    setActiveEdit(null);
  }, [actionUnavailable, activeEdit, onReload, requiresAcknowledgement, update, updateState]);
  const acknowledge = React.useCallback(async () => {
    if (!activeEdit) return;
    const latest = await onReload();
    setActiveEdit({ field: activeEdit.field, subject: latest });
    updateState.reset();
  }, [activeEdit, onReload, updateState]);
  const edit = React.useMemo<RecurrenceInlineEditController>(() => ({
    field: activeEdit?.field ?? null,
    recurrence: editSubject,
    canEdit: recurrence.lifecycle !== "ENDED",
    canStart: recurrence.lifecycle !== "ENDED" && !commandBusy && !activeEdit && !editingSchedule && !confirmingEnd,
    busy,
    error: updateState.error ? stale ? "Changed elsewhere. Reload the latest version. Your draft is safe." : updateState.error.message : null,
    requiresAcknowledgement,
    actionUnavailable,
    start: startEdit,
    cancel: cancelEdit,
    saveTitle: async (title) => saveEdit({ title }),
    saveDocument: async (taskDocument) => saveEdit({ taskDocument }),
    acknowledge
  }), [acknowledge, actionUnavailable, activeEdit, busy, cancelEdit, commandBusy, confirmingEnd, editSubject, editingSchedule, recurrence.lifecycle, requiresAcknowledgement, saveEdit, stale, startEdit, updateState.error]);
  React.useEffect(() => {
    onTitleChange(recurrence.title, edit);
  }, [edit, onTitleChange, recurrence.title]);
  const change = async (kind: "pause" | "resume" | "skip") => {
    if (kind === "pause") await pause({ variables: { input: commandInput } });
    if (kind === "resume") await resume({ variables: { input: commandInput } });
    if (kind === "skip") await skip({ variables: { input: commandInput } });
    await onReload();
  };
  const active = recurrence.lifecycle === "ACTIVE";
  const ended = recurrence.lifecycle === "ENDED";
  const lifecycleLabel = ended ? "Ended" : active ? "Active" : "Paused";
  const occurrenceGroups = groupOccurrences(recurrence.occurrences, recurrence.timeZone);
  return (
    <VStack className={stylex.props(styles.scroller).className}>
      <VStack gap={4} className={stylex.props(styles.root).className}>
        <VStack as="section" aria-label="Recurring task schedule" gap={2} className={stylex.props(styles.section).className}>
          <HStack justify="between" align="start" gap={3}>
            <VStack gap={1}>
              <HStack align="center" gap={1}>
                <StatusDot variant={ended ? "neutral" : active ? "success" : "warning"} label={`${lifecycleLabel} recurring task`} />
                <span {...stylex.props(styles.eyebrow)}>{lifecycleLabel}</span>
              </HStack>
              <HStack align="center" gap={1}>
                <strong {...stylex.props(styles.cadence)}>{recurrenceSummary(recurrence.cronExpression)}</strong>
                {!ended ? <IconButton type="button" size="sm" variant="ghost" label="Edit schedule" tooltip="Edit schedule" icon={<Pencil aria-hidden="true" size={14} />} isDisabled={busy || Boolean(activeEdit)} onClick={() => setEditingSchedule(true)} /> : null}
              </HStack>
              {active && recurrence.nextRunAt ? <span {...stylex.props(styles.meta)}>Next run · {dateLabel(recurrence.nextRunAt, recurrence.timeZone)}</span> : null}
              <span {...stylex.props(styles.meta)}>{recurrence.timeZone}</span>
            </VStack>
            {!ended ? <DropdownMenu
              button={{ label: "Recurring task actions", icon: <MoreHorizontal aria-hidden="true" size={16} />, isIconOnly: true, size: "sm", variant: "ghost", isDisabled: busy || Boolean(activeEdit) }}
              hasChevron={false}
              placement="below"
              menuWidth={210}
              items={[
                { label: "Skip next run", icon: <SkipForward aria-hidden="true" size={14} />, onClick: () => void change("skip").catch(() => undefined), isDisabled: busy || !recurrence.nextRunAt },
                { type: "divider" },
                { label: "End recurring task", icon: <CircleStop aria-hidden="true" size={14} />, onClick: () => setConfirmingEnd(true), isDisabled: busy }
              ]}
            /> : null}
          </HStack>
          {!ended ? (
            <HStack gap={2} wrap="wrap">
              <Button
                type="button"
                size="sm"
                variant="secondary"
                label="Run now"
                icon={<Play aria-hidden="true" size={14} />}
                isLoading={runNowState.loading}
                isDisabled={busy || Boolean(activeEdit)}
                onClick={() => void runNow({ variables: { input: commandInput } }).then(onReload).catch(() => undefined)}
              />
              <Button
                type="button"
                size="sm"
                variant="ghost"
                label={active ? "Pause schedule" : "Resume schedule"}
                icon={active ? <Pause aria-hidden="true" size={14} /> : <Play aria-hidden="true" size={14} />}
                isLoading={active ? pauseState.loading : resumeState.loading}
                isDisabled={busy || Boolean(activeEdit)}
                onClick={() => void change(active ? "pause" : "resume").catch(() => undefined)}
              />
            </HStack>
          ) : null}
          <RecurrenceDescription recurrence={recurrence} edit={edit} />
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error.message}</span> : null}
        </VStack>
        <VStack as="section" aria-labelledby="recurrence-history-title" gap={2} className={stylex.props(styles.section).className}>
          <HStack justify="between" align="center" gap={2}>
            <h3 id="recurrence-history-title" {...stylex.props(styles.sectionTitle)}>Occurrences</h3>
            <span aria-label={`${recurrence.occurrences.length} occurrences`} {...stylex.props(styles.count)}>{recurrence.occurrences.length}</span>
          </HStack>
          {occurrenceGroups.length ? <VStack gap={3}>
            {occurrenceGroups.map((group) => <VStack as="section" gap={1} key={group.label} aria-label={group.label}>
              <h4 {...stylex.props(styles.occurrenceDay)}>{group.label}</h4>
              <VStack as="ul" gap={0} className={stylex.props(styles.history).className}>
                {group.occurrences.map((occurrence) => (
                  <li key={`${occurrence.localSlot}:${occurrence.recurrenceRevision}`} {...stylex.props(styles.rowItem)}>
                    {occurrence.taskId ? <Link to="/tasks/$taskId" params={{ taskId: occurrence.taskId }} search={(current) => normalizeTasksSearch(current)} {...stylex.props(styles.row, styles.rowLink)}>
                      <VStack gap={0.5} className={stylex.props(styles.rowIdentity).className}>
                        <time dateTime={occurrence.scheduledFor} {...stylex.props(styles.rowDate)}>{timeLabel(occurrence.scheduledFor, recurrence.timeZone)}</time>
                        <span {...stylex.props(styles.rowMeta)}>{occurrenceTriggerLabel(occurrence.trigger)}</span>
                      </VStack>
                      <span {...stylex.props(styles.rowState)}>Open task</span>
                    </Link> : <HStack justify="between" align="start" className={stylex.props(styles.row).className}>
                      <VStack gap={0.5} className={stylex.props(styles.rowIdentity).className}>
                        <time dateTime={occurrence.scheduledFor} {...stylex.props(styles.rowDate)}>{timeLabel(occurrence.scheduledFor, recurrence.timeZone)}</time>
                        <span {...stylex.props(styles.rowMeta)}>{occurrenceTriggerLabel(occurrence.trigger)}</span>
                      </VStack>
                      <span {...stylex.props(styles.rowState)}>{occurrenceLabel(occurrence.resolution)}</span>
                    </HStack>}
                  </li>
                ))}
              </VStack>
            </VStack>)}
          </VStack> : <p {...stylex.props(styles.empty)}>No occurrences yet.</p>}
        </VStack>
        {editingSchedule ? <RecurrenceScheduleDialog recurrence={recurrence} onClose={() => setEditingSchedule(false)} onUpdated={onReload} /> : null}
        {confirmingEnd ? <EndRecurrenceDialog submitting={endState.loading} error={endState.error?.message ?? null} onClose={() => setConfirmingEnd(false)} onConfirm={() => void end({ variables: { input: commandInput } }).then(async () => { await onReload(); setConfirmingEnd(false); }).catch(() => undefined)} /> : null}
      </VStack>
    </VStack>
  );
}

function RecurrenceDescription({ recurrence, edit }: { recurrence: Recurrence; edit: RecurrenceInlineEditController }) {
  const [saved, setSaved] = React.useState(false);
  React.useEffect(() => {
    if (!saved) return;
    const timeout = window.setTimeout(() => setSaved(false), 1600);
    return () => window.clearTimeout(timeout);
  }, [saved]);
  const saveDocument = async (taskDocument: string) => { await edit.saveDocument(taskDocument); setSaved(true); };
  if (edit.field === "DOCUMENT") return <TaskDocumentInlineEditor key={`${recurrence.recurrenceId}:description`} document={edit.recurrence.taskDocument} digest={edit.recurrence.taskDocumentDigest} edit={edit} label="Recurring task description" scope="Future runs only" onSave={saveDocument} />;
  return (
    <VStack as="section" aria-labelledby="recurrence-description-title" gap={2}>
      <RecurrenceDescriptionBar edit={edit} saved={saved} />
      {recurrence.taskDocument.trim() ? <MarkdownContent density="compact" className={stylex.props(styles.description).className}>{recurrence.taskDocument}</MarkdownContent> : <p {...stylex.props(styles.empty, styles.description)}>The template is empty.</p>}
    </VStack>
  );
}

function RecurrenceDescriptionBar({ edit, saved = false }: { edit: RecurrenceInlineEditController; saved?: boolean }) {
  const label = <HStack align="center" gap={1}>
    <strong id="recurrence-description-title" {...stylex.props(styles.descriptionLabel)}>Description</strong>
    {edit.canEdit ? <IconButton type="button" size="sm" variant="ghost" label="Edit description" tooltip="Edit description" icon={<Pencil aria-hidden="true" size={14} />} isDisabled={edit.busy || !edit.canStart} onClick={() => void edit.start("DOCUMENT")} /> : null}
    <span {...stylex.props(styles.descriptionScope)}>Future runs only</span>
    {edit.canEdit ? <span role="status" aria-live="polite" {...stylex.props(styles.savedStatus)}>{saved ? "Saved" : null}</span> : null}
  </HStack>;
  return <HStack align="center" className={stylex.props(styles.descriptionBar).className}>{label}</HStack>;
}

function EndRecurrenceDialog({ submitting, error, onClose, onConfirm }: { submitting: boolean; error: string | null; onClose: () => void; onConfirm: () => void }) {
  return <Dialog isOpen onOpenChange={(open) => { if (!open) onClose(); }} purpose="form" width={480} aria-label="End recurring task"><Layout height="auto" header={<DialogHeader title="End recurring task?" onOpenChange={(open) => { if (!open) onClose(); }} />} content={<LayoutContent><VStack gap={3}><HStack as="p" align="start" gap={2} className={stylex.props(styles.warning).className}><AlertTriangle aria-hidden="true" size={16} /><span>No future Tasks will be created. Existing occurrences stay unchanged.</span></HStack>{error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}<HStack justify="end" gap={2}><Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={submitting} onClick={onClose} /><Button type="button" size="sm" variant="destructive" label="End recurring task" isLoading={submitting} isDisabled={submitting} onClick={onConfirm} /></HStack></VStack></LayoutContent>} /></Dialog>;
}

function RecurrenceScheduleDialog({ recurrence, onClose, onUpdated }: { recurrence: Recurrence; onClose: () => void; onUpdated: () => Promise<unknown> }) {
  const initial = () => ({ ...initialScheduleDraft({ scheduledFor: recurrence.startsAt, timeZone: recurrence.timeZone, missedRunPolicy: recurrence.missedRunPolicy, recurrenceId: recurrence.recurrenceId }), repeat: "custom" as const, cron: recurrence.cronExpression, overlapPolicy: recurrence.overlapPolicy });
  const [draft, setDraft] = React.useState<ScheduleDraft>(initial);
  const [update, state] = useMutation(TasksUpdateTaskRecurrenceDocument);
  React.useEffect(() => { requestAnimationFrame(() => document.querySelector<HTMLElement>('[aria-label="Edit recurring task"] input')?.focus()); }, []);
  return <Dialog isOpen onOpenChange={(next) => { if (!next) onClose(); }} purpose="form" width={500} aria-label="Edit recurring task"><Layout height="auto" header={<DialogHeader title="Edit recurring task" subtitle="Changes apply to future occurrences." onOpenChange={(next) => { if (!next) onClose(); }} />} content={<LayoutContent><VStack as="form" gap={3} onSubmit={(event) => { event.preventDefault(); const next = scheduleInput(draft)?.recurrence; if (!next) return; void update({ variables: { input: { recurrenceId: recurrence.recurrenceId, expectedRevision: recurrence.revision, startsAt: next.startsAt, cronExpression: next.cronExpression, timeZone: draft.timeZone, missedRunPolicy: draft.missedRunPolicy, overlapPolicy: draft.overlapPolicy, clientMutationId: createClientId() } } }).then(async () => { await onUpdated(); onClose(); }).catch(() => undefined); }}><ScheduleFields value={draft} onChange={setDraft} recurringOnly />{state.error ? <span role="alert" {...stylex.props(styles.error)}>{state.error.message}</span> : null}<HStack justify="end" gap={2}><Button type="button" size="sm" variant="ghost" label="Cancel" onClick={onClose} /><Button type="submit" size="sm" variant="primary" label="Save" isLoading={state.loading} isDisabled={state.loading || !scheduleInput(draft)?.recurrence} /></HStack></VStack></LayoutContent>} /></Dialog>;
}

function dateLabel(value: string, timeZone: string) { return new Intl.DateTimeFormat(undefined, { timeZone, weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZoneName: "short" }).format(new Date(value)); }
function timeLabel(value: string, timeZone: string) { return new Intl.DateTimeFormat(undefined, { timeZone, hour: "numeric", minute: "2-digit", timeZoneName: "short" }).format(new Date(value)); }
function groupOccurrences(occurrences: Recurrence["occurrences"], timeZone: string) {
  const formatter = new Intl.DateTimeFormat(undefined, { timeZone, weekday: "long", month: "short", day: "numeric", year: "numeric" });
  const groups = new Map<string, Array<Recurrence["occurrences"][number]>>();
  for (const occurrence of occurrences) {
    const label = formatter.format(new Date(occurrence.scheduledFor));
    const group = groups.get(label) ?? [];
    group.push(occurrence);
    groups.set(label, group);
  }
  return Array.from(groups, ([label, groupedOccurrences]) => ({ label, occurrences: groupedOccurrences }));
}
function occurrenceLabel(resolution: Recurrence["occurrences"][number]["resolution"]) { return resolution === "SKIPPED" ? "Skipped" : resolution === "COALESCED" ? "Combined" : "Created"; }
function occurrenceTriggerLabel(trigger: Recurrence["occurrences"][number]["trigger"]) { return trigger === "MANUAL" ? "Manual run" : "Scheduled run"; }

const styles = stylex.create({
  scroller: { width: "100%", height: "100%", minWidth: 0, minHeight: 0, overflowY: "auto", overscrollBehavior: "contain", scrollbarWidth: "thin" },
  root: { boxSizing: "border-box", width: "100%", minWidth: 0, maxWidth: 760, marginInline: "auto", padding: "var(--spacing-4)" },
  section: { minWidth: 0 },
  eyebrow: { color: "var(--noema-text-muted)", fontSize: 12, fontWeight: 650, textTransform: "uppercase", letterSpacing: "0.04em" },
  cadence: { color: "var(--noema-text-primary)", fontSize: 15, lineHeight: 1.35 },
  meta: { color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.4, fontVariantNumeric: "tabular-nums" },
  description: { paddingInline: "var(--spacing-2)" },
  descriptionBar: { minHeight: 36 },
  descriptionLabel: { color: "var(--noema-text-muted)", fontSize: 12, fontWeight: 650 },
  descriptionScope: { color: "var(--noema-text-muted)", fontSize: 11 },
  savedStatus: { minWidth: "4ch", color: "var(--noema-text-secondary)", fontSize: 11, fontWeight: 650 },
  sectionTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 650 },
  count: { color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 12 },
  occurrenceDay: { margin: 0, color: "var(--noema-text-muted)", fontSize: 12, fontWeight: 650 },
  history: { margin: 0, padding: 0, listStyle: "none", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  rowItem: { minWidth: 0, listStyle: "none", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)" },
  row: { display: "flex", minWidth: 0, alignItems: "flex-start", justifyContent: "space-between", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 12, fontVariantNumeric: "tabular-nums", textDecoration: "none" },
  rowLink: { borderRadius: "var(--radius-element)", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 2 } },
  rowIdentity: { minWidth: 0 },
  rowDate: { color: "var(--noema-text-secondary)", lineHeight: 1.4, overflowWrap: "anywhere" },
  rowMeta: { color: "var(--noema-text-muted)", lineHeight: 1.35 },
  rowState: { flexShrink: 0, color: "var(--noema-text-muted)", lineHeight: 1.4 },
  empty: { margin: 0, color: "var(--noema-text-muted)", fontSize: 12 },
  state: { minHeight: "100%", padding: "var(--spacing-4)", color: "var(--noema-text-muted)", fontSize: 13 },
  warning: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 },
  error: { color: "var(--destructive)", fontSize: 12 }
});
