import type { Dispatch, SetStateAction } from "react";
import type { WebStatus } from "./generated/noema";

export async function refreshStatus(setStatus: Dispatch<SetStateAction<WebStatus | null>>) {
  try {
    const response = await fetch("/api/status");
    if (response.ok) {
      setStatus((await response.json()) as WebStatus);
    }
  } catch {
    setStatus(null);
  }
}

export function webSocketUrl() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/api/chat/ws`;
}
