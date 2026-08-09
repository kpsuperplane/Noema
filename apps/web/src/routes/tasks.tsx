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
  const selectedTaskId = taskMatch ? taskMatch.taskId : undefined;

  const closeTask = () => {
    void navigate({ to: "/tasks", search, replace: true });
  };

  return (
    <TasksSurface
      search={search}
      selectedTaskId={selectedTaskId}
      onCloseTask={selectedTaskId ? closeTask : undefined}
    />
  );
}
