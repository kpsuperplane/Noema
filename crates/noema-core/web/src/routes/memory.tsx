import { createFileRoute, redirect } from "@tanstack/react-router";

export const Route = createFileRoute("/memory")({
  beforeLoad: () => {
    throw redirect({ to: "/settings/memory" });
  }
});
