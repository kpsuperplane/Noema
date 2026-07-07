import { createFileRoute } from "@tanstack/react-router";
import { useAppRuntime } from "@/app/AppRuntimeContext";
import { MemoryHomePage } from "@/pages/MemoryHomePage";

export const Route = createFileRoute("/memory")({
  component: MemoryRoute
});

function MemoryRoute() {
  const { openMemoryGraph } = useAppRuntime();
  return <MemoryHomePage onOpenGraph={openMemoryGraph} />;
}
