import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Collapsible } from "@astryxdesign/core/Collapsible";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { AcpAgentsDocument, TasksCaptureTaskDocument } from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { TasksProject } from "./tasksTypes";
import { pwaRuntime } from "@/pwa/runtime";
import { readTaskCaptureDraft, writeTaskCaptureDraft } from "@/pwa/storage";
import { initialScheduleDraft, scheduleInput, ScheduleFields } from "./ScheduleFields";
import { TaskMarkdownEditor } from "./TaskMarkdownEditor";

export function CaptureTaskDialog({ open, projects, initialProjectId, onOpenChange }: { open: boolean; projects: readonly TasksProject[]; initialProjectId?: string; onOpenChange: (open: boolean) => void }) {
  const [title, setTitle] = React.useState("");
  const [taskDocument, setTaskDocument] = React.useState("");
  const [projectId, setProjectId] = React.useState(initialProjectId ?? "");
  const [scheduling, setScheduling] = React.useState(false);
  const [executorAgentId, setExecutorAgentId] = React.useState("agent:task-executor");
  const [cwdOverride, setCwdOverride] = React.useState("");
  const [schedule, setSchedule] = React.useState(initialScheduleDraft);
  const [capture, state] = useMutation(TasksCaptureTaskDocument);
  const acpAgents = useQuery(AcpAgentsDocument, { fetchPolicy: "cache-first" });
  const pwa = React.useSyncExternalStore(
    pwaRuntime.subscribe,
    pwaRuntime.getSnapshot,
    pwaRuntime.getSnapshot
  );
  const restoredRef = React.useRef(!pwa.installed);

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

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" variant="fullscreen" aria-label="New task">
      <Layout
        height="fill"
        header={
          <DialogHeader
            title="New task"
            subtitle={scheduling ? "Runs automatically at the time you choose." : "Saved to Inbox until you queue it."}
            onOpenChange={onOpenChange}
          />
        }
        content={
          <LayoutContent>
            <VStack
              id="capture-task-form"
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                void capture({
                  variables: {
                    input: {
                      workspaceId: "workspace:personal",
                      projectId: projectId || null,
                      title: title.trim(),
                      taskDocument,
                      schedule: scheduling ? scheduleInput(schedule) : null,
                      executorAgentId,
                      cwdOverride: cwdOverride.trim() || null,
                      clientMutationId: createClientId()
                    }
                  }
                })
                  .then(async () => {
                    setTitle("");
                    setTaskDocument("");
                    setProjectId("");
                    setScheduling(false);
                    setExecutorAgentId("agent:task-executor");
                    setCwdOverride("");
                    setSchedule(initialScheduleDraft());
                    await writeTaskCaptureDraft({ title: "", taskDocument: "", projectId: "" });
                    onOpenChange(false);
                  })
                  .catch(() => undefined);
              }}
            >
              <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                <span>Title</span>
                <input
                  data-autofocus
                  required
                  value={title}
                  {...stylex.props(styles.input)}
                  onChange={(event) => setTitle(event.currentTarget.value)}
                />
              </VStack>
              <VStack gap={1.5} className={stylex.props(styles.field).className}>
                <span>Task document</span>
                <TaskMarkdownEditor value={taskDocument} onChange={setTaskDocument} />
              </VStack>
              <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                <span>Project (optional)</span>
                <select
                  value={projectId}
                  {...stylex.props(styles.input)}
                  onChange={(event) => setProjectId(event.currentTarget.value)}
                >
                  <option value="">No project</option>
                  {projects.filter((project) => !project.archivedAt).map((project) => (
                    <option key={project.projectId} value={project.projectId}>{project.name}</option>
                  ))}
                </select>
              </VStack>
              <CheckboxInput
                label="Schedule for later"
                value={scheduling}
                onChange={setScheduling}
              />
              {scheduling ? <ScheduleFields value={schedule} onChange={setSchedule} /> : null}
              <Collapsible trigger="Advanced" defaultIsOpen={false}>
                <VStack gap={2} className={stylex.props(styles.advanced).className}>
                  <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                    <span>Executor</span>
                    <select value={executorAgentId} {...stylex.props(styles.input)} onChange={(event) => setExecutorAgentId(event.currentTarget.value)}>
                      <option value="agent:task-executor">Built-in executor</option>
                      {(acpAgents.data?.acpAgents ?? []).filter((agent) => agent.enabled).map((agent) => <option key={agent.agentId} value={agent.agentId}>{agent.displayName} (ACP)</option>)}
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
        footer={<LayoutFooter><HStack gap={2} justify="end"><Button type="button" size="sm" variant="ghost" label="Cancel" onClick={() => onOpenChange(false)} /><Button form="capture-task-form" type="submit" size="sm" variant="primary" label={scheduling ? "Schedule task" : "Add to Inbox"} isLoading={state.loading} isDisabled={state.loading || !pwa.canMutate || !title.trim() || (scheduling && !scheduleInput(schedule))} /></HStack></LayoutFooter>}
      />
    </Dialog>
  );
}

const styles = stylex.create({
  field: { fontSize: 13, fontWeight: 600 },
  input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-2)", color: "var(--foreground)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13 },
  advanced: { paddingTop: "var(--spacing-2)" },
  hint: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 400 }
});

function effectiveCwdSummary(override: string, projectFolder?: string | null): string {
  if (override.trim()) return `Task directory · under ${override.trim()}`;
  if (projectFolder) return `Task directory · under ${projectFolder}`;
  return "Task directory · Noema Tasks folder (created when queued)";
}
