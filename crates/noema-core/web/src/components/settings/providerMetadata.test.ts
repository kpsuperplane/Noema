import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  providerAuthMethodLabel,
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderSettingsAccount
} from "./providerMetadata";

const connectedAccount: ProviderSettingsAccount = {
  providerKind: "codex",
  accountKey: "default",
  displayName: "Codex",
  authMethod: "oauth_device_code",
  status: "AUTHENTICATED",
  isActive: true,
  isDefault: true,
  lastCheckedAt: "2026-06-30T12:00:00Z",
  lastAuthenticatedAt: "2026-06-30T11:59:00Z",
  lastErrorCode: null,
  lastErrorMessage: null
};

describe("provider settings metadata", () => {
  test("formats status and auth method labels", () => {
    assert.equal(providerStatusLabel("AUTHENTICATED"), "Authenticated");
    assert.equal(providerStatusLabel("UNAUTHENTICATED"), "Unauthenticated");
    assert.equal(providerAuthMethodLabel("oauth_device_code"), "OAuth device code");
  });

  test("returns safe technical rows without provider account ids or credentials", () => {
    const rows = providerTechnicalRows(connectedAccount);

    assert.deepEqual(rows, [
      { label: "Provider kind", value: "codex" },
      { label: "Account key", value: "default" },
      { label: "Auth method", value: "OAuth device code" },
      { label: "Status", value: "Authenticated" },
      { label: "Active", value: "Yes" },
      { label: "Default", value: "Yes" },
      { label: "Last checked", value: "2026-06-30T12:00:00Z" },
      { label: "Last authenticated", value: "2026-06-30T11:59:00Z" }
    ]);

    const text = JSON.stringify(rows);
    assert.equal(text.includes("provider_account:"), false);
    assert.equal(text.includes("auth.json"), false);
    assert.equal(text.includes("codex_tokens.json"), false);
    assert.equal(text.includes("token"), false);
    assert.equal(text.includes("api_key"), false);
  });

  test("includes non-secret provider error metadata when present", () => {
    const rows = providerTechnicalRows({
      ...connectedAccount,
      lastErrorCode: "codex_unavailable",
      lastErrorMessage: "Codex command is unavailable"
    });

    assert.deepEqual(rows.slice(-2), [
      { label: "Last error code", value: "codex_unavailable" },
      { label: "Last error message", value: "Codex command is unavailable" }
    ]);
  });
});
