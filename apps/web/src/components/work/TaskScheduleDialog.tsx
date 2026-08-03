import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import {
  WorkRescheduleTaskDocument,
  WorkScheduleTaskDocument,
  WorkTaskRecurrenceDocument,
  WorkUnscheduleTaskDocument
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { initialScheduleDraft, scheduleInput, ScheduleFields, type ScheduleDraft } from "./ScheduleFields";
import type { TaskCommandSubject } from "./useTaskCommands";

export function TaskScheduleDialog({ action, task, onClose, onUpdated }: {
  action: "SCHEDULE" | "RESCHEDULE" | "UNSCHEDULE" | null;
  task: TaskCommandSubject;
  onClose: () => void;
  onUpdated?: () => void | Promise<void>;
}) {
  const open = action !== null;
  const [draft, setDraft] = React.useState<ScheduleDraft>(() => initialScheduleDraft(task.schedule ?? undefined));
  const [schedule, scheduleState] = useMutation(WorkScheduleTaskDocument);
  const [reschedule, rescheduleState] = useMutation(WorkRescheduleTaskDocument);
  const [unschedule, unscheduleState] = useMutation(WorkUnscheduleTaskDocument);
  const recurrence = useQuery(WorkTaskRecurrenceDocument, {
    variables: { recurrenceId: task.schedule?.recurrenceId ?? "" },
    skip: !task.schedule?.recurrenceId
  });
  const currentRecurrence = recurrence.data?.taskRecurrence;
  const effectiveDraft = currentRecurrence && draft.repeat === "custom" && !draft.cron
    ? { ...draft, cron: currentRecurrence.cronExpression, overlapPolicy: currentRecurrence.overlapPolicy }
    : draft;
  React.useEffect(() => {
    if (!open || action === "UNSCHEDULE") return;
    requestAnimationFrame(() => document.querySelector<HTMLElement>('[aria-label="Schedule task"] input')?.focus());
  }, [action, open]);
  const busy = scheduleState.loading || rescheduleState.loading || unscheduleState.loading;
  const error = scheduleState.error ?? rescheduleState.error ?? unscheduleState.error;

  return (
    <Dialog isOpen={open} onOpenChange={(next) => { if (!next) onClose(); }} purpose="form" width={500} aria-label="Schedule task">
      <Layout
        height="auto"
        header={<DialogHeader title={action === "UNSCHEDULE" ? "Unschedule task" : action === "RESCHEDULE" ? "Reschedule task" : "Schedule task"} subtitle={action === "UNSCHEDULE" ? "Return this task to Inbox." : "The task will enter Queue when it is due."} onOpenChange={(next) => { if (!next) onClose(); }} />}
        content={<LayoutContent>
          <VStack as="form" gap={3} onSubmit={(event) => {
            event.preventDefault();
            const base = { taskId: task.taskId, expectedRevision: task.revision, expectedGeneration: task.generation, clientMutationId: createClientId() };
            const nextSchedule = scheduleInput(effectiveDraft);
            const mutation = action === "UNSCHEDULE"
              ? unschedule({ variables: { input: base } })
              : nextSchedule
                ? (action === "RESCHEDULE" ? reschedule : schedule)({ variables: { input: { ...base, schedule: nextSchedule } } })
                : Promise.reject(new Error("Invalid schedule"));
            void mutation.then(async () => { await onUpdated?.(); onClose(); }).catch(() => undefined);
          }}>
            {action === "UNSCHEDULE" ? <p>This removes only future timing. The task content and history stay intact.</p> : <ScheduleFields value={effectiveDraft} onChange={setDraft} />}
            {error ? <p role="alert">{error.message}</p> : null}
            <HStack gap={2} justify="end">
              <Button type="button" size="sm" variant="ghost" label="Cancel" onClick={onClose} />
              <Button type="submit" size="sm" variant="primary" label={action === "UNSCHEDULE" ? "Unschedule" : "Save"} isLoading={busy} isDisabled={busy || (action !== "UNSCHEDULE" && !scheduleInput(effectiveDraft))} />
            </HStack>
          </VStack>
        </LayoutContent>}
      />
    </Dialog>
  );
}
