import { createFileRoute, useMatch } from "@tanstack/react-router";
import { TasksSurface } from "@/components/tasks/TasksSurface";
import { normalizeTasksSearch } from "@/components/tasks/tasksTypes";

export const Route = createFileRoute("/tasks")({
  validateSearch: normalizeTasksSearch,
  component: TasksLayoutRoute
});

function TasksLayoutRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const taskMatch = useMatch({ from: "/tasks/$taskId", shouldThrow: false });
  const recurrenceMatch = useMatch({ from: "/tasks/recurrences/$recurrenceId", shouldThrow: false });
  const projectMatch = useMatch({ from: "/tasks/projects/$projectId", shouldThrow: false });
  const selectedTaskId = taskMatch ? taskMatch.params.taskId : undefined;
  const selectedDetail = projectMatch
    ? { type: "project" as const, projectId: projectMatch.params.projectId }
    : recurrenceMatch
    ? { type: "recurrence" as const, recurrenceId: recurrenceMatch.params.recurrenceId }
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
