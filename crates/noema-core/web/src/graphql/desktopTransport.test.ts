import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { isTauriRuntime } from "./transportMode";

describe("isTauriRuntime", () => {
  test("detects Tauri from window internals", () => {
    assert.equal(isTauriRuntime({ __TAURI_INTERNALS__: {} }), true);
    assert.equal(isTauriRuntime({}), false);
    assert.equal(isTauriRuntime(undefined), false);
  });
});
