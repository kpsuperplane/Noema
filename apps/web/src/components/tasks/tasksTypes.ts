import type {
  TasksProjectsQuery,
  TasksTaskDetailQuery,
  TasksListQuery
} from "@/generated/graphql";

export const PERSONAL_WORKSPACE_ID = "workspace:personal";

export type TasksProject = TasksProjectsQuery["projects"]["edges"][number]["node"];
export type TasksTask = TasksListQuery["tasks"]["edges"][number]["node"];
export type TasksTaskDetail = NonNullable<TasksTaskDetailQuery["task"]>;

export type TasksSearch = {
  project?: string;
  q?: string;
  terminal?: "all" | "completed" | "cancelled";
};

export function normalizeTasksSearch(search: Record<string, unknown>): TasksSearch {
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
