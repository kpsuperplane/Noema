import type {
  WorkActivityQuery,
  WorkCompletedTasksQuery,
  WorkNeedsYouQuery,
  WorkOverviewQuery,
  WorkProjectsQuery,
  WorkTaskDetailQuery,
  WorkTaskRunItemsQuery,
  WorkTasksQuery
} from "@/generated/graphql";

export const PERSONAL_WORKSPACE_ID = "workspace:personal";

export type WorkView = "board" | "list" | "needs-you" | "activity" | "completed";
export type WorkProject = WorkProjectsQuery["projects"]["edges"][number]["node"];
export type WorkOverview = WorkOverviewQuery["workOverview"];
export type WorkTask = WorkTasksQuery["workTasks"]["edges"][number]["node"];
export type CompletedWorkTask = WorkCompletedTasksQuery["completedTasks"]["edges"][number]["node"];
export type WorkAttention = WorkNeedsYouQuery["needsYou"]["edges"][number]["node"];
export type WorkEvent = WorkActivityQuery["workActivity"]["edges"][number]["node"];
export type WorkTaskDetail = NonNullable<WorkTaskDetailQuery["task"]>;
export type WorkTaskRun = WorkTaskDetail["runs"]["edges"][number]["node"];
export type WorkTaskRunItem = WorkTaskRunItemsQuery["taskRunItems"]["edges"][number]["node"];

export type WorkSearch = {
  view: WorkView;
  project?: string;
  q?: string;
  terminal?: "all" | "accepted" | "cancelled";
};

export function normalizeWorkSearch(search: Record<string, unknown>): WorkSearch {
  const view = isWorkView(search.view) ? search.view : "board";
  const project = normalizedText(search.project);
  const q = normalizedText(search.q);
  const terminal =
    search.terminal === "accepted" || search.terminal === "cancelled"
      ? search.terminal
      : "all";
  return { view, project, q, terminal };
}

function isWorkView(value: unknown): value is WorkView {
  return (
    value === "board" ||
    value === "list" ||
    value === "needs-you" ||
    value === "activity" ||
    value === "completed"
  );
}

function normalizedText(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}
