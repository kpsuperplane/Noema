import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import type { WorkProject } from "./workTypes";
import type { TaskCommandDraft, TaskCommandSubject } from "./useTaskCommands";
import { taskActionLabel } from "./taskActionModel";

export function TaskActionDialog({ action, task, projects, busy, acknowledging, requiresAcknowledgement, actionUnavailable, error, onAcknowledge, onClose, onSubmit }: { action: string | null; task: TaskCommandSubject; projects: readonly WorkProject[]; busy: boolean; acknowledging: boolean; requiresAcknowledgement: boolean; actionUnavailable: boolean; error: string | null; onAcknowledge: () => void; onClose: () => void; onSubmit: (draft: TaskCommandDraft) => Promise<void> }) {
  const [message, setMessage] = React.useState("");
  const [title, setTitle] = React.useState(task.title);
  const [description, setDescription] = React.useState(task.description ?? "");
  const [projectId, setProjectId] = React.useState(task.project?.projectId ?? "");
  const [approvalDecision, setApprovalDecision] = React.useState<"APPROVED" | "DECLINED">("APPROVED");
  if (!action) return null;
  const requiresMessage = action === "ANSWER" || action === "REOPEN";
  const approval = action === "ANSWER" && task.activeGate?.kind === "APPROVAL";
  const width = action === "QUEUE" || action === "CANCEL" ? 480 : 520;
  return (
    <Dialog isOpen onOpenChange={(open) => !open && onClose()} purpose="form" width={width} aria-label={actionTitle(action)}>
      <Layout
        height="auto"
        header={<DialogHeader title={actionTitle(action)} subtitle={actionDescription(action)} onOpenChange={(open) => !open && onClose()} />}
        content={
          <LayoutContent>
            <VStack
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                void onSubmit({ message, title, description, projectId: projectId || null, approvalDecision: approval ? approvalDecision : undefined }).catch(() => undefined);
              }}
            >
              {requiresAcknowledgement ? <VStack as="div" role="alert" gap={2} align="start" className={stylex.props(styles.stale).className}><span>This task changed while the dialog was open. Review the latest command target before submitting your saved draft.</span><Button type="button" size="sm" variant="secondary" label="Review latest task" isLoading={acknowledging} isDisabled={busy || acknowledging} onClick={onAcknowledge} /></VStack> : null}
              {actionUnavailable ? <p role="alert" {...stylex.props(styles.error)}>This action is no longer available for the latest task version. Your draft remains available until you close the dialog.</p> : null}
              {action === "EDIT" ? <><VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Title</span><input data-autofocus value={title} required {...stylex.props(styles.input)} onChange={(event) => setTitle(event.currentTarget.value)} /></VStack><VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Description (optional)</span><textarea value={description} rows={3} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => setDescription(event.currentTarget.value)} /></VStack><VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Project (optional)</span><select value={projectId} {...stylex.props(styles.input)} onChange={(event) => setProjectId(event.currentTarget.value)}><option value="">No project</option>{task.project && !projects.some((project) => project.projectId === task.project?.projectId) ? <option value={task.project.projectId}>{task.project.name ?? "Current project"}</option> : null}{projects.filter((project) => !project.archivedAt || project.projectId === task.project?.projectId).map((project) => <option key={project.projectId} value={project.projectId}>{project.name}</option>)}</select></VStack></> : null}
              {approval ? <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Decision</span><select value={approvalDecision} {...stylex.props(styles.input)} onChange={(event) => setApprovalDecision(event.currentTarget.value as "APPROVED" | "DECLINED")}><option value="APPROVED">Approve</option><option value="DECLINED">Decline</option></select></VStack> : null}
              {action === "ANSWER" || action === "RETRY" || action === "REOPEN" || action === "CANCEL" ? <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>{messageLabel(action)}</span><textarea data-autofocus value={message} required={requiresMessage} rows={3} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => setMessage(event.currentTarget.value)} /></VStack> : null}
              {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
              <HStack gap={2} justify="end" className={stylex.props(styles.actions).className}><Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={busy} onClick={onClose} /><Button type="submit" size="sm" variant={action === "CANCEL" ? "destructive" : "primary"} label={taskActionLabel(action, false, approval ? approvalDecision : undefined)} isLoading={busy} isDisabled={busy || requiresAcknowledgement || actionUnavailable || (requiresMessage && !message.trim()) || (action === "EDIT" && !title.trim())} /></HStack>
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

function actionTitle(action: string): string {
  const labels: Record<string, string> = { EDIT: "Edit Inbox task", QUEUE: "Queue this task?", ANSWER: "Answer this request", RETRY: "Retry this task", CANCEL: "Cancel this task?", REOPEN: "Reopen this task?" };
  return labels[action] ?? "Update task";
}

function actionDescription(action: string): string {
  if (action === "QUEUE") return "This confirms the current Inbox version and authorizes planning.";
  if (action === "REOPEN") return "Add what should change. Historic runs and evidence stay intact; the new cycle starts in Queue.";
  if (action === "CANCEL") return "Active work is fenced immediately. Historic evidence remains available.";
  if (action === "EDIT") return "Changes the Inbox copy before the task is queued.";
  if (action === "ANSWER") return "Your response resumes the task from its current human gate.";
  if (action === "RETRY") return "Starts a new run from the current task state.";
  return "Applies this command to the task version shown when the dialog opened.";
}

function messageLabel(action: string): string {
  if (action === "ANSWER") return "Response";
  if (action === "REOPEN") return "Additional direction";
  if (action === "RETRY") return "Retry guidance (optional)";
  return "Reason (optional)";
}

const styles = stylex.create({
  field: { color: "var(--foreground)", fontSize: 13, fontWeight: 600 }, input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-2)", color: "var(--foreground)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } }, textarea: { resize: "vertical", lineHeight: 1.5 }, stale: { borderRadius: 9, backgroundColor: "var(--clay-50)", padding: "var(--spacing-2)", color: "var(--clay-700)", fontSize: 12, lineHeight: 1.45 }, error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }, actions: { paddingTop: "var(--spacing-1)" }
});
