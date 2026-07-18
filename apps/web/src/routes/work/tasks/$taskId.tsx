import { createFileRoute, useRouter } from "@tanstack/react-router";
import { WorkTaskDetailContainer } from "@/components/work/detail/WorkTaskDetailContainer";
import { normalizeWorkSearch } from "@/components/work/workTypes";

export const Route = createFileRoute("/work/tasks/$taskId")({
  validateSearch: normalizeWorkSearch,
  component: WorkTaskDetailRoute
});

function WorkTaskDetailRoute() {
  const { taskId } = Route.useParams();
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  const router = useRouter();
  const back = () => {
    if (router.history.canGoBack()) {
      router.history.back();
      return;
    }
    void navigate({ to: "/work", search });
  };
  return <WorkTaskDetailContainer taskId={taskId} onBack={back} />;
}
