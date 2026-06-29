import assert from "node:assert/strict";
import { describe, test } from "node:test";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { IdentityAvatar, NOEMA_AVATAR_COLORS, avatarSeedForActorId } from "./IdentityAvatar";

describe("IdentityAvatar", () => {
  test("uses actor ids as stable seeds without Noema green avatar fills", () => {
    const markup = ["human:local", "agent:local", "workspace:design", "project:rail"]
      .map((actorId) => renderToStaticMarkup(React.createElement(IdentityAvatar, { actorId, actorType: "agent" })))
      .join("");

    assert.match(markup, new RegExp(`data-avatar-seed="${avatarSeedForActorId("human:local")}"`));
    assert.match(markup, new RegExp(`data-avatar-seed="${avatarSeedForActorId("agent:local")}"`));
    assert.doesNotMatch(markup, /human:local|agent:local/);
    assert.doesNotMatch(markup, /#(?:176046|1f7a57|114a37)/i);
  });

  test("uses a muted harmonious palette for generated avatar pairings", () => {
    assert.deepEqual(NOEMA_AVATAR_COLORS, ["#3b4a6b", "#7d6a91", "#b9786d", "#d6ad6b", "#e6d8c4", "#2f3440"]);
  });

  test("uses marble avatars for humans and beam avatars for agents", () => {
    const humanMarkup = renderToStaticMarkup(
      React.createElement(IdentityAvatar, { actorId: "human:local", actorType: "human" })
    );
    const agentMarkup = renderToStaticMarkup(
      React.createElement(IdentityAvatar, { actorId: "agent:local", actorType: "agent" })
    );

    assert.match(humanMarkup, /data-avatar-variant="marble"/);
    assert.match(agentMarkup, /data-avatar-variant="beam"/);
  });

  test("hashes actor ids into stable avatar seeds", () => {
    const humanSeed = avatarSeedForActorId("human:local");
    const agentSeed = avatarSeedForActorId("agent:local");

    assert.match(humanSeed, /^actor-[a-f0-9]{8}$/);
    assert.equal(humanSeed, avatarSeedForActorId("human:local"));
    assert.notEqual(humanSeed, "human:local");
    assert.notEqual(humanSeed, agentSeed);
  });
});
