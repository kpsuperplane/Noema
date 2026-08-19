import * as React from "react";
import { useMutation } from "@apollo/client/react";
import {
  TasksAnswerTaskDocument,
  TasksCancelTaskDocument,
  TasksQueueTaskDocument,
  TasksReopenTaskDocument,
  TasksRetryTaskDocument,
  TasksRunScheduledTaskNowDocument,
  TasksUpdateInboxTaskDocument
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { isStaleCommandError } from "./semanticCommand";

export type TaskCommandSubject = {
  taskId: string;
  revision: number;
  generation: number;
  title: string;
  taskDocument?: string;
  taskDocumentDigest?: string;
  project?: { projectId: string; name?: string } | null;
  executorAgentId?: string;
  executorBackend?: string;
  cwdOverride?: string | null;
  effectiveCwd?: string | null;
  effectiveCwdSource?: string;
  activeGate?: { gateId: string; kind: string } | null;
  schedule?: {
    scheduledFor: string;
    timeZone: string;
    missedRunPolicy: "RUN_ONCE" | "SKIP";
    recurrenceId?: string | null;
    recurrenceRevision?: number | null;
  } | null;
};

export type TaskCommandDraft = {
  message?: string;
  approvalDecision?: "APPROVED" | "DECLINED";
  title?: string;
  taskDocument?: string;
  projectId?: string | null;
  executorAgentId?: string;
  cwdOverride?: string | null;
};

export function useTaskCommands({
  task,
  onUpdated
}: {
  task: TaskCommandSubject;
  onUpdated?: () => void | Promise<void>;
}) {
  const [queue] = useMutation(TasksQueueTaskDocument);
  const [answer] = useMutation(TasksAnswerTaskDocument);
  const [retry] = useMutation(TasksRetryTaskDocument);
  const [cancel] = useMutation(TasksCancelTaskDocument);
  const [reopen] = useMutation(TasksReopenTaskDocument);
  const [runScheduledNow] = useMutation(TasksRunScheduledTaskNowDocument);
  const [updateInbox] = useMutation(TasksUpdateInboxTaskDocument);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [notice, setNotice] = React.useState("");
  const [requiresAcknowledgement, setRequiresAcknowledgement] = React.useState(false);

  const run = React.useCallback(
    async (action: string, draft: TaskCommandDraft = {}) => {
      if (busy || requiresAcknowledgement) return;
      setBusy(action);
      setError(null);
      const base = {
        taskId: task.taskId,
        expectedRevision: task.revision,
        expectedGeneration: task.generation,
        clientMutationId: createClientId()
      };
      try {
        switch (action) {
          case "EDIT": {
            const nextProjectId = draft.projectId ?? null;
            await updateInbox({
              variables: {
                input: {
                  ...base,
                  title: draft.title,
                  taskDocument: draft.taskDocument,
                  expectedTaskDocumentDigest: task.taskDocumentDigest,
                  projectId: nextProjectId,
                  clearProject: nextProjectId === null,
                  executorAgentId: draft.executorAgentId,
                  cwdOverride: draft.cwdOverride || null,
                  clearCwdOverride: !draft.cwdOverride
                }
              }
            });
            break;
          }
          case "QUEUE":
            await queue({ variables: { input: base } });
            break;
          case "RUN_NOW":
            await runScheduledNow({ variables: { input: base } });
            break;
          case "ANSWER":
            await answer({
              variables: {
                input: {
                  ...base,
                  gateId: requiredGate(task),
                  answerMarkdown: requiredMessage(draft.message),
                  approvalDecision: draft.approvalDecision
                }
              }
            });
            break;
          case "RETRY":
            await retry({
              variables: {
                input: { ...base, gateId: requiredGate(task), retryNote: draft.message || null }
              }
            });
            break;
          case "CANCEL":
            await cancel({ variables: { input: { ...base, reason: draft.message || null } } });
            break;
          case "REOPEN":
            await reopen({
              variables: { input: { ...base, feedbackMarkdown: requiredMessage(draft.message) } }
            });
            break;
          default:
            return;
        }
        setNotice(actionNotice(action));
        setRequiresAcknowledgement(false);
        try {
          await onUpdated?.();
        } catch {
          // The committed mutation remains authoritative; mounted queries surface refresh failures.
        }
      } catch (caught) {
        const message = caught instanceof Error ? caught.message : "Noema could not update this task.";
        if (isStaleCommandError(caught)) {
          try {
            await onUpdated?.();
          } catch {
            // Explicit acknowledgement performs a fresh subject read before retrying.
          }
          setRequiresAcknowledgement(true);
          setError("This task changed elsewhere. Review the latest version before submitting again; your text is still here.");
        } else {
          setError(message);
        }
        throw caught;
      } finally {
        setBusy(null);
      }
    },
    [answer, busy, cancel, onUpdated, queue, reopen, requiresAcknowledgement, retry, runScheduledNow, task, updateInbox]
  );

  const acknowledge = React.useCallback(() => {
    setRequiresAcknowledgement(false);
    setError(null);
  }, []);

  return {
    run,
    busy,
    error,
    notice,
    requiresAcknowledgement,
    acknowledge,
    clearError: () => setError(null)
  };
}

function requiredGate(task: TaskCommandSubject): string {
  if (!task.activeGate?.gateId) throw new Error("The active gate changed. Reload the task.");
  return task.activeGate.gateId;
}

function requiredMessage(value?: string): string {
  const message = value?.trim();
  if (!message) throw new Error("Add a message before continuing.");
  return message;
}

function actionNotice(action: string): string {
  const labels: Record<string, string> = {
    EDIT: "Task updated.",
    QUEUE: "Task queued.",
    RUN_NOW: "Task started.",
    ANSWER: "Answer sent.",
    RETRY: "Retry requested.",
    CANCEL: "Task cancelled.",
    REOPEN: "Task reopened in Queue."
  };
  return labels[action] ?? "Task updated.";
}
