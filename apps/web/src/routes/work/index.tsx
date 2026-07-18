import { createFileRoute } from "@tanstack/react-router";
import { WorkSurface } from "@/components/work/WorkSurface";
import { normalizeWorkSearch } from "@/components/work/workTypes";

export const Route = createFileRoute("/work/")({
  validateSearch: normalizeWorkSearch,
  component: WorkIndexRoute
});

function WorkIndexRoute() {
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  return <WorkSurface search={search} onSearchChange={(next, replace) => void navigate({ search: next, replace })} />;
}
