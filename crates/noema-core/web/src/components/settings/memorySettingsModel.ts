import type { MemoryServiceMode, MemoryServiceStatusKind } from "@/generated/graphql";

export function memoryStatusLabel(
  status: MemoryServiceStatusKind,
  mode: MemoryServiceMode
): string {
  if (mode === "MANAGED") {
    switch (status) {
      case "NOT_CONFIGURED":
        return "Managed Supermemory has not started.";
      case "STARTING":
        return "Managed Supermemory is starting.";
      case "READY":
        return "Managed Supermemory is ready.";
      case "UNAVAILABLE":
        return "Managed Supermemory is unavailable.";
      case "AUTH_ERROR":
        return "Managed Supermemory authentication needs attention.";
    }
  }

  switch (status) {
    case "NOT_CONFIGURED":
      return "Supermemory has not been configured.";
    case "STARTING":
      return "Supermemory is starting.";
    case "READY":
      return "Supermemory is ready.";
    case "UNAVAILABLE":
      return "Supermemory is unavailable.";
    case "AUTH_ERROR":
      return "Supermemory authentication needs attention.";
  }
}
