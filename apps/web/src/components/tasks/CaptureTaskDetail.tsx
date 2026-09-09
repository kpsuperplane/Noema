import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Grid } from "@astryxdesign/core/Grid";
import { TaskDocumentEditor, TaskTitleField } from "./TaskDocumentFields";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { Popover } from "@astryxdesign/core/Popover";
import { VStack } from "@astryxdesign/core/VStack";
import { useBlocker } from "@tanstack/react-router";
import * as stylex from "@stylexjs/stylex";
import { CalendarClock, Check, Folder } from "lucide-react";
import { ChatDetailCloseButton } from "@/components/chatDetail/ChatDetailCloseButton";
import { TaskDocumentLayout, TaskTitleHeader } from "./TaskDocumentLayout";
import { TaskScheduleSummary } from "./TaskScheduleSummary";
import { taskScheduleTimestampLabel } from "./tasksModel";
import { TasksCaptureTaskDocument, TasksQueueTaskDocument } from "@/generated/graphql";
import { pwaRuntime } from "@/pwa/runtime";
import { readTaskCaptureDraft, writeTaskCaptureDraft } from "@/pwa/storage";
import { createClientId } from "@/shared/clientId";
import { initialScheduleDraft, scheduleInput, ScheduleFields } from "./ScheduleFields";
import type { TasksProject } from "./tasksTypes";

const captureFormId = "capture-task-form";
const defaultExecutorId = "agent:task-executor";

export type CaptureTaskDetailHandle = {
  focus: () => void;
};

type SubmitIntent = "run" | "schedule" | "inbox";

