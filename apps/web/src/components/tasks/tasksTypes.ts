import type {
  TasksProjectsQuery,
  TasksTaskSummaryFieldsFragment,
  TasksTaskDetailQuery,
} from "@/generated/graphql";

export const PERSONAL_WORKSPACE_ID = "workspace:personal";

export type TasksProject = TasksProjectsQuery["projects"]["edges"][number]["node"];
export type TasksTask = TasksTaskSummaryFieldsFragment;
export type TasksTaskDetail = NonNullable<TasksTaskDetailQuery["task"]>;

export type TasksSearch = {
  project?: string;
  terminal?: "all" | "completed" | "cancelled";
};

export function normalizeTasksSearch(search: Record<string, unknown>): TasksSearch {
  const project = normalizedText(search.project);
  const terminal =
    search.terminal === "completed" || search.terminal === "cancelled"
      ? search.terminal
      : "all";
  return { project, terminal };
}

function normalizedText(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
