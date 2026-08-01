import { createFileRoute, useMatchRoute } from "@tanstack/react-router";
import { WorkSurface } from "@/components/work/WorkSurface";
import { normalizeWorkSearch } from "@/components/work/workTypes";

export const Route = createFileRoute("/work")({
  validateSearch: normalizeWorkSearch,
  component: WorkLayoutRoute
});

function WorkLayoutRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const matchRoute = useMatchRoute();
  const taskMatch = matchRoute({ to: "/work/tasks/$taskId" });
  const selectedTaskId = taskMatch ? taskMatch.taskId : undefined;

  const closeTask = () => {
    void navigate({ to: "/work", search, replace: true });
  };

  return (
    <WorkSurface
      search={search}
      selectedTaskId={selectedTaskId}
      onCloseTask={selectedTaskId ? closeTask : undefined}
    />
  );
}