export const CaptureTaskDetail = React.forwardRef<CaptureTaskDetailHandle, {
  projects: readonly TasksProject[];
  initialProjectId?: string;
  onClose: () => void;
  onCreated: (taskId: string) => void;
}>(function CaptureTaskDetail({ projects, initialProjectId, onClose, onCreated }, ref) {
  const [title, setTitle] = React.useState("");
  const [taskDocument, setTaskDocument] = React.useState("");
  const [draftRevision, setDraftRevision] = React.useState(0);
  const [projectId, setProjectId] = React.useState(initialProjectId ?? "");
  const [scheduling, setScheduling] = React.useState(false);
  const [scheduleOpen, setScheduleOpen] = React.useState(false);
  const [schedule, setSchedule] = React.useState(initialScheduleDraft);
  const [submitting, setSubmitting] = React.useState(false);
  const [submitError, setSubmitError] = React.useState<string | null>(null);
  const [capture] = useMutation(TasksCaptureTaskDocument);
  const [queue] = useMutation(TasksQueueTaskDocument);
  const titleRef = React.useRef<HTMLTextAreaElement>(null);
  const readValueRef = React.useRef<(() => string) | null>(null);
  const documentRef = React.useRef<HTMLElement>(null);
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const pwa = React.useSyncExternalStore(
    pwaRuntime.subscribe,
    pwaRuntime.getSnapshot,
    pwaRuntime.getSnapshot
  );
  const restoredRef = React.useRef(!pwa.installed);

  React.useImperativeHandle(ref, () => ({
    focus: () => titleRef.current?.focus({ preventScroll: true })
  }), []);

  React.useEffect(() => {
    titleRef.current?.focus({ preventScroll: true });
  }, []);

  React.useEffect(() => {
    if (!pwa.installed) return;
    let active = true;
    void readTaskCaptureDraft().then((draft) => {
      if (!active) return;
      if (draft) {
        setTitle(draft.title);
        setTaskDocument(draft.taskDocument);
        setProjectId(draft.projectId);
      }
      restoredRef.current = true;
      setDraftRevision((current) => current + 1);
    });
    return () => {
      active = false;
    };
  }, [pwa.installed]);

  React.useEffect(() => {
    if (!pwa.installed || !restoredRef.current) return;
    const timeout = window.setTimeout(
      () => void writeTaskCaptureDraft({ title, taskDocument: readValueRef.current?.() ?? taskDocument, projectId }),
      250
    );
    return () => window.clearTimeout(timeout);
  }, [projectId, pwa.installed, taskDocument, title]);

  React.useEffect(() => {
    if (!pwa.installed) return;
    return pwaRuntime.registerFlusher(() =>
      writeTaskCaptureDraft({ title, taskDocument: readValueRef.current?.() ?? taskDocument, projectId })
    );
  }, [projectId, pwa.installed, taskDocument, title]);

  const flushDraft = React.useCallback(async () => {
    if (pwa.installed) await writeTaskCaptureDraft({ title, taskDocument: readValueRef.current?.() ?? taskDocument, projectId });
  }, [projectId, pwa.installed, taskDocument, title]);
  const close = () => { void flushDraft().then(onClose); };
  useBlocker({ shouldBlockFn: async () => { await flushDraft(); return false; }, enableBeforeUnload: false });

  const activeProjects = projects.filter((project) => !project.archivedAt);
  const selectedProject = activeProjects.find((project) => project.projectId === projectId);
  const validSchedule = scheduling ? scheduleInput(schedule) : null;
  const baseDisabled = submitting || !pwa.canMutate || !title.trim();
  const mainDisabled = baseDisabled || (scheduling && !validSchedule);

  async function submit(intent: SubmitIntent) {
    if (baseDisabled || (intent === "schedule" && !validSchedule)) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      const response = await capture({
        variables: {
          input: {
            workspaceId: "workspace:personal",
            projectId: projectId || null,
            title: title.trim(),
            taskDocument: readValueRef.current?.() ?? taskDocument,
            schedule: intent === "schedule" ? validSchedule : null,
            executorAgentId: defaultExecutorId,
            clientMutationId: createClientId()
          }
        }
      });
      const task = response.data?.captureTask.task;
      if (!task) throw new Error("The created Task is unavailable.");
      if (intent === "run") {
        try {
          await queue({ variables: { input: {
            taskId: task.taskId,
            expectedRevision: task.revision,
            expectedGeneration: task.generation,
            clientMutationId: createClientId()
          } } });
        } catch {
          // Capture is durable. Its detail shows the current Inbox state when queueing fails.
        }
      }
      await writeTaskCaptureDraft({ title: "", taskDocument: "", projectId: "" });
      onCreated(task.taskId);
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : "Noema could not create this Task.");
    } finally {
      setSubmitting(false);
    }
  }

  const mainIntent: SubmitIntent = scheduling ? "schedule" : "run";

  return (
    <section
      aria-label="New task"
      data-slot="task-capture-detail"
      {...stylex.props(styles.root)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.defaultPrevented) close();
      }}
    >
      <Grid height="100%" xstyle={styles.frame}>
      <Layout
        height="fill"
        header={
          <VStack gap={0} xstyle={styles.header}>
            <TaskTitleHeader close={<ChatDetailCloseButton closeButtonRef={closeButtonRef} onClose={close} />}>
              <TaskTitleField
                ref={titleRef}
                form={captureFormId}
                aria-label="Task title"
                autoComplete="off"
                required
                value={title}
                placeholder="What needs to be done?"
                onChange={(event) => setTitle(event.currentTarget.value)}
              />
            </TaskTitleHeader>
          </VStack>
        }
        content={
          <LayoutContent padding={0} className={stylex.props(styles.content).className}>
            <form
              id={captureFormId}
              name="capture-task"
              {...stylex.props(styles.form)}
              onSubmit={(event) => {
                event.preventDefault();
                void submit(mainIntent);
              }}
            >
              <TaskDocumentLayout>
                <HStack>
                  <DropdownMenu button={{ label: selectedProject?.name ?? "Personal", icon: <Folder aria-hidden="true" size={16} />, size: "sm", variant: "secondary", isDisabled: submitting }} placement="below" items={[
                    { label: "Personal", icon: !projectId ? <Check aria-hidden="true" size={14} /> : undefined, onClick: () => setProjectId("") },
                    ...activeProjects.map((project) => ({ label: project.name, icon: project.projectId === projectId ? <Check aria-hidden="true" size={14} /> : undefined, onClick: () => setProjectId(project.projectId) }))
                  ]} />
                </HStack>
                <TaskScheduleSummary value={scheduling ? validSchedule ? taskScheduleTimestampLabel(validSchedule.scheduledFor, schedule.timeZone) : "Choose a start time" : undefined}>
                  <Popover isOpen={scheduleOpen} onOpenChange={(open) => { setScheduleOpen(open); if (open) setScheduling(true); }} placement="below" alignment="start" label="Task schedule" width="min(340px, calc(100vw - var(--spacing-4)))" xstyle={styles.schedulePopover} content={<VStack gap={3} onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); setScheduleOpen(false); } }}><ScheduleFields value={schedule} onChange={setSchedule} /><Button size="sm" variant="ghost" label="Remove schedule" onClick={() => { setScheduling(false); setScheduleOpen(false); }} /></VStack>}>
                    <Button size="sm" variant={scheduling ? "secondary" : "ghost"} label={scheduling ? "Reschedule" : "Schedule"} icon={<CalendarClock aria-hidden="true" size={16} />} isDisabled={submitting} />
                  </Popover>
                </TaskScheduleSummary>
              <section
                ref={documentRef}
                aria-label="Task document"
                data-slot="task-capture-document"
                {...stylex.props(styles.document)}
                onClick={(event) => {
                  const target = event.target as HTMLElement;
                  if (target.closest("button, input, textarea, a, [contenteditable='true']")) return;
                  documentRef.current?.querySelector<HTMLElement>("[contenteditable='true'], textarea")?.focus();
                }}
              >
                <TaskDocumentEditor
                  readValueRef={readValueRef}
                  key={draftRevision}
                  value={taskDocument}
                  onChange={setTaskDocument}
                  label="Task document"
                  placeholder="Add details or instructions…"
                />
              </section>
                {submitError ? <p role="alert" {...stylex.props(styles.error)}>{submitError}</p> : null}
              </TaskDocumentLayout>
            </form>
          </LayoutContent>
        }
        footer={
          <LayoutFooter padding={0} className={stylex.props(styles.footer).className}>
            <HStack justify="end" gap={2} wrap="wrap">
              <Button type="button" size="md" variant="secondary" label="Add to Inbox" isDisabled={baseDisabled} onClick={() => void submit("inbox")} />
              <Button form={captureFormId} type="submit" size="md" variant="primary" label={scheduling ? "Schedule task" : "Run now"} isLoading={submitting} isDisabled={mainDisabled} />
            </HStack>
          </LayoutFooter>
        }
      />
      <VStack aria-hidden="true" xstyle={styles.emptyPanel} />
      </Grid>
    </section>
  );
});

