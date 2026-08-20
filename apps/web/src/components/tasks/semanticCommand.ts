import { CombinedGraphQLErrors } from "@apollo/client/errors";
import type { TaskCommandSubject } from "./useTaskCommands";

export function isStaleCommandError(error: unknown): boolean {
  if (!CombinedGraphQLErrors.is(error)) return false;
  return error.errors.some((item) =>
    item.extensions?.code === "stale_revision" || item.extensions?.code === "stale_generation" || item.extensions?.code === "stale_document"
  );
}

export function taskSubjectChanged(snapshot: TaskCommandSubject, current: TaskCommandSubject): boolean {
  if (snapshot.taskId !== current.taskId) return true;
  if (current.generation > snapshot.generation) return true;
  if (current.generation < snapshot.generation) return false;
  if (current.revision > snapshot.revision) return true;
  if (current.revision < snapshot.revision) return false;
  return snapshot.activeGate?.gateId !== current.activeGate?.gateId
    || snapshot.activeGate?.kind !== current.activeGate?.kind;
}

export function snapshotTaskSubject(task: TaskCommandSubject): TaskCommandSubject {
  return {
    ...task,
    project: task.project ? { ...task.project } : null,
    activeGate: task.activeGate ? { ...task.activeGate } : null
  };
}
