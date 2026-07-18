import * as React from "react";

const taskEventCursorByTask = new Map<string, string>();

export function taskEventCursor(taskId: string): string | undefined {
  return taskEventCursorByTask.get(taskId);
}

export function recordTaskEventCursor(taskId: string, cursor: string): void {
  taskEventCursorByTask.set(taskId, cursor);
}

export function useTaskEventCursor(taskId: string): readonly [string | undefined, (cursor: string) => void] {
  const [localCursor, setLocalCursor] = React.useState(() => ({
    taskId,
    cursor: taskEventCursor(taskId)
  }));
  const cursor = localCursor.taskId === taskId ? localCursor.cursor : taskEventCursor(taskId);
  const recordCursor = React.useCallback(
    (nextCursor: string) => {
      recordTaskEventCursor(taskId, nextCursor);
      setLocalCursor({ taskId, cursor: nextCursor });
    },
    [taskId]
  );
  return [cursor, recordCursor];
}
