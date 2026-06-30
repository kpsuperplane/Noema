import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { ApolloLink } from "@apollo/client";
import type { ApolloClient } from "@apollo/client";
import { parse } from "graphql";
import { createDesktopGraphqlLink } from "./desktopTransport";
import { isTauriRuntime } from "./transportMode";

describe("isTauriRuntime", () => {
  test("detects Tauri from window internals", () => {
    assert.equal(isTauriRuntime({ __TAURI_INTERNALS__: { invoke() {} } }), true);
    assert.equal(isTauriRuntime({ __TAURI_INTERNALS__: {} }), false);
    assert.equal(isTauriRuntime({}), false);
    assert.equal(isTauriRuntime(undefined), false);
  });
});

describe("createDesktopGraphqlLink", () => {
  test("waits for subscription listener registration before invoking subscribe", async () => {
    let resolveListen: ((unlisten: () => void) => void) | undefined;
    const listenPromise = new Promise<() => void>((resolve) => {
      resolveListen = resolve;
    });
    const invokedCommands: string[] = [];
    const link = createDesktopGraphqlLink({
      createSubscriptionId: () => "sub_1",
      invokeDesktop: async <T>(command: string) => {
        invokedCommands.push(command);
        return {} as T;
      },
      listenDesktop: async () => listenPromise
    });

    const subscription = ApolloLink.execute(
      link,
      {
        query: parse(`
          subscription WatchConversation {
            conversationEvents(conversationId: "conversation:1") {
              __typename
            }
          }
        `)
      },
      { client: {} as unknown as ApolloClient }
    ).subscribe({});

    await Promise.resolve();
    assert.deepEqual(invokedCommands, []);

    resolveListen?.(() => {});
    await listenPromise;
    await Promise.resolve();
    assert.deepEqual(invokedCommands, ["graphql_subscribe"]);

    subscription.unsubscribe();
  });

  test("waits for subscribe to finish before invoking unsubscribe", async () => {
    let resolveSubscribe: (() => void) | undefined;
    const subscribePromise = new Promise<void>((resolve) => {
      resolveSubscribe = resolve;
    });
    const invokedCommands: string[] = [];
    const link = createDesktopGraphqlLink({
      createSubscriptionId: () => "sub_1",
      invokeDesktop: async <T>(command: string) => {
        invokedCommands.push(command);
        if (command === "graphql_subscribe") {
          await subscribePromise;
        }
        return {} as T;
      },
      listenDesktop: async () => () => {}
    });

    const subscription = ApolloLink.execute(
      link,
      {
        query: parse(`
          subscription WatchConversation {
            conversationEvents(conversationId: "conversation:1") {
              __typename
            }
          }
        `)
      },
      { client: {} as unknown as ApolloClient }
    ).subscribe({});

    await Promise.resolve();
    await Promise.resolve();
    assert.deepEqual(invokedCommands, ["graphql_subscribe"]);

    subscription.unsubscribe();
    await Promise.resolve();
    assert.deepEqual(invokedCommands, ["graphql_subscribe"]);

    resolveSubscribe?.();
    await subscribePromise;
    await Promise.resolve();
    assert.deepEqual(invokedCommands, ["graphql_subscribe", "graphql_unsubscribe"]);
  });
});
