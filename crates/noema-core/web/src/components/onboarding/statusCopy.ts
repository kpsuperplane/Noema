import type { ProviderAccountStatus, ProviderAuthAttemptView } from "./types";

export const statusCopy: Record<ProviderAccountStatus | ProviderAuthAttemptView["status"], string> = {
  UNKNOWN: "Not checked yet",
  CHECKING: "Checking",
  AUTHENTICATED: "Connected",
  UNAUTHENTICATED: "Not connected",
  UNAVAILABLE: "Unavailable",
  STARTING: "Starting login",
  WAITING_FOR_USER: "Waiting for login",
  COMPLETED: "Connected",
  FAILED: "Login failed",
  EXPIRED: "Login expired",
  CANCELLED: "Login cancelled"
};
