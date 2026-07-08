import type { MemoryServiceStatusKind } from "@/generated/graphql";

export function memoryStatusLabel(status: MemoryServiceStatusKind): string {
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
