import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import {
  TasksRescheduleTaskDocument,
  TasksScheduleTaskDocument,
  TasksTaskRecurrenceDocument,
  TasksUnscheduleTaskDocument
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { initialRecurrenceScheduleDraft, initialScheduleDraft, scheduleInput, ScheduleFields, type ScheduleDraft } from "./ScheduleFields";
import { TaskCard } from "./TasksViews";
import { taskStatusFromProjection } from "@/components/chatDetail/task/TaskStatusBadge";
import * as stylex from "@stylexjs/stylex";
import type { TaskCommandSubject } from "./useTaskCommands";

export function TaskScheduleDialog({ action, task, onClose }: {
  action: "SCHEDULE" | "RESCHEDULE" | "UNSCHEDULE" | null;
  task: TaskCommandSubject;
  onClose: () => void;
}) {
  const formId = React.useId();
  const open = action !== null;
  const [draft, setDraft] = React.useState<ScheduleDraft>(() => initialScheduleDraft(task.schedule ?? undefined));
  const [schedule, scheduleState] = useMutation(TasksScheduleTaskDocument);
  const [reschedule, rescheduleState] = useMutation(TasksRescheduleTaskDocument);
  const [unschedule, unscheduleState] = useMutation(TasksUnscheduleTaskDocument);
  const recurrence = useQuery(TasksTaskRecurrenceDocument, {
    variables: { recurrenceId: task.schedule?.recurrenceId ?? "" },
    skip: !task.schedule?.recurrenceId
  });
  const currentRecurrence = recurrence.data?.taskRecurrence;
  const effectiveDraft = currentRecurrence && draft.repeat === "custom" && !draft.cron
    ? { ...initialRecurrenceScheduleDraft({ ...currentRecurrence, startsAt: task.schedule!.scheduledFor }), missedRunPolicy: draft.missedRunPolicy }
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
        header={<DialogHeader title={action === "UNSCHEDULE" ? "Unschedule task" : action === "RESCHEDULE" ? "Reschedule task" : "Schedule task"} onOpenChange={(next) => { if (!next) onClose(); }} />}
        content={<LayoutContent>
          <VStack as="form" id={formId} gap={3} onSubmit={(event) => {
            event.preventDefault();
            if (busy) return;
            const base = { taskId: task.taskId, expectedRevision: task.revision, expectedGeneration: task.generation, clientMutationId: createClientId() };
            const nextSchedule = scheduleInput(effectiveDraft);
            const mutation = action === "UNSCHEDULE"
              ? unschedule({ variables: { input: base } })
              : nextSchedule
                ? (action === "RESCHEDULE" ? reschedule : schedule)({ variables: { input: { ...base, schedule: nextSchedule } } })
                : Promise.reject(new Error("Invalid schedule"));
            void mutation.then(onClose).catch(() => undefined);
          }}>
            <TaskCard taskId={task.taskId} title={task.title} note={task.taskDocumentPreview ?? task.taskDocument?.slice(0, 240)} timestamp={task.updatedAt} project={task.project?.name} status={task.stage ? taskStatusFromProjection({ ...task, stage: task.stage }) : undefined} statusLabel={task.schedule ? "Scheduled" : task.stage?.name} listItem={false} onClick={onClose} />
            {action === "UNSCHEDULE" ? <p>This removes only future timing. The task content and history stay intact.</p> : <ScheduleFields value={effectiveDraft} onChange={setDraft} />}
            {error ? <p role="alert">{error.message}</p> : null}

          </VStack>
        </LayoutContent>}
        footer={<LayoutFooter><HStack gap={2} className={stylex.props(styles.actions).className}>
              <Button type="button" size="md" variant="secondary" label="Cancel" onClick={onClose} />
              <Button type="submit" form={formId} size="md" variant="primary" label={action === "UNSCHEDULE" ? "Return to Inbox" : action === "RESCHEDULE" ? "Save schedule" : "Schedule task"} isLoading={busy} isDisabled={busy || (action !== "UNSCHEDULE" && !scheduleInput(effectiveDraft))} />
            </HStack></LayoutFooter>}
      />
    </Dialog>
  );
}

const styles = stylex.create({ actions: { display: "grid", gridAutoFlow: "column", gridAutoColumns: "minmax(0, 1fr)" } });
