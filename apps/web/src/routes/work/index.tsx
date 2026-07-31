import { createFileRoute } from "@tanstack/react-router";
import { WorkSurface } from "@/components/work/WorkSurface";
import { normalizeWorkSearch } from "@/components/work/workTypes";

export const Route = createFileRoute("/work/")({
  validateSearch: normalizeWorkSearch,
  component: WorkIndexRoute
});

function WorkIndexRoute() {
  const search = Route.useSearch();
  return <WorkSurface search={search} />;
}
