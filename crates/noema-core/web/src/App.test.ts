import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { canSendMessage } from "./App";

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
