import { TaskScheduleSummary } from "./TaskScheduleSummary";
import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { Grid } from "@astryxdesign/core/Grid";
import { HStack } from "@astryxdesign/core/HStack";
import { Selector } from "@astryxdesign/core/Selector";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { CalendarClock, CircleStop, MoreHorizontal, Pause, Play, SkipForward } from "lucide-react";
import { TaskInstructionsField, useTaskEditFlush } from "./TaskDocumentFields";
import { TaskCard } from "./TasksViews";
import { taskStatusFromProjection } from "@/components/chatDetail/task/TaskStatusBadge";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import {
  TasksEndTaskRecurrenceDocument,
  TasksPauseTaskRecurrenceDocument,
  TasksResumeTaskRecurrenceDocument,
  TasksRunTaskRecurrenceNowDocument,
  TasksSkipTaskRecurrenceNextDocument,
  TasksTaskRecurrenceDocument,
  TasksRecurrenceRunCardDocument,
  TasksUpdateTaskRecurrenceDocument,
  type TasksTaskRecurrenceQuery
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { initialRecurrenceScheduleDraft, scheduleInput, ScheduleFields, type ScheduleDraft } from "./ScheduleFields";
import { recurrenceSummary } from "./tasksModel";
import { isStaleCommandError } from "./semanticCommand";

type Recurrence = NonNullable<TasksTaskRecurrenceQuery["taskRecurrence"]>;

type RecurrenceEditField = "TITLE" | "DOCUMENT";

export type RecurrenceInlineEditController = {
  flush: () => Promise<boolean>;
  registerFlush: (flush: (() => Promise<boolean>) | null) => void;
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

export function TaskRecurrenceDetailPanel({ recurrenceId, header, onTitleChange }: { recurrenceId: string; header?: React.ReactNode; onTitleChange: (title: string, edit: RecurrenceInlineEditController) => void }) {
  const result = useQuery(TasksTaskRecurrenceDocument, { variables: { recurrenceId } });
  const refetch = result.refetch;
  const reload = React.useCallback(async () => {
    const recurrence = (await refetch()).data?.taskRecurrence;
    if (!recurrence) throw new Error("Recurring task unavailable");
    return recurrence;
  }, [refetch]);
  if (!result.data?.taskRecurrence) {
    return (
      <VStack gap={0} className={stylex.props(styles.viewport).className}>
        <Grid height="100%" xstyle={styles.frame}>
        <VStack className={stylex.props(styles.heading).className}>{header}</VStack>
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
        </Grid>
      </VStack>
    );
  }
  return <LoadedRecurrenceDetail header={header} recurrence={result.data.taskRecurrence} onReload={reload} onTitleChange={onTitleChange} />;
}

function LoadedRecurrenceDetail({ recurrence, header, onReload, onTitleChange }: { recurrence: Recurrence; header?: React.ReactNode; onReload: () => Promise<Recurrence>; onTitleChange: (title: string, edit: RecurrenceInlineEditController) => void }) {
  const { flush, registerFlush } = useTaskEditFlush();
  const commandPending = React.useRef(false);
  const [pause, pauseState] = useMutation(TasksPauseTaskRecurrenceDocument);
  const [resume, resumeState] = useMutation(TasksResumeTaskRecurrenceDocument);
  const [runNow, runNowState] = useMutation(TasksRunTaskRecurrenceNowDocument);
  const [skip, skipState] = useMutation(TasksSkipTaskRecurrenceNextDocument);
  const [end, endState] = useMutation(TasksEndTaskRecurrenceDocument);
  const [update, updateState] = useMutation(TasksUpdateTaskRecurrenceDocument);
  const [updatePolicy, policyState] = useMutation(TasksUpdateTaskRecurrenceDocument);
  const [editingSchedule, setEditingSchedule] = React.useState(false);
  const [confirmingEnd, setConfirmingEnd] = React.useState(false);
  const [activeEdit, setActiveEdit] = React.useState<{ field: RecurrenceEditField; subject: Recurrence } | null>(null);
  const commandBusy = pauseState.loading || resumeState.loading || runNowState.loading || skipState.loading || endState.loading;
  const busy = commandBusy || updateState.loading || policyState.loading;
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
    if (recurrence.lifecycle === "ENDED" || commandBusy || editingSchedule || confirmingEnd || activeEdit?.field === field) return;
    if (activeEdit && !await flush()) return;
    const subject = activeEdit ? await onReload() : recurrence;
    if (subject.lifecycle === "ENDED") return;
    updateState.reset();
    setActiveEdit({ field, subject });
  }, [activeEdit, commandBusy, confirmingEnd, editingSchedule, recurrence, updateState, flush, onReload]);
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
    flush,
    registerFlush,
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
  }), [flush, registerFlush, acknowledge, actionUnavailable, activeEdit, busy, cancelEdit, commandBusy, confirmingEnd, editSubject, editingSchedule, recurrence.lifecycle, requiresAcknowledgement, saveEdit, stale, startEdit, updateState.error]);
  React.useEffect(() => {
    onTitleChange(recurrence.title, edit);
  }, [edit, onTitleChange, recurrence.title]);
  const currentCommand = async () => {
    if (!await edit.flush()) return null;
    const latest = await onReload();
    return { recurrenceId: latest.recurrenceId, expectedRevision: latest.revision, clientMutationId: createClientId() };
  };
  const change = async (kind: "pause" | "resume" | "skip" | "run" | "end") => {
    if (commandPending.current) return;
    commandPending.current = true;
    try {
      const input = await currentCommand();
      if (!input) return;
      if (kind === "pause") await pause({ variables: { input } });
      if (kind === "resume") await resume({ variables: { input } });
      if (kind === "skip") await skip({ variables: { input } });
      if (kind === "run") await runNow({ variables: { input } });
      if (kind === "end") await end({ variables: { input } });
      await onReload();
      if (kind === "end") setConfirmingEnd(false);
    } finally { commandPending.current = false; }
  };
  const openDialog = async (setOpen: (open: boolean) => void) => {
    if (!await edit.flush()) return;
    setOpen(true);
  };
  const active = recurrence.lifecycle === "ACTIVE";
  const ended = recurrence.lifecycle === "ENDED";
  const lifecycleLabel = ended ? "Ended" : active ? "Active" : "Paused";
  const occurrenceGroups = groupOccurrences(recurrence.occurrences, recurrence.timeZone);
  const savePolicy = async (policy: { missedRunPolicy?: Recurrence["missedRunPolicy"]; overlapPolicy?: Recurrence["overlapPolicy"] }) => {
    if (busy || activeEdit || ended) return;
    await updatePolicy({ variables: { input: { ...commandInput, ...policy } } });
    await onReload();
  };
  return (
    <VStack gap={0} className={stylex.props(styles.viewport).className}>
      <Grid height="100%" xstyle={styles.frame}>
      <VStack gap={0} className={stylex.props(styles.heading).className}>{header}</VStack>
      <VStack gap={0} className={stylex.props(styles.content).className}>
      <VStack as="section" aria-label="Recurring task details" gap={3} className={stylex.props(styles.root, styles.settings).className}>
        <span {...stylex.props(styles.meta)}>{recurrenceSummary(recurrence.cronExpression)} · {recurrence.timeZone}</span>
        <TaskScheduleSummary label="Next run" value={active && recurrence.nextRunAt ? dateLabel(recurrence.nextRunAt, recurrence.timeZone) : ended ? "This schedule has ended" : "Schedule paused"} paused={!active}>
          {!ended ? <Button size="sm" variant="ghost" label="Reschedule" icon={<CalendarClock size={14} aria-hidden="true" />} isDisabled={busy} onClick={() => void openDialog(setEditingSchedule)} /> : null}
        </TaskScheduleSummary>
        <TaskInstructionsField key={recurrence.recurrenceId} value={edit.recurrence.taskDocument} edit={edit} scope="Changes apply to future runs. " />
        {!ended ? <VStack as="section" aria-label="Advanced settings" gap={3} className={stylex.props(styles.advanced).className}>
          <h3 {...stylex.props(styles.sectionTitle)}>Advanced settings</h3>
          <Selector label="If a run was missed" size="sm" options={[{ value: "RUN_ONCE", label: "Run once" }, { value: "SKIP", label: "Skip" }]} value={recurrence.missedRunPolicy} isDisabled={busy || Boolean(activeEdit)} onChange={(value) => void savePolicy({ missedRunPolicy: value as Recurrence["missedRunPolicy"] }).catch(() => undefined)} />
          <Selector label="If another run is active" size="sm" options={[{ value: "SKIP", label: "Skip" }, { value: "QUEUE_ONE", label: "Queue one" }, { value: "ALLOW", label: "Allow overlap" }]} value={recurrence.overlapPolicy} isDisabled={busy || Boolean(activeEdit)} onChange={(value) => void savePolicy({ overlapPolicy: value as Recurrence["overlapPolicy"] }).catch(() => undefined)} />
          {policyState.error ? <HStack gap={2} wrap="wrap"><span role="alert" {...stylex.props(styles.error)}>{isStaleCommandError(policyState.error) ? "Changed elsewhere. Load the latest settings and try again." : policyState.error.message}</span><Button size="sm" variant="secondary" label="Reload settings" onClick={() => void onReload().then(() => policyState.reset()).catch(() => undefined)} /></HStack> : null}
        </VStack> : null}
        {error ? <span role="alert" {...stylex.props(styles.error)}>{error.message}</span> : null}
      </VStack>
        <VStack as="section" aria-labelledby="recurrence-history-title" gap={2} className={stylex.props(styles.root, styles.historyPane).className}>
          <HStack justify="between" align="center" gap={2}>
            <h3 id="recurrence-history-title" {...stylex.props(styles.sectionTitle)}>Run history</h3>
            <span aria-label={`${recurrence.occurrences.length} occurrences`} {...stylex.props(styles.count)}>{recurrence.occurrences.length}</span>
          </HStack>
          {occurrenceGroups.length ? <VStack gap={3}>
            {occurrenceGroups.map((group) => <VStack as="section" gap={2} key={group.label} aria-label={group.label}>
              <h4 {...stylex.props(styles.occurrenceDay)}>{group.label}</h4>
              <VStack as="ul" gap={2} className={stylex.props(styles.history).className}>
                {group.occurrences.map((occurrence) => occurrence.taskId
                  ? <RecurrenceRunCard key={occurrence.taskId} taskId={occurrence.taskId} />
                  : <HStack as="li" key={`${occurrence.localSlot}:${occurrence.recurrenceRevision}`} justify="between" gap={2}>
                    <time dateTime={occurrence.scheduledFor} {...stylex.props(styles.meta)}>{timeLabel(occurrence.scheduledFor, recurrence.timeZone)}</time>
                    <span {...stylex.props(styles.meta)}>{occurrenceLabel(occurrence.resolution)}</span>
                  </HStack>)}
              </VStack>
            </VStack>)}
          </VStack> : <p {...stylex.props(styles.empty)}>No runs yet.</p>}
        </VStack>
      </VStack>
      <HStack as="aside" aria-label="Recurring task actions" justify="between" align="center" gap={2} className={stylex.props(styles.actionBar).className}>
        <HStack gap={2} align="center"><StatusDot variant={ended ? "neutral" : active ? "success" : "warning"} label={`${lifecycleLabel} recurring task`} /><span {...stylex.props(styles.meta)}>{lifecycleLabel}</span></HStack>
        {!ended ? <HStack gap={1}>
          <Button type="button" size="sm" variant="ghost" isIconOnly label="Run now" tooltip="Run now" icon={<Play size={16} aria-hidden="true" />} isLoading={runNowState.loading} isDisabled={busy} onClick={() => void change("run").catch(() => undefined)} />
          <Button type="button" size="sm" variant="ghost" isIconOnly label={active ? "Pause schedule" : "Resume schedule"} tooltip={active ? "Pause schedule" : "Resume schedule"} icon={active ? <Pause size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />} isDisabled={busy} onClick={() => void change(active ? "pause" : "resume").catch(() => undefined)} />
          <DropdownMenu button={{ label: "More recurring task actions", icon: <MoreHorizontal size={16} aria-hidden="true" />, isIconOnly: true, size: "sm", variant: "ghost", isDisabled: busy }} hasChevron={false} placement="above" menuWidth={210} items={[
            { label: "Skip next run", icon: <SkipForward size={14} aria-hidden="true" />, onClick: () => void change("skip").catch(() => undefined), isDisabled: busy || !recurrence.nextRunAt },
            { type: "divider" },
            { label: "End recurring task", icon: <CircleStop size={14} aria-hidden="true" />, onClick: () => void openDialog(setConfirmingEnd), isDisabled: busy }
          ]} />
        </HStack> : null}
      </HStack>
      </Grid>
      {editingSchedule ? <RecurrenceScheduleDialog recurrence={recurrence} onClose={() => setEditingSchedule(false)} onUpdated={onReload} /> : null}
      {confirmingEnd ? <EndRecurrenceDialog recurrence={recurrence} submitting={endState.loading} error={endState.error?.message ?? null} onClose={() => setConfirmingEnd(false)} onConfirm={() => void change("end").catch(() => undefined)} /> : null}
    </VStack>
  );
}

