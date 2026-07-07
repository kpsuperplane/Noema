import { createFileRoute } from "@tanstack/react-router";
import { useAppRuntime } from "@/app/AppRuntimeContext";
import { MemoryHomePage } from "@/pages/MemoryHomePage";

export const Route = createFileRoute("/memory/")({
  component: MemoryHomeRoute
});

function MemoryHomeRoute() {
  const { openMemoryGraph } = useAppRuntime();
  return <MemoryHomePage onOpenGraph={openMemoryGraph} />;
}
