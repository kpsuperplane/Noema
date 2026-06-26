import type { Dispatch, SetStateAction } from "react";
import type {
  OnboardingStatus,
  ProviderAuthAttemptView,
  StartProviderAuthAttemptRequest,
  WebStatus
} from "./generated/noema";

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

export async function fetchOnboardingStatus(): Promise<OnboardingStatus> {
  const response = await fetch("/api/onboarding/status");
  if (!response.ok) {
    throw new Error("Failed to load onboarding status");
  }
  return (await response.json()) as OnboardingStatus;
}

export async function startProviderAuthAttempt(
  request: StartProviderAuthAttemptRequest
): Promise<ProviderAuthAttemptView> {
  const response = await fetch("/api/provider-auth/attempts", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(request)
  });
  if (!response.ok) {
    throw new Error(await responseErrorMessage(response, "Failed to start provider login"));
  }
  return (await response.json()) as ProviderAuthAttemptView;
}

export async function fetchProviderAuthAttempt(attemptId: string): Promise<ProviderAuthAttemptView> {
  const response = await fetch(`/api/provider-auth/attempts/${encodeURIComponent(attemptId)}`);
  if (!response.ok) {
    throw new Error(await responseErrorMessage(response, "Failed to poll provider login"));
  }
  return (await response.json()) as ProviderAuthAttemptView;
}

async function responseErrorMessage(response: Response, fallback: string): Promise<string> {
  const text = await response.text();
  if (!text.trim()) {
    return fallback;
  }

  try {
    const body = JSON.parse(text) as { error?: unknown };
    if (typeof body.error === "string" && body.error.trim()) {
      return body.error;
    }
  } catch {
    return text;
  }

  return text;
}
