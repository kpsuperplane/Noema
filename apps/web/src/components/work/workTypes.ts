import type {
  WorkProjectsQuery,
  WorkTaskDetailQuery,
  WorkTaskRunItemsQuery,
  WorkTasksQuery
} from "@/generated/graphql";

export const PERSONAL_WORKSPACE_ID = "workspace:personal";

export type WorkProject = WorkProjectsQuery["projects"]["edges"][number]["node"];
export type WorkTask = WorkTasksQuery["workTasks"]["edges"][number]["node"];
export type WorkTaskDetail = NonNullable<WorkTaskDetailQuery["task"]>;
export type WorkTaskRun = WorkTaskDetail["runs"][number];
export type WorkTaskRunItem = WorkTaskRunItemsQuery["taskRunItems"]["edges"][number]["node"];

export type WorkSearch = {
  project?: string;
  q?: string;
  terminal?: "all" | "completed" | "cancelled";
};

export function normalizeWorkSearch(search: Record<string, unknown>): WorkSearch {
  const project = normalizedText(search.project);
  const q = normalizedText(search.q);
  const terminal =
    search.terminal === "completed" || search.terminal === "cancelled"
      ? search.terminal
      : "all";
  return { project, q, terminal };
}

function normalizedText(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
