import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { mcpOAuthRedirectUri } from "./mcpOAuthCallback";

describe("mcpOAuthRedirectUri", () => {
  test("uses same-origin callback outside desktop", async () => {
    const uri = await mcpOAuthRedirectUri(
      { origin: "http://127.0.0.1:3737" },
      { isDesktop: false }
    );

    assert.equal(uri, "http://127.0.0.1:3737/mcp/oauth/callback");
  });

  test("uses desktop loopback callback URL inside Tauri", async () => {
    const invoked: string[] = [];
    const uri = await mcpOAuthRedirectUri(
      { origin: "http://127.0.0.1:5173" },
      {
        isDesktop: true,
        invoke: async (command) => {
          invoked.push(command);
          return "http://127.0.0.1:49152/mcp/oauth/callback";
        }
      }
    );

    assert.deepEqual(invoked, ["mcp_oauth_callback_url"]);
    assert.equal(uri, "http://127.0.0.1:49152/mcp/oauth/callback");
  });
});
