import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { composerPlaceholder } from "./components/ChatSurface";
import {
  canSendMessage,
  shouldRefreshLocalStatusForConversationEvent
} from "./App";

describe("canSendMessage", () => {
  test("allows sending while an agent turn is pending", () => {
    assert.equal(
      canSendMessage({
        text: "Interrupt with this",
        conversationId: "conversation_123",
        socketState: "ready",
        pending: true
      }),
      true
    );
  });

  test("blocks empty, disconnected, and non-ready sends", () => {
    assert.equal(canSendMessage({ text: "   ", conversationId: "conversation_123", socketState: "ready", pending: false }), false);
    assert.equal(canSendMessage({ text: "Hello", conversationId: null, socketState: "ready", pending: false }), false);
    assert.equal(
      canSendMessage({ text: "Hello", conversationId: "conversation_123", socketState: "connecting", pending: false }),
      false
    );
  });
});

describe("composerPlaceholder", () => {
  test("uses the agent name only after chat is ready and the agent is named", () => {
    assert.equal(composerPlaceholder({ ready: true, agentName: "Fred" }), "Message Fred");
    assert.equal(composerPlaceholder({ ready: true, agentName: null }), "Send a message");
    assert.equal(composerPlaceholder({ ready: true, agentName: "   " }), "Send a message");
    assert.equal(
      composerPlaceholder({ ready: false, agentName: "Fred" }),
      "Starting Noema chat..."
    );
  });
});

describe("shouldRefreshLocalStatusForConversationEvent", () => {
  test("refreshes local status after a successful own-name tool result", () => {
    assert.equal(
      shouldRefreshLocalStatusForConversationEvent({
        __typename: "GraphqlConversationItemEvent",
        item: {
          __typename: "GraphqlActivity",
          activityKind: "tool_result",
          metadata: {
            action: {
              name: "update_own_name",
              success: true
            }
          }
        }
      }),
      true
    );
    assert.equal(
      shouldRefreshLocalStatusForConversationEvent({
        __typename: "GraphqlConversationItemEvent",
        item: {
          __typename: "GraphqlActivity",
          activityKind: "tool_result",
          metadata: {
            action: {
              name: "search_memory",
              success: true
            }
          }
        }
      }),
      false
    );
  });
});
