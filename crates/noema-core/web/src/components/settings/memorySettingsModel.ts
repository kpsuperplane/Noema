import type { MemoryServiceMode, MemoryServiceStatusKind } from "@/generated/graphql";

export function memoryStatusLabel(
  status: MemoryServiceStatusKind,
  mode: MemoryServiceMode
): string {
  if (mode === "MANAGED") {
    switch (status) {
      case "NOT_CONFIGURED":
        return "Managed Mnemosyne has not started.";
      case "STARTING":
        return "Managed Mnemosyne is starting.";
      case "READY":
        return "Managed Mnemosyne is ready.";
      case "UNAVAILABLE":
        return "Managed Mnemosyne is unavailable.";
      case "AUTH_ERROR":
        return "Managed Mnemosyne authentication needs attention.";
    }
  }

  switch (status) {
    case "NOT_CONFIGURED":
      return "Mnemosyne has not been configured.";
    case "STARTING":
      return "Mnemosyne is starting.";
    case "READY":
      return "Mnemosyne is ready.";
    case "UNAVAILABLE":
      return "Mnemosyne is unavailable.";
    case "AUTH_ERROR":
      return "Mnemosyne authentication needs attention.";
  }
}
