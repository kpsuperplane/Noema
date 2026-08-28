import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Collapsible } from "@astryxdesign/core/Collapsible";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Code2 } from "lucide-react";
import { ChatDetailCloseButton } from "@/components/chatDetail/ChatDetailCloseButton";
import { MarkdownInlineEditor } from "@/components/MarkdownEditor";
import { AcpAgentsDocument, TasksCaptureTaskDocument } from "@/generated/graphql";
import { pwaRuntime } from "@/pwa/runtime";
import { readTaskCaptureDraft, writeTaskCaptureDraft } from "@/pwa/storage";
import { createClientId } from "@/shared/clientId";
import { initialScheduleDraft, scheduleInput, ScheduleFields } from "./ScheduleFields";
import type { TasksProject } from "./tasksTypes";

const captureFormId = "capture-task-form";

export type CaptureTaskDetailHandle = {
  focus: () => void;
};

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
  const [executorAgentId, setExecutorAgentId] = React.useState("agent:task-executor");
  const [cwdOverride, setCwdOverride] = React.useState("");
  const [schedule, setSchedule] = React.useState(initialScheduleDraft);
  const [capture, state] = useMutation(TasksCaptureTaskDocument);
  const acpAgents = useQuery(AcpAgentsDocument, { fetchPolicy: "cache-first" });
  const titleRef = React.useRef<HTMLInputElement>(null);
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

  const validSchedule = scheduling ? scheduleInput(schedule) : null;

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
        header={
          <HStack as="header" align="center" justify="between" gap={2} className={stylex.props(styles.header).className}>
            <VStack gap={0.5}>
              <h2 {...stylex.props(styles.heading)}>New task</h2>
              <span {...stylex.props(styles.subtitle)}>
                {scheduling ? "Runs automatically at the time you choose." : "Saved to Inbox until you queue it."}
              </span>
            </VStack>
            <ChatDetailCloseButton closeButtonRef={closeButtonRef} onClose={onClose} />
          </HStack>
        }
        content={
          <LayoutContent>
            <VStack
              id={captureFormId}
              as="form"
              gap={3}
              className={stylex.props(styles.form).className}
              onSubmit={(event) => {
                event.preventDefault();
                if (state.loading) return;
                void capture({
                  variables: {
                    input: {
                      workspaceId: "workspace:personal",
                      projectId: projectId || null,
                      title: title.trim(),
                      taskDocument,
                      schedule: validSchedule,
                      executorAgentId,
                      cwdOverride: cwdOverride.trim() || null,
                      clientMutationId: createClientId()
                    }
                  }
                })
                  .then(async (response) => {
                    const taskId = response.data?.captureTask.task.taskId;
                    if (!taskId) throw new Error("The created task is unavailable.");
                    await writeTaskCaptureDraft({ title: "", taskDocument: "", projectId: "" });
                    onCreated(taskId);
                  })
                  .catch(() => undefined);
              }}
            >
              <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                <span>Title</span>
                <input
                  ref={titleRef}
                  required
                  value={title}
                  {...stylex.props(styles.input)}
                  onChange={(event) => setTitle(event.currentTarget.value)}
                />
              </VStack>
              <VStack gap={1.5} className={stylex.props(styles.documentField).className}>
                <HStack align="center" justify="between" gap={2}>
                  <span {...stylex.props(styles.fieldLabel)}>Task document</span>
                  <IconButton
                    type="button"
                    size="sm"
                    variant="ghost"
                    label={sourceMode ? "Use rich editor" : "Edit source"}
                    tooltip={sourceMode ? "Use rich editor" : "Edit source"}
                    icon={<Code2 aria-hidden="true" size={14} />}
                    onClick={() => setSourceMode((current) => !current)}
                  />
                </HStack>
                <MarkdownInlineEditor key={draftRevision} value={taskDocument} onChange={setTaskDocument} label="Task document" sourceMode={sourceMode} onSourceModeChange={setSourceMode} />
              </VStack>
              <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                <span>Project (optional)</span>
                <select value={projectId} {...stylex.props(styles.input)} onChange={(event) => setProjectId(event.currentTarget.value)}>
                  <option value="">No project</option>
                  {projects.filter((project) => !project.archivedAt).map((project) => (
                    <option key={project.projectId} value={project.projectId}>{project.name}</option>
                  ))}
                </select>
              </VStack>
              <CheckboxInput label="Schedule for later" value={scheduling} onChange={setScheduling} />
              {scheduling ? <ScheduleFields value={schedule} onChange={setSchedule} /> : null}
              <Collapsible trigger="Advanced" defaultIsOpen={false}>
                <VStack gap={2} className={stylex.props(styles.advanced).className}>
                  <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                    <span>Executor</span>
                    <select value={executorAgentId} {...stylex.props(styles.input)} onChange={(event) => setExecutorAgentId(event.currentTarget.value)}>
                      <option value="agent:task-executor">Built-in executor</option>
                      {(acpAgents.data?.acpAgents ?? []).filter((agent) => agent.enabled).map((agent) => (
                        <option key={agent.agentId} value={agent.agentId}>{agent.displayName} (ACP)</option>
                      ))}
                    </select>
                  </VStack>
                  <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                    <span>Task directory base (optional)</span>
                    <input value={cwdOverride} placeholder="/absolute/path" {...stylex.props(styles.input)} onChange={(event) => setCwdOverride(event.currentTarget.value)} />
                    <span {...stylex.props(styles.hint)}>{effectiveCwdSummary(cwdOverride, projects.find((project) => project.projectId === projectId)?.folder)}</span>
                  </VStack>
                </VStack>
              </Collapsible>
              {state.error ? <p role="alert" {...stylex.props(styles.error)}>{state.error.message}</p> : null}
            </VStack>
          </LayoutContent>
        }
        footer={
          <LayoutFooter>
            <HStack gap={2} justify="end">
              <Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={state.loading} onClick={onClose} />
              <Button form={captureFormId} type="submit" size="sm" variant="primary" label={scheduling ? "Schedule task" : "Add to Inbox"} isLoading={state.loading} isDisabled={state.loading || !pwa.canMutate || !title.trim() || (scheduling && !validSchedule)} />
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
  subtitle: { color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.35 },
  form: { width: "100%", maxWidth: 760, marginInline: "auto" },
  field: { color: "var(--foreground)", fontSize: 13, fontWeight: 600 },
  documentField: { minHeight: 260, color: "var(--foreground)" },
  fieldLabel: { fontSize: 13, fontWeight: 600 },
  input: {
    width: "100%",
    minHeight: 38,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 8,
    backgroundColor: "var(--background)",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)",
    color: "var(--foreground)",
    font: "inherit",
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 }
  },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13 },
  advanced: { paddingTop: "var(--spacing-2)" },
  hint: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 400 }
});

function effectiveCwdSummary(override: string, projectFolder?: string | null): string {
  if (override.trim()) return `Task directory · under ${override.trim()}`;
  if (projectFolder) return `Task directory · under ${projectFolder}`;
  return "Task directory · Noema Tasks folder (created when queued)";
}
