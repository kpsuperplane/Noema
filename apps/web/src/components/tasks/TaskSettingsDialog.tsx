import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Collapsible } from "@astryxdesign/core/Collapsible";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { AcpAgentsDocument } from "@/generated/graphql";
import type { TasksProject } from "./tasksTypes";
import type { TaskCommandDraft, TaskCommandSubject } from "./useTaskCommands";

export function TaskSettingsDialog({ task, projects, busy, acknowledging, requiresAcknowledgement, actionUnavailable, error, onAcknowledge, onCancel, onSubmit }: { task: TaskCommandSubject; projects: readonly TasksProject[]; busy: boolean; acknowledging: boolean; requiresAcknowledgement: boolean; actionUnavailable: boolean; error: string | null; onAcknowledge: () => void; onCancel: () => void; onSubmit: (draft: TaskCommandDraft) => Promise<void> }) {
  const [projectId, setProjectId] = React.useState(task.project?.projectId ?? "");
  const [executorAgentId, setExecutorAgentId] = React.useState(task.executorAgentId ?? "agent:task-executor");
  const [cwdOverride, setCwdOverride] = React.useState(task.cwdOverride ?? "");
  const acpAgents = useQuery(AcpAgentsDocument, { fetchPolicy: "cache-first" });
  return (
    <Dialog isOpen onOpenChange={(open) => { if (!open) onCancel(); }} purpose="form" width={520} aria-label="Edit task settings">
      <Layout
        height="auto"
        header={<DialogHeader title="Task settings" onOpenChange={(open) => { if (!open) onCancel(); }} />}
        content={<LayoutContent>
          <VStack as="form" gap={3} onSubmit={(event) => { event.preventDefault(); void onSubmit({ projectId: projectId || null, executorAgentId, cwdOverride: cwdOverride.trim() || null }).catch(() => undefined); }}>
            <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
              <span>Project (optional)</span>
              <select data-autofocus value={projectId} {...stylex.props(styles.input)} onChange={(event) => setProjectId(event.currentTarget.value)}>
                <option value="">No project</option>
                {task.project && !projects.some((project) => project.projectId === task.project?.projectId) ? <option value={task.project.projectId}>{task.project.name ?? "Current project"}</option> : null}
                {projects.filter((project) => !project.archivedAt || project.projectId === task.project?.projectId).map((project) => <option key={project.projectId} value={project.projectId}>{project.name}</option>)}
              </select>
            </VStack>
            <Collapsible trigger="Advanced" defaultIsOpen={Boolean(task.cwdOverride || task.executorAgentId && task.executorAgentId !== "agent:task-executor")}>
              <VStack gap={2} className={stylex.props(styles.advanced).className}>
                <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                  <span>Executor</span>
                  <select value={executorAgentId} {...stylex.props(styles.input)} onChange={(event) => setExecutorAgentId(event.currentTarget.value)}>
                    <option value="agent:task-executor">Built-in executor</option>
                    {(acpAgents.data?.acpAgents ?? []).filter((agent) => agent.enabled || agent.agentId === executorAgentId).map((agent) => <option key={agent.agentId} value={agent.agentId}>{agent.displayName} (ACP)</option>)}
                  </select>
                </VStack>
                <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                  <span>Task directory base (optional)</span>
                  <input value={cwdOverride} placeholder="/absolute/path" {...stylex.props(styles.input)} onChange={(event) => setCwdOverride(event.currentTarget.value)} />
                  <span {...stylex.props(styles.hint)}>{taskCwdSummary(cwdOverride, projects.find((project) => project.projectId === projectId)?.folder, task.effectiveCwd)}</span>
                </VStack>
              </VStack>
            </Collapsible>
            {requiresAcknowledgement ? <VStack as="div" role="alert" gap={2} align="start" className={stylex.props(styles.stale).className}><span>Your draft is still here. Load the latest Task version before you save it again.</span><Button type="button" size="sm" variant="secondary" label="Use latest version" isLoading={acknowledging} isDisabled={busy || acknowledging} onClick={onAcknowledge} /></VStack> : null}
            {actionUnavailable ? <p role="alert" {...stylex.props(styles.error)}>This Task is no longer editable.</p> : null}
            {!requiresAcknowledgement && error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
            <HStack gap={2} justify="end">
              <Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={busy} onClick={onCancel} />
              <Button type="submit" size="sm" variant="primary" label="Save settings" isLoading={busy} isDisabled={busy || requiresAcknowledgement || actionUnavailable} />
            </HStack>
          </VStack>
        </LayoutContent>}
      />
    </Dialog>
  );
}

function taskCwdSummary(override: string, projectFolder?: string | null, frozen?: string | null): string {
  if (override.trim()) return `Task directory · under ${override.trim()}`;
  if (projectFolder) return `Task directory · under ${projectFolder}`;
  if (frozen) return `Task directory · ${frozen}`;
  return "Task directory · Noema Tasks folder (created when queued)";
}

const styles = stylex.create({
  field: { color: "var(--foreground)", fontSize: 13, fontWeight: 600 },
  input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: "var(--radius-element)", backgroundColor: "var(--background)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-2)", color: "var(--foreground)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  advanced: { paddingTop: "var(--spacing-2)" },
  hint: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 400 },
  stale: { borderRadius: "var(--radius-element)", backgroundColor: "var(--noema-surface-card)", padding: "var(--spacing-2)", color: "var(--noema-clay-600)", fontSize: 12, lineHeight: 1.4 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});
