import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { ButtonGroup } from "@astryxdesign/core/ButtonGroup";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { Popover } from "@astryxdesign/core/Popover";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Bot, CalendarClock, Check, ChevronDown, Code2, Folder, HardDrive } from "lucide-react";
import { ChatDetailCloseButton } from "@/components/chatDetail/ChatDetailCloseButton";
import { MarkdownInlineEditor } from "@/components/MarkdownEditor";
import { AcpAgentsDocument, TasksCaptureTaskDocument, TasksQueueTaskDocument } from "@/generated/graphql";
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
  const [sourceMode, setSourceMode] = React.useState(false);
  const [projectId, setProjectId] = React.useState(initialProjectId ?? "");
  const [scheduling, setScheduling] = React.useState(false);
  const [executorAgentId, setExecutorAgentId] = React.useState(defaultExecutorId);
  const [cwdOverride, setCwdOverride] = React.useState("");
  const [schedule, setSchedule] = React.useState(initialScheduleDraft);
  const [submitting, setSubmitting] = React.useState(false);
  const [submitError, setSubmitError] = React.useState<string | null>(null);
  const [capture] = useMutation(TasksCaptureTaskDocument);
  const [queue] = useMutation(TasksQueueTaskDocument);
  const acpAgents = useQuery(AcpAgentsDocument, { fetchPolicy: "cache-first" });
  const titleRef = React.useRef<HTMLInputElement>(null);
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
      () => void writeTaskCaptureDraft({ title, taskDocument, projectId }),
      250
    );
    return () => window.clearTimeout(timeout);
  }, [projectId, pwa.installed, taskDocument, title]);

  React.useEffect(() => {
    if (!pwa.installed) return;
    return pwaRuntime.registerFlusher(() =>
      writeTaskCaptureDraft({ title, taskDocument, projectId })
    );
  }, [projectId, pwa.installed, taskDocument, title]);

  const activeProjects = projects.filter((project) => !project.archivedAt);
  const selectedProject = activeProjects.find((project) => project.projectId === projectId);
  const enabledAcpAgents = (acpAgents.data?.acpAgents ?? []).filter((agent) => agent.enabled);
  const selectedExecutor = enabledAcpAgents.find((agent) => agent.agentId === executorAgentId);
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
            taskDocument,
            schedule: intent === "schedule" ? validSchedule : null,
            executorAgentId,
            cwdOverride: cwdOverride.trim() || null,
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
        if (event.key === "Escape" && !event.defaultPrevented) onClose();
      }}
    >
      <Layout
        height="fill"
        contentWidth={760}
        header={
          <HStack as="header" align="center" justify="between" gap={2} className={stylex.props(styles.header).className}>
            <h2 {...stylex.props(styles.heading)}>New task</h2>
            <ChatDetailCloseButton closeButtonRef={closeButtonRef} onClose={onClose} />
          </HStack>
        }
        content={
          <LayoutContent className={stylex.props(styles.content).className}>
            <form
              id={captureFormId}
              name="capture-task"
              {...stylex.props(styles.form)}
              onSubmit={(event) => {
                event.preventDefault();
                void submit(mainIntent);
              }}
            >
              <input
                ref={titleRef}
                aria-label="Task title"
                autoComplete="off"
                required
                value={title}
                placeholder="What needs to be done?"
                {...stylex.props(styles.titleInput)}
                onChange={(event) => setTitle(event.currentTarget.value)}
              />
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
                <MarkdownInlineEditor
                  key={draftRevision}
                  value={taskDocument}
                  onChange={setTaskDocument}
                  label="Task document"
                  placeholder="Add details or instructions…"
                  sourceMode={sourceMode}
                  onSourceModeChange={setSourceMode}
                />
              </section>
              {submitError ? <p role="alert" {...stylex.props(styles.error)}>{submitError}</p> : null}
            </form>
          </LayoutContent>
        }
        footer={
          <LayoutFooter>
            <HStack align="center" justify="between" gap={2} wrap="wrap" className={stylex.props(styles.controlRow).className}>
              <HStack align="center" gap={0.5} role="group" aria-label="Task options">
                <DropdownMenu
                  button={{
                    label: selectedProject ? `Project: ${selectedProject.name}` : "Choose project",
                    tooltip: selectedProject ? `Project: ${selectedProject.name}` : "Choose project",
                    icon: <Folder aria-hidden="true" size={16} />,
                    isIconOnly: true,
                    size: "sm",
                    variant: projectId ? "secondary" : "ghost",
                    isDisabled: submitting
                  }}
                  hasChevron={false}
                  placement="above"
                  menuWidth={220}
                  items={[
                    { label: "No project", icon: !projectId ? <Check aria-hidden="true" size={14} /> : undefined, onClick: () => setProjectId("") },
                    ...activeProjects.map((project) => ({
                      label: project.name,
                      icon: project.projectId === projectId ? <Check aria-hidden="true" size={14} /> : undefined,
                      onClick: () => setProjectId(project.projectId)
                    }))
                  ]}
                />
                <Popover
                  placement="above"
                  alignment="start"
                  label="Task schedule"
                  width="min(340px, calc(100vw - var(--spacing-4)))"
                  xstyle={styles.schedulePopover}
                  content={
                    <VStack gap={3}>
                      <h3 {...stylex.props(styles.menuHeading)}>Schedule</h3>
                      <CheckboxInput label="Schedule for later" value={scheduling} onChange={setScheduling} />
                      {scheduling ? <ScheduleFields value={schedule} onChange={setSchedule} /> : null}
                    </VStack>
                  }
                >
                  <IconButton
                    type="button"
                    size="sm"
                    variant={scheduling ? "secondary" : "ghost"}
                    label={scheduling ? "Edit schedule" : "Schedule for later"}
                    tooltip={scheduling ? "Edit schedule" : "Schedule for later"}
                    icon={<CalendarClock aria-hidden="true" size={16} />}
                    isDisabled={submitting}
                  />
                </Popover>
                <DropdownMenu
                  button={{
                    label: selectedExecutor ? `Executor: ${selectedExecutor.displayName}` : "Executor: Built-in",
                    tooltip: selectedExecutor ? `Executor: ${selectedExecutor.displayName}` : "Executor: Built-in",
                    icon: <Bot aria-hidden="true" size={16} />,
                    isIconOnly: true,
                    size: "sm",
                    variant: executorAgentId === defaultExecutorId ? "ghost" : "secondary",
                    isDisabled: submitting
                  }}
                  hasChevron={false}
                  placement="above"
                  menuWidth={220}
                  items={[
                    { label: "Built-in executor", icon: executorAgentId === defaultExecutorId ? <Check aria-hidden="true" size={14} /> : undefined, onClick: () => setExecutorAgentId(defaultExecutorId) },
                    ...enabledAcpAgents.map((agent) => ({
                      label: `${agent.displayName} (ACP)`,
                      icon: agent.agentId === executorAgentId ? <Check aria-hidden="true" size={14} /> : undefined,
                      onClick: () => setExecutorAgentId(agent.agentId)
                    }))
                  ]}
                />
                <Popover
                  placement="above"
                  alignment="start"
                  label="Task directory base"
                  width="min(320px, calc(100vw - var(--spacing-4)))"
                  content={
                    <VStack gap={3}>
                      <h3 {...stylex.props(styles.menuHeading)}>Task directory base</h3>
                      <TextInput
                        label="Absolute path"
                        description={effectiveCwdSummary(cwdOverride, selectedProject?.folder)}
                        value={cwdOverride}
                        placeholder="/absolute/path"
                        size="sm"
                        onChange={setCwdOverride}
                      />
                    </VStack>
                  }
                >
                  <IconButton
                    type="button"
                    size="sm"
                    variant={cwdOverride.trim() ? "secondary" : "ghost"}
                    label="Set Task directory base"
                    tooltip={effectiveCwdSummary(cwdOverride, selectedProject?.folder)}
                    icon={<HardDrive aria-hidden="true" size={16} />}
                    isDisabled={submitting}
                  />
                </Popover>
                <IconButton
                  type="button"
                  size="sm"
                  variant={sourceMode ? "secondary" : "ghost"}
                  label={sourceMode ? "Use rich editor" : "Edit Markdown source"}
                  tooltip={sourceMode ? "Use rich editor" : "Edit Markdown source"}
                  icon={<Code2 aria-hidden="true" size={16} />}
                  isDisabled={submitting}
                  onClick={() => setSourceMode((current) => !current)}
                />
              </HStack>
              <ButtonGroup label="Create Task">
                <Button
                  form={captureFormId}
                  type="submit"
                  size="sm"
                  variant="primary"
                  xstyle={styles.submitPrimary}
                  label={scheduling ? "Schedule" : "Run Now"}
                  isLoading={submitting}
                  isDisabled={mainDisabled}
                />
                <DropdownMenu
                  button={{
                    label: "More Task creation actions",
                    tooltip: "More Task creation actions",
                    icon: <ChevronDown aria-hidden="true" size={14} />,
                    isIconOnly: true,
                    size: "sm",
                    variant: "primary",
                    xstyle: styles.submitMenu,
                    isDisabled: baseDisabled
                  }}
                  hasChevron={false}
                  placement="above"
                  menuWidth={160}
                  items={[{ label: "Add to Inbox", onClick: () => void submit("inbox") }]}
                />
              </ButtonGroup>
            </HStack>
          </LayoutFooter>
        }
      />
    </section>
  );
});

