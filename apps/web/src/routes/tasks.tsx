import { createFileRoute, useMatchRoute } from "@tanstack/react-router";
import { TasksSurface } from "@/components/tasks/TasksSurface";
import { normalizeTasksSearch } from "@/components/tasks/tasksTypes";

export const Route = createFileRoute("/tasks")({
  validateSearch: normalizeTasksSearch,
  component: TasksLayoutRoute
});

function TasksLayoutRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const matchRoute = useMatchRoute();
  const taskMatch = matchRoute({ to: "/tasks/$taskId" });
  const recurrenceMatch = matchRoute({ to: "/tasks/recurrences/$recurrenceId" });
  const selectedTaskId = taskMatch ? taskMatch.taskId : undefined;
  const selectedDetail = recurrenceMatch
    ? { type: "recurrence" as const, recurrenceId: recurrenceMatch.recurrenceId }
    : selectedTaskId
      ? { type: "task" as const, taskId: selectedTaskId }
      : undefined;

  const closeTask = () => {
    void navigate({ to: "/tasks", search, replace: true });
  };

  return (
    <TasksSurface
      search={search}
      selectedDetail={selectedDetail}
      onCloseDetail={selectedDetail ? closeTask : undefined}
    />
  );
}
