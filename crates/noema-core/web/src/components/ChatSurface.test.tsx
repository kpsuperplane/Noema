import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { ShellSurfaceProvider } from "./shell/ShellSurfaceContext";
import { ChatSurface, composerPlaceholder, shouldFocusChatComposer } from "./ChatSurface";

describe("shouldFocusChatComposer", () => {
  test("focuses only when chat is ready and the shell surface is visible", () => {
    assert.equal(shouldFocusChatComposer({ ready: true, visibility: "visible" }), true);
    assert.equal(shouldFocusChatComposer({ ready: true, visibility: "showing" }), false);
    assert.equal(shouldFocusChatComposer({ ready: true, visibility: "hidden" }), false);
    assert.equal(shouldFocusChatComposer({ ready: false, visibility: "visible" }), false);
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

describe("ChatSurface", () => {
  test("server-renders the chat landmark without native autofocus while shell surface is showing", () => {
    const markup = renderToStaticMarkup(
      <ShellSurfaceProvider value={{ visibility: "showing" }}>
        <ChatSurface
          transcript={[]}
          pending={false}
          expandedActivities={new Set()}
          draft=""
          ready={true}
          agentName="Fred"
          onPickStarter={() => {}}
          onToggleActivity={() => {}}
          onDraftChange={() => {}}
          onSubmit={() => {}}
        />
      </ShellSurfaceProvider>
    );

    assert.match(markup, /aria-label="Noema chat"/);
    assert.doesNotMatch(markup, /autofocus/i);
  });
});
