import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { parseKeyValueLines } from "./mcpSetupForm";

describe("parseKeyValueLines", () => {
  test("parses key value lines", () => {
    assert.deepEqual(parseKeyValueLines("A=1\nB = two").value, { A: "1", B: "two" });
  });

  test("rejects duplicate keys", () => {
    const result = parseKeyValueLines("A=1\nA=2");
    assert.equal(result.error, "Duplicate key: A");
  });

  test("rejects malformed lines", () => {
    const result = parseKeyValueLines("NO_EQUALS");
    assert.equal(result.error, "Line 1 must use KEY=value.");
  });
});
