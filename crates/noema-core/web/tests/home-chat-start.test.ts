import { readFile } from "node:fs/promises";
import { expect, test } from "bun:test";

const appSourcePath = new URL("../src/App.tsx", import.meta.url);
const embeddedAssetPath = new URL("../../src/daemon/web/assets/app.js", import.meta.url);

test("home chat opens the durable primary conversation", async () => {
  const appSource = await readFile(appSourcePath, "utf8");
  const embeddedAsset = await readFile(embeddedAssetPath, "utf8");

  expect(appSource).toContain('type: "primary_conversation_start"');
  expect(appSource).not.toContain('type: "conversation_start"');
  expect(embeddedAsset).toContain('"primary_conversation_start"');
  expect(embeddedAsset).not.toContain('type:"conversation_start"');
});
