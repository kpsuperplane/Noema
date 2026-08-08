import type { TaskDetail } from "./taskTypes";

export function taskStageLabel(detail: TaskDetail): string {
  switch (detail.stageBehavior) {
    case "INTAKE":
      return "Inbox";
    case "DISPATCH":
      return "Queue";
    case "ACTIVE":
      return "Doing";
    case "HUMAN_GATE":
      return "Waiting";
    case "TERMINAL_SUCCESS":
      return "Done";
    case "TERMINAL_CANCELLED":
      return "Cancelled";
  }
}
