type ApprovalDecision = "APPROVED" | "DECLINED";

const actionPriority: Record<string, number> = {
  QUEUE: 10,
  RETRY: 30,
  REOPEN: 30,
  ANSWER: 40,
  EDIT: 50,
  CANCEL: 100
};

export function orderTaskActions(actions: readonly string[]): string[] {
  return actions
    .map((action, index) => ({ action, index }))
    .sort((left, right) => {
      const priority = (actionPriority[left.action] ?? 90) - (actionPriority[right.action] ?? 90);
      return priority || left.index - right.index;
    })
    .map(({ action }) => action);
}

export function taskActionLabel(action: string, compact: boolean, approvalDecision?: ApprovalDecision): string {
  if (action === "ANSWER" && approvalDecision) {
    return approvalDecision === "APPROVED" ? "Approve" : "Decline";
  }
  const labels: Record<string, string> = { EDIT: "Edit", QUEUE: "Queue", ANSWER: "Answer", RETRY: "Retry", CANCEL: "Cancel", REOPEN: "Reopen" };
  return labels[action] ?? action;
}
