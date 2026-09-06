import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import type { TaskCommandDraft, TaskCommandSubject } from "./useTaskCommands";
import { TaskCard } from "./TasksViews";
import { taskStatusFromProjection } from "@/components/chatDetail/task/TaskStatusBadge";
import { taskActionLabel } from "./taskActionModel";

export function TaskActionDialog({ action, task, busy, acknowledging, requiresAcknowledgement, actionUnavailable, error, onAcknowledge, onClose, onSubmit }: { action: string | null; task: TaskCommandSubject; busy: boolean; acknowledging: boolean; requiresAcknowledgement: boolean; actionUnavailable: boolean; error: string | null; onAcknowledge: () => void; onClose: () => void; onSubmit: (draft: TaskCommandDraft) => Promise<void> }) {
  const formId = React.useId();
  const [message, setMessage] = React.useState("");
  const [pendingApprovalDecision, setPendingApprovalDecision] = React.useState<"APPROVED" | "DECLINED" | null>(null);
  if (!action) return null;
  const approval = action === "ANSWER" && task.activeGate?.kind === "APPROVAL";
  const requiresMessage = (action === "ANSWER" && !approval) || action === "REOPEN";
  const width = action === "QUEUE" || action === "CANCEL" ? 480 : 520;
  const submitApproval = async (approvalDecision: "APPROVED" | "DECLINED") => {
    if (busy || requiresAcknowledgement || actionUnavailable) return;
    setPendingApprovalDecision(approvalDecision);
    try {
      await onSubmit({ message, approvalDecision });
    } finally {
      setPendingApprovalDecision(null);
    }
  };
  return (
    <Dialog isOpen onOpenChange={(open) => !open && onClose()} purpose="form" width={width} variant="standard" aria-label={actionTitle(action, approval)}>
      <Layout
        height="auto"
        header={<DialogHeader title={actionTitle(action, approval)} onOpenChange={(open) => !open && onClose()} />}
        content={
          <LayoutContent>
            <VStack
              id={formId}
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                if (approval || busy || requiresAcknowledgement || actionUnavailable) return;
                void onSubmit(action === "CANCEL" ? {} : { message }).catch(() => undefined);
              }}
            >
              <TaskCard taskId={task.taskId} title={task.title} note={task.taskDocumentPreview ?? task.taskDocument?.slice(0, 240)} timestamp={task.updatedAt} project={task.project?.name} status={task.stage ? taskStatusFromProjection({ ...task, stage: task.stage }) : undefined} statusLabel={task.schedule ? "Scheduled" : task.stage?.name} listItem={false} onClick={onClose} />
              <p {...stylex.props(styles.description)}>{actionDescription(action, approval)}</p>
              {requiresAcknowledgement ? <VStack as="div" role="alert" gap={2} align="start" className={stylex.props(styles.stale).className}><span>This task changed while the dialog was open. Review the latest command target before submitting your saved draft.</span><Button type="button" size="sm" variant="secondary" label="Review latest task" isLoading={acknowledging} isDisabled={busy || acknowledging} onClick={onAcknowledge} /></VStack> : null}
              {actionUnavailable ? <p role="alert" {...stylex.props(styles.error)}>This action is no longer available for the latest task version. Your draft remains available until you close the dialog.</p> : null}
              {action === "ANSWER" || action === "RETRY" || action === "REOPEN" ? <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>{approval ? "Optional note" : messageLabel(action)}</span><textarea data-autofocus value={message} required={requiresMessage} rows={3} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => setMessage(event.currentTarget.value)} /></VStack> : null}
              {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}

            </VStack>
          </LayoutContent>
        }
        footer={<LayoutFooter><HStack gap={2} justify="end" className={stylex.props(styles.actions).className}>
                <Button type="button" size="md" variant="secondary" label={action === "CANCEL" ? "Keep task" : "Cancel"} isDisabled={busy} onClick={onClose} />
                {approval ? (
                  <>
                    <Button type="button" size="md" variant="secondary" label="Decline" isLoading={pendingApprovalDecision === "DECLINED"} isDisabled={busy || requiresAcknowledgement || actionUnavailable} onClick={() => void submitApproval("DECLINED").catch(() => undefined)} />
                    <Button type="button" size="md" variant="primary" label="Approve" isLoading={pendingApprovalDecision === "APPROVED"} isDisabled={busy || requiresAcknowledgement || actionUnavailable} onClick={() => void submitApproval("APPROVED").catch(() => undefined)} />
                  </>
                ) : (
                  <Button type="submit" form={formId} size="md" variant={action === "CANCEL" ? "destructive" : "primary"} label={action === "CANCEL" ? "Cancel task" : taskActionLabel(action, false)} isLoading={busy} isDisabled={busy || requiresAcknowledgement || actionUnavailable || (requiresMessage && !message.trim())} />
                )}
              </HStack></LayoutFooter>}
      />
    </Dialog>
  );
}

function actionTitle(action: string, approval = false): string {
  if (approval) return "Approve this task?";
  const labels: Record<string, string> = { QUEUE: "Start this task?", ANSWER: "Answer this request", RETRY: "Retry this task", CANCEL: "Cancel this task?", REOPEN: "Reopen this task?" };
  return labels[action] ?? "Update task";
}

function actionDescription(action: string, approval = false): string {
  if (approval) return "Choose a decision. You can add a note.";
  if (action === "QUEUE") return "Noema will plan this task and add it to the queue.";
  if (action === "REOPEN") return "Add what should change. Noema will start again and keep the previous history.";
  if (action === "CANCEL") return "Noema will stop work on this task. Saved history remains available. Actions already sent may still finish.";
  if (action === "ANSWER") return "Your response lets Noema continue this task.";
  if (action === "RETRY") return "Starts a new run from the current task state.";
  return "The change applies to the task shown here.";
}

function messageLabel(action: string): string {
  if (action === "ANSWER") return "Response";
  if (action === "REOPEN") return "Additional direction";
  if (action === "RETRY") return "Retry guidance (optional)";
  return "Reason (optional)";
}

const styles = stylex.create({
  field: { color: "var(--foreground)", fontSize: 13, fontWeight: 600 }, input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-2)", color: "var(--foreground)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } }, textarea: { resize: "vertical", lineHeight: 1.5 }, stale: { borderRadius: 9, backgroundColor: "var(--clay-50)", padding: "var(--spacing-2)", color: "var(--clay-700)", fontSize: 12, lineHeight: 1.45 }, error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }, description: { margin: "var(--spacing-0)", textWrap: "pretty" }, actions: { display: "grid", gridAutoFlow: "column", gridAutoColumns: "minmax(0, 1fr)" }
});
