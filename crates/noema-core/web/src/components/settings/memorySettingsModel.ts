import type { MemoryServiceMode, MemoryServiceStatusKind } from "@/generated/graphql";

export function memoryStatusLabel(
  status: MemoryServiceStatusKind,
  mode: MemoryServiceMode
): string {
  if (mode === "MANAGED") {
    switch (status) {
      case "NOT_CONFIGURED":
        return "Managed Mem0 has not started.";
      case "STARTING":
        return "Managed Mem0 is starting.";
      case "READY":
        return "Managed Mem0 is ready.";
      case "UNAVAILABLE":
        return "Managed Mem0 is unavailable.";
      case "AUTH_ERROR":
        return "Managed Mem0 authentication needs attention.";
    }
  }

  switch (status) {
    case "NOT_CONFIGURED":
      return "Mem0 has not been configured.";
    case "STARTING":
      return "Mem0 is starting.";
    case "READY":
      return "Mem0 is ready.";
    case "UNAVAILABLE":
      return "Mem0 is unavailable.";
    case "AUTH_ERROR":
      return "Mem0 authentication needs attention.";
  }
}
