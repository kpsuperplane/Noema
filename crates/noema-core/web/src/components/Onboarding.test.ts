import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  isProviderAuthAttemptPending,
  PROVIDER_AUTH_POLL_INTERVAL_MS
} from "./Onboarding";

describe("isProviderAuthAttemptPending", () => {
  test("keeps polling only while provider auth is active", () => {
    assert.equal(PROVIDER_AUTH_POLL_INTERVAL_MS, 5_000);
    assert.equal(isProviderAuthAttemptPending("STARTING"), true);
    assert.equal(isProviderAuthAttemptPending("WAITING_FOR_USER"), true);
    assert.equal(isProviderAuthAttemptPending("COMPLETED"), false);
    assert.equal(isProviderAuthAttemptPending("FAILED"), false);
    assert.equal(isProviderAuthAttemptPending("EXPIRED"), false);
    assert.equal(isProviderAuthAttemptPending("CANCELLED"), false);
  });
});
