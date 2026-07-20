import { createFileRoute } from "@tanstack/react-router";
import { memoryPagePathFromUrl } from "@/app/routes";
import { MemoryPage } from "@/pages/MemoryPage";

export const Route = createFileRoute("/memory/$")({
  component: MemoryArticleRoute
});

function MemoryArticleRoute() {
  const { _splat } = Route.useParams();
  return <MemoryPage pagePath={_splat ? memoryPagePathFromUrl(_splat) : null} />;
}