const styles = stylex.create({
  root: { height: "100%", minHeight: 0, backgroundColor: "var(--noema-surface-card)" },
  header: { minWidth: 0, paddingBlock: "var(--spacing-3)", paddingInline: "var(--spacing-4)" },
  heading: { margin: "var(--spacing-0)", color: "var(--foreground)", fontSize: 16, fontWeight: 650, lineHeight: 1.25 },
  content: { minHeight: 0 },
  form: { display: "flex", width: "100%", minHeight: "100%", flexDirection: "column", gap: "var(--spacing-2)" },
  titleInput: {
    width: "100%",
    borderWidth: 0,
    backgroundColor: "transparent",
    padding: "var(--spacing-0)",
    color: "var(--foreground)",
    fontFamily: "var(--font-family-heading)",
    fontSize: "var(--text-heading-1-size)",
    fontWeight: "var(--text-heading-1-weight)",
    lineHeight: "var(--text-heading-1-leading)",
    outline: "none",
    "::placeholder": { color: "var(--muted-foreground)", opacity: 0.72 }
  },
  document: { flexGrow: 1, minHeight: "calc(var(--spacing-10) * 6)", color: "var(--foreground)", cursor: "text" },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13 },
  controlRow: { width: "100%" },
  submitPrimary: { borderStartStartRadius: "var(--radius-element)", borderEndStartRadius: "var(--radius-element)" },
  submitMenu: { borderStartEndRadius: "var(--radius-element)", borderEndEndRadius: "var(--radius-element)" },
  schedulePopover: { maxHeight: "calc(100vh - var(--spacing-8))", overflowY: "auto", overscrollBehavior: "contain" },
  menuHeading: { margin: "var(--spacing-0)", color: "var(--foreground)", fontSize: "var(--text-heading-4-size)", fontWeight: "var(--text-heading-4-weight)", lineHeight: "var(--text-heading-4-leading)" }
});

function effectiveCwdSummary(override: string, projectFolder?: string | null): string {
  if (override.trim()) return `Task directory · under ${override.trim()}`;
  if (projectFolder) return `Task directory · under ${projectFolder}`;
  return "Task directory · Noema Tasks folder (created when queued)";
}