const styles = stylex.create({
  root: { containerType: "inline-size", width: "100%", height: "100%", minHeight: 0, backgroundColor: "var(--noema-surface-card)" },
  frame: { minWidth: 0, minHeight: 0, gridTemplateColumns: "minmax(0, 1fr)", gridTemplateRows: "minmax(0, 1fr)", "@container (width > 1200px)": { gridTemplateColumns: "minmax(0, 1fr) 600px" } },
  emptyPanel: { display: { default: "none", "@container (width > 1200px)": "flex" }, borderInlineStartWidth: "var(--border-width)", borderInlineStartStyle: "solid", borderInlineStartColor: "var(--noema-border-subtle)" },
  header: { paddingBlockEnd: "var(--spacing-3)", borderBottomWidth: "var(--border-width)", borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)" },
  content: { minHeight: 0, padding: "var(--spacing-0)" },
  footer: { paddingBlock: "var(--spacing-4)", paddingInline: "max(var(--spacing-6), calc((100% - 760px) / 2))" },
  form: { display: "flex", width: "100%", minHeight: "100%", flexDirection: "column", gap: "var(--spacing-2)" },
  document: { display: "flex", flexDirection: "column", flexGrow: 1, minHeight: "calc(var(--spacing-10) * 6)", color: "var(--foreground)", cursor: "text" },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13 },

  schedulePopover: { maxHeight: "calc(100vh - var(--spacing-8))", overflowY: "auto", overscrollBehavior: "contain" },
  });
