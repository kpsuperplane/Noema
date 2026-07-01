import type { TurnActivityStatus } from "./generated/graphql";

export function statusLabel(status: TurnActivityStatus) {
  if (status === "STARTED") {
    return "Running";
  }
  if (status === "FAILED") {
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
