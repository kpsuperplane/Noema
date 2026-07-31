import { createFileRoute } from "@tanstack/react-router";
import { WorkSurface } from "@/components/work/WorkSurface";
import { normalizeWorkSearch } from "@/components/work/workTypes";

export const Route = createFileRoute("/work/tasks/$taskId")({
  validateSearch: normalizeWorkSearch,
  component: WorkTaskDetailRoute
});

function WorkTaskDetailRoute() {
  const { taskId } = Route.useParams();
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const back = () => {
    void navigate({ to: "/work", search, replace: true });
  };
  return <WorkSurface
    search={search}
    selectedTaskId={taskId}
    onCloseTask={back}
  />;
}
