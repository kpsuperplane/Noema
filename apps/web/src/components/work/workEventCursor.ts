import * as React from "react";

const cursorByWorkspace = new Map<string, string>();

export function useWorkEventCursor(workspaceId: string): readonly [string | undefined, (cursor: string) => void] {
  const [local, setLocal] = React.useState(() => ({ workspaceId, cursor: cursorByWorkspace.get(workspaceId) }));
  const cursor = local.workspaceId === workspaceId ? local.cursor : cursorByWorkspace.get(workspaceId);
  const record = React.useCallback((nextCursor: string) => {
    cursorByWorkspace.set(workspaceId, nextCursor);
    setLocal({ workspaceId, cursor: nextCursor });
  }, [workspaceId]);
  return [cursor, record];
}
