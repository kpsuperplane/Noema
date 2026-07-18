export function taskActionLabel(action: string, compact: boolean): string {
  const labels: Record<string, string> = { EDIT: "Edit", QUEUE: "Queue", ANSWER: "Answer", RETRY: "Retry", ACCEPT: "Accept", REQUEST_CHANGES: compact ? "Changes" : "Request changes", CANCEL: "Cancel", REOPEN: "Reopen" };
  return labels[action] ?? action;
}
