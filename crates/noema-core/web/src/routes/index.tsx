import { createFileRoute } from "@tanstack/react-router";
import { useAppRuntime } from "@/app/AppRuntimeContext";

export const Route = createFileRoute("/")({
  component: ChatRoute
});

function ChatRoute() {
  return useAppRuntime().chatView;
}