function RecurrenceRunCard({ taskId }: { taskId: string }) {
  const result = useQuery(TasksRecurrenceRunCardDocument, { variables: { taskId } });
  const task = result.data?.task;
  if (!task) return <VStack as="li" gap={1}><span role={result.error ? "alert" : "status"}>{result.error ? "Could not load this run." : "Loading run…"}</span>{result.error ? <Button size="sm" variant="ghost" label="Retry" onClick={() => void result.refetch().catch(() => undefined)} /> : null}</VStack>;
  return <TaskCard taskId={task.taskId} title={task.title} note={task.taskDocument.slice(0, 240)} project={task.project?.name} status={taskStatusFromProjection(task)} statusLabel={task.stage.name} timestamp={task.completedAt ?? task.updatedAt} />;
}

function RecurrenceDialogCard({ recurrence, onClose }: { recurrence: Recurrence; onClose: () => void }) {
  return <TaskCard recurrenceId={recurrence.recurrenceId} title={recurrence.title} note={recurrence.taskDocument.slice(0, 240)} status="queued" statusLabel={recurrence.lifecycle === "ENDED" ? "Ended" : recurrence.lifecycle === "PAUSED" ? "Paused" : "Recurring"} listItem={false} onClick={onClose} />;
}

function EndRecurrenceDialog({ recurrence, submitting, error, onClose, onConfirm }: { recurrence: Recurrence; submitting: boolean; error: string | null; onClose: () => void; onConfirm: () => void }) {
  const formId = React.useId();
  return <Dialog isOpen onOpenChange={(open) => { if (!open) onClose(); }} purpose="form" width={480} aria-label="End recurring task"><Layout height="auto" header={<DialogHeader title="End recurring task?" onOpenChange={(open) => { if (!open) onClose(); }} />} content={<LayoutContent><VStack as="form" id={formId} gap={3} onSubmit={(event) => { event.preventDefault(); if (!submitting) onConfirm(); }}><RecurrenceDialogCard recurrence={recurrence} onClose={onClose} /><p {...stylex.props(styles.meta)}>Noema will create no more runs. Existing tasks and their history stay available.</p>{error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}</VStack></LayoutContent>} footer={<LayoutFooter><HStack gap={2} className={stylex.props(styles.dialogActions).className}><Button type="button" size="md" variant="secondary" label="Keep schedule" isDisabled={submitting} onClick={onClose} /><Button type="submit" form={formId} size="md" variant="destructive" label="End recurring task" isLoading={submitting} isDisabled={submitting} /></HStack></LayoutFooter>} /></Dialog>;
}

