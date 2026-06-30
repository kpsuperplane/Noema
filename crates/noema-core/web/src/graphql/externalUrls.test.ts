import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { openExternalUrlForAuth } from "./externalUrls";

describe("openExternalUrlForAuth", () => {
  test("uses desktop command in Tauri mode", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const handled = await openExternalUrlForAuth("https://example.com/login", {
      isDesktop: true,
      invoke: async (command, args) => {
        calls.push({ command, args });
      }
    });

    assert.equal(handled, true);
    assert.deepEqual(calls, [
      {
        command: "open_external_url",
        args: { url: "https://example.com/login" }
      }
    ]);
  });

  test("does not invoke desktop command outside Tauri mode", async () => {
    const calls: string[] = [];
    const handled = await openExternalUrlForAuth("https://example.com/login", {
      isDesktop: false,
      invoke: async (command) => {
        calls.push(command);
      }
    });

    assert.equal(handled, false);
    assert.deepEqual(calls, []);
  });

  test("returns false when desktop command rejects", async () => {
    const handled = await openExternalUrlForAuth("https://example.com/login", {
      isDesktop: true,
      invoke: async () => {
        throw new Error("open command unavailable");
      }
    });

    assert.equal(handled, false);
  });
});
