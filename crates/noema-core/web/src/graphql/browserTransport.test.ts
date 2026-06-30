import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  browserGraphqlWsRetryDelayMs,
  createBrowserGraphqlWsClientOptions
} from "./browserTransport";

describe("createBrowserGraphqlWsClientOptions", () => {
  test("keeps retrying subscription sockets without unbounded backoff", () => {
    const options = createBrowserGraphqlWsClientOptions({
      protocol: "http:",
      host: "127.0.0.1:5173"
    });

    assert.equal(options.url, "ws://127.0.0.1:5173/graphql/ws");
    assert.equal(options.lazy, true);
    assert.equal(options.retryAttempts, Number.POSITIVE_INFINITY);
    assert.equal(browserGraphqlWsRetryDelayMs(0), 500);
    assert.equal(browserGraphqlWsRetryDelayMs(8), 5_000);
  });

  test("uses secure websocket URLs on HTTPS origins", () => {
    const options = createBrowserGraphqlWsClientOptions({
      protocol: "https:",
      host: "noema.local"
    });

    assert.equal(options.url, "wss://noema.local/graphql/ws");
  });
});
