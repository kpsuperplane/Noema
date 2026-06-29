import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { IdentityAvatar } from "./IdentityAvatar";

describe("IdentityAvatar", () => {
  test("uses actor ids as stable seeds without Noema green avatar fills", () => {
    const markup = ["human:local", "agent:local", "workspace:design", "project:rail"]
      .map((actorId) => renderToStaticMarkup(React.createElement(IdentityAvatar, { actorId })))
      .join("");

    assert.match(markup, /data-avatar-seed="human:local"/);
    assert.match(markup, /data-avatar-seed="agent:local"/);
    assert.doesNotMatch(markup, /#(?:176046|1f7a57|114a37)/i);
  });
});