function RecurrenceScheduleDialog({ recurrence, onClose, onUpdated }: { recurrence: Recurrence; onClose: () => void; onUpdated: () => Promise<Recurrence> }) {
  const formId = React.useId();
  const [draft, setDraft] = React.useState<ScheduleDraft>(() => initialRecurrenceScheduleDraft(recurrence));
  const [revision, setRevision] = React.useState(recurrence.revision);
  const [update, state] = useMutation(TasksUpdateTaskRecurrenceDocument);
  const stale = recurrence.revision !== revision || Boolean(state.error && isStaleCommandError(state.error));
  React.useEffect(() => { requestAnimationFrame(() => document.querySelector<HTMLElement>('[aria-label="Reschedule recurring task"] input')?.focus()); }, []);
  return <Dialog isOpen onOpenChange={(next) => { if (!next) onClose(); }} purpose="form" width={500} aria-label="Reschedule recurring task"><Layout height="auto" header={<DialogHeader title="Reschedule" onOpenChange={(next) => { if (!next) onClose(); }} />} content={<LayoutContent><VStack as="form" id={formId} gap={3} onSubmit={(event) => { event.preventDefault(); const next = scheduleInput(draft)?.recurrence; if (!next || stale || state.loading) return; void update({ variables: { input: { recurrenceId: recurrence.recurrenceId, expectedRevision: revision, startsAt: next.startsAt, cronExpression: next.cronExpression, timeZone: draft.timeZone, clientMutationId: createClientId() } } }).then(async () => { await onUpdated(); onClose(); }).catch(() => undefined); }}><RecurrenceDialogCard recurrence={recurrence} onClose={onClose} /><ScheduleFields value={draft} onChange={setDraft} recurringOnly showPolicies={false} />{stale ? <HStack gap={2} wrap="wrap"><span role="alert">Changed elsewhere. Your schedule draft is safe.</span><Button size="sm" variant="secondary" label="Use latest version" onClick={() => void onUpdated().then((latest) => { setRevision(latest.revision); state.reset(); }).catch(() => undefined)} /></HStack> : state.error ? <span role="alert" {...stylex.props(styles.error)}>{state.error.message}</span> : null}</VStack></LayoutContent>} footer={<LayoutFooter><HStack gap={2} className={stylex.props(styles.dialogActions).className}><Button type="button" size="md" variant="secondary" label="Cancel" onClick={onClose} /><Button type="submit" form={formId} size="md" variant="primary" label="Save schedule" isLoading={state.loading} isDisabled={state.loading || stale || !scheduleInput(draft)?.recurrence} /></HStack></LayoutFooter>} /></Dialog>;
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

const styles = stylex.create({
  viewport: { width: "100%", height: "100%", minWidth: 0, minHeight: 0, containerType: "inline-size" },
  frame: { width: "100%", minWidth: 0, minHeight: 0, gridTemplateColumns: "minmax(0, 1fr)", gridTemplateRows: "auto minmax(0, 1fr) auto", overflow: "hidden", "@container (width > 1200px)": { gridTemplateColumns: "minmax(0, 1fr) 600px" } },
  heading: { gridColumn: "1", gridRow: "1", minWidth: 0, paddingBlockEnd: "var(--spacing-2)", borderBottomWidth: "var(--border-width)", borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)" },
  content: { gridColumn: "1", gridRow: "2", minWidth: 0, minHeight: 0, overflowY: "auto", overscrollBehavior: "contain", scrollbarWidth: "thin", "@container (width > 1200px)": { display: "contents" } },
  root: { boxSizing: "border-box", width: "calc(100% - var(--spacing-6) - var(--spacing-6))", minWidth: 0, maxWidth: 760, marginInline: "auto", paddingBlock: "var(--spacing-4)", flexShrink: 0 },
  settings: { "@container (width > 1200px)": { width: "calc(100% - var(--spacing-6))", maxWidth: "calc(760px + var(--spacing-6))", paddingInline: "var(--spacing-3)", gridColumn: "1", gridRow: "2", minHeight: 0, overflowY: "auto", overscrollBehavior: "contain", scrollbarWidth: "thin" } },
  historyPane: { "@container (width > 1200px)": { gridColumn: "2", gridRow: "1 / -1", width: "100%", maxWidth: "none", minHeight: 0, overflowY: "auto", overscrollBehavior: "contain", scrollbarWidth: "thin", paddingInline: "var(--spacing-4)", borderInlineStartWidth: "var(--border-width)", borderInlineStartStyle: "solid", borderInlineStartColor: "var(--noema-border-subtle)" } },
  actionBar: { gridColumn: "1", gridRow: "3", minWidth: 0, flexShrink: 0, marginInline: "var(--spacing-4)", marginBlockEnd: "var(--spacing-4)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-4)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: "var(--spacing-6)", backgroundColor: "var(--noema-surface-card)", boxShadow: "0 10px 28px color-mix(in srgb, var(--noema-text-primary) 13%, transparent)" },
  dialogActions: { display: "grid", gridAutoFlow: "column", gridAutoColumns: "minmax(0, 1fr)" },
  meta: { margin: "var(--spacing-0)", color: "var(--noema-text-secondary)", fontSize: "var(--text-supporting-size)", lineHeight: 1.4, fontVariantNumeric: "tabular-nums" },
  advanced: { borderBlockStart: "var(--border-width) solid var(--noema-border-subtle)", paddingBlockStart: "var(--spacing-3)" },
  sectionTitle: { margin: "var(--spacing-0)", color: "var(--noema-text-primary)", fontSize: "var(--text-heading-3-size)", lineHeight: "var(--text-heading-3-leading)", fontWeight: "var(--text-heading-3-weight)" },
  count: { color: "var(--noema-text-muted)", fontSize: "var(--text-supporting-size)" },
  occurrenceDay: { margin: "var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: "var(--text-supporting-size)", fontWeight: 650 },
  history: { margin: "var(--spacing-0)", padding: "var(--spacing-0)", listStyle: "none" },
  empty: { margin: "var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: "var(--text-supporting-size)" },
  state: { gridColumn: "1", gridRow: "2", flexGrow: 1, minHeight: 0, padding: "var(--spacing-4)", color: "var(--noema-text-muted)", fontSize: "var(--text-supporting-size)" },
  error: { color: "var(--destructive)", fontSize: "var(--text-supporting-size)" }
});
