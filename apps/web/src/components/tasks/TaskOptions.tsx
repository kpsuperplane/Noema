import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { Selector } from "@astryxdesign/core/Selector";
import { VStack } from "@astryxdesign/core/VStack";
import type { TasksProject } from "./tasksTypes";
import type { TaskCommandDraft, TaskCommandSubject } from "./useTaskCommands";

export function TaskOptions({ task, projects, busy, readOnly = false, requiresAcknowledgement, error, onAcknowledge, onSubmit }: {
  task: TaskCommandSubject; projects: readonly TasksProject[]; busy: boolean; readOnly?: boolean;
  requiresAcknowledgement: boolean; error: string | null;
  onAcknowledge: () => Promise<void>; onSubmit: (draft: TaskCommandDraft) => Promise<void>;
}) {
  const [pending, setPending] = React.useState<TaskCommandDraft | null>(null);
  const [saved, setSaved] = React.useState(false);
  const save = async (draft: TaskCommandDraft) => {
    setPending(draft); setSaved(false);
    try { await onSubmit(draft); setPending(null); setSaved(true); } catch { /* Keep the choice for retry. */ }
  };
  const projectId = pending && "projectId" in pending ? pending.projectId : task.project?.projectId;
  const projectOptions = projects.filter((project) => !project.archivedAt || project.projectId === task.project?.projectId).map((project) => ({ value: project.projectId, label: project.name }));
  if (task.project && !projectOptions.some((option) => option.value === task.project?.projectId)) projectOptions.push({ value: task.project.projectId, label: task.project.name ?? "Current project" });
  return <VStack gap={2}>
    <HStack gap={2} wrap="wrap" aria-label="Task options">
      <Selector label="Project" isLabelHidden size="sm" value={projectId ?? ""} isDisabled={readOnly || busy || requiresAcknowledgement} onChange={(value) => void save({ projectId: value || null })} options={[{ value: "", label: "Personal" }, ...projectOptions]} />
    </HStack>
    {error || pending && !busy ? <HStack gap={2} wrap="wrap">{error ? <span role="alert">{error}</span> : null}<Button size="sm" variant="secondary" label={requiresAcknowledgement ? "Use latest version" : "Retry"} isDisabled={busy} onClick={() => { if (requiresAcknowledgement) void onAcknowledge().catch(() => undefined); else if (pending) void save(pending); }} /></HStack> : busy && pending ? <span role="status">Saving…</span> : saved ? <span role="status">Saved</span> : null}
  </VStack>;
}
