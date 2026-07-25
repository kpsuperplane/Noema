import type {
  WorkProjectsQuery,
  WorkTaskDetailQuery,
  WorkTaskRunItemsQuery,
  WorkTasksQuery
} from "@/generated/graphql";

export const PERSONAL_WORKSPACE_ID = "workspace:personal";

export type WorkView = "tasks" | "history";
export type WorkProject = WorkProjectsQuery["projects"]["edges"][number]["node"];
export type WorkTask = WorkTasksQuery["workTasks"]["edges"][number]["node"];
export type WorkTaskDetail = NonNullable<WorkTaskDetailQuery["task"]>;
export type WorkTaskRun = WorkTaskDetail["runs"][number];
export type WorkTaskRunItem = WorkTaskRunItemsQuery["taskRunItems"]["edges"][number]["node"];

export type WorkSearch = {
  view: WorkView;
  project?: string;
  q?: string;
  terminal?: "all" | "completed" | "cancelled";
};

export function normalizeWorkSearch(search: Record<string, unknown>): WorkSearch {
  const view = isWorkView(search.view) ? search.view : "tasks";
  const project = normalizedText(search.project);
  const q = normalizedText(search.q);
  const terminal =
    search.terminal === "completed" || search.terminal === "cancelled"
      ? search.terminal
      : "all";
  return { view, project, q, terminal };
}

function isWorkView(value: unknown): value is WorkView {
  return value === "tasks" || value === "history";
}

function normalizedText(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
