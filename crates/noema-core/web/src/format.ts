import type { TurnActivityStatus as ActivityStatus } from "./generated/noema";

export function statusLabel(status: ActivityStatus) {
  if (status === "started") {
    return "Running";
  }
  if (status === "failed") {
    return "Failed";
  }
  return "Done";
}

export function readableKind(kind: string) {
  return kind.replaceAll("_", " ");
}

export function formatPercent(value: number) {
  return `${Math.round(value * 100)}% confidence`;
}
