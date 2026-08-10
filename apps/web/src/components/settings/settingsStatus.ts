const STATUS_LABELS: Record<string, string> = {
  active: "Ready",
  authenticated: "Authenticated",
  checking: "Checking",
  defaulted: "Cautious",
  disabled: "Off",
  failed: "Authentication failed",
  healthy: "Ready",
  needs_auth: "Authorization required",
  pending: "Checking",
  ready: "Ready",
  required: "Authorization required",
  unavailable: "Unavailable",
  unauthenticated: "Authorization required",
  unknown: "Unknown"
};

export function settingsStatusLabel(status: string) {
  const key = status.toLowerCase();
  return STATUS_LABELS[key]
    ?? key.replaceAll("_", " ").replace(/^./, (character) => character.toUpperCase());
}
