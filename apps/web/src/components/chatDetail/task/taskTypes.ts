export type TaskRunRole = "planner" | "executor" | "reviewer";

export type TaskRunItem = {
  id: string;
  runId?: string | null;
  sequenceIndex?: number | null;
  roundIndex?: number | null;
  sourceKind?: string;
  kind: "tool" | "message" | "result" | "artifact" | "status";
  title: string;
  summary?: string | null;
  details?: string | null;
  payload?: unknown;
  role?: TaskRunRole;
  status?: "queued" | "running" | "completed" | "failed" | "cancelled" | "skipped" | null;
  correlationId?: string | null;
  parentItemId?: string | null;
  responseIndex?: number | null;
  occurredAt?: string | null;
  updatedAt?: string | null;
};
