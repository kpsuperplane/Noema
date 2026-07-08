import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/memory/graph")({
  beforeLoad: () => {
    throw redirect({ to: "/settings/memory" });
  }
});
