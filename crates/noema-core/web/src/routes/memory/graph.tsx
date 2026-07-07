import { createFileRoute } from "@tanstack/react-router";
import { MemoryGraphPage } from "@/pages/MemoryGraphPage";

export const Route = createFileRoute("/memory/graph")({
  component: MemoryGraphRoute
});

function MemoryGraphRoute() {
  return <MemoryGraphPage />;
}
