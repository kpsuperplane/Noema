export type ChatDetailTarget =
  | {
      type: "artifact";
      version: string;
    }
  | {
      type: "task";
      taskId: string;
    }
  | {
      type: "recurrence";
      recurrenceId: string;
    }
  | {
      type: "project";
      projectId: string;
    };

export function artifactDetailTarget(version: string | null | undefined): ChatDetailTarget | null {
  const trimmed = version?.trim();
  return trimmed ? { type: "artifact", version: trimmed } : null;
}

export function taskDetailTarget(taskId: string | null | undefined): ChatDetailTarget | null {
  const trimmed = taskId?.trim();
  return trimmed ? { type: "task", taskId: trimmed } : null;
}
