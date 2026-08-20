import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import type { TaskCommandDraft, TaskCommandSubject } from "./useTaskCommands";
import { taskActionLabel } from "./taskActionModel";

export function TaskActionDialog({ action, task, busy, acknowledging, requiresAcknowledgement, actionUnavailable, error, onAcknowledge, onClose, onSubmit }: { action: string | null; task: TaskCommandSubject; busy: boolean; acknowledging: boolean; requiresAcknowledgement: boolean; actionUnavailable: boolean; error: string | null; onAcknowledge: () => void; onClose: () => void; onSubmit: (draft: TaskCommandDraft) => Promise<void> }) {
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
        header={<DialogHeader title={actionTitle(action, approval)} subtitle={actionDescription(action, approval)} onOpenChange={(open) => !open && onClose()} />}
        content={
          <LayoutContent>
            <VStack
              id="task-action-form"
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                if (approval) return;
                void onSubmit({ message }).catch(() => undefined);
              }}
            >
              {requiresAcknowledgement ? <VStack as="div" role="alert" gap={2} align="start" className={stylex.props(styles.stale).className}><span>This task changed while the dialog was open. Review the latest command target before submitting your saved draft.</span><Button type="button" size="sm" variant="secondary" label="Review latest task" isLoading={acknowledging} isDisabled={busy || acknowledging} onClick={onAcknowledge} /></VStack> : null}
              {actionUnavailable ? <p role="alert" {...stylex.props(styles.error)}>This action is no longer available for the latest task version. Your draft remains available until you close the dialog.</p> : null}
              {action === "ANSWER" || action === "RETRY" || action === "REOPEN" || action === "CANCEL" ? <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>{approval ? "Optional note" : messageLabel(action)}</span><textarea data-autofocus value={message} required={requiresMessage} rows={3} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => setMessage(event.currentTarget.value)} /></VStack> : null}
              {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
              <HStack gap={2} justify="end" className={stylex.props(styles.actions).className}>
                <Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={busy} onClick={onClose} />
                {approval ? (
                  <>
                    <Button type="button" size="sm" variant="secondary" label="Decline" isLoading={pendingApprovalDecision === "DECLINED"} isDisabled={busy || requiresAcknowledgement || actionUnavailable} onClick={() => void submitApproval("DECLINED").catch(() => undefined)} />
                    <Button type="button" size="sm" variant="primary" label="Approve" isLoading={pendingApprovalDecision === "APPROVED"} isDisabled={busy || requiresAcknowledgement || actionUnavailable} onClick={() => void submitApproval("APPROVED").catch(() => undefined)} />
                  </>
                ) : (
                  <Button type="submit" size="sm" variant={action === "CANCEL" ? "destructive" : "primary"} label={taskActionLabel(action, false)} isLoading={busy} isDisabled={busy || requiresAcknowledgement || actionUnavailable || (requiresMessage && !message.trim())} />
                )}
              </HStack>
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

function actionTitle(action: string, approval = false): string {
  if (approval) return "Approve this task?";
  const labels: Record<string, string> = { QUEUE: "Queue this task?", ANSWER: "Answer this request", RETRY: "Retry this task", CANCEL: "Cancel this task?", REOPEN: "Reopen this task?" };
  return labels[action] ?? "Update task";
}

function actionDescription(action: string, approval = false): string {
  if (approval) return "Choose a decision. You can add a note.";
  if (action === "QUEUE") return "This confirms the current Inbox version and authorizes planning.";
  if (action === "REOPEN") return "Add what should change. Historic runs and evidence stay intact; the new cycle starts in Queue.";
  if (action === "CANCEL") return "The active task stops immediately. Saved history remains available.";
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
