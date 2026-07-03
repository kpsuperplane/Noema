import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { createClientId } from "./clientId";

describe("createClientId", () => {
  test("uses native randomUUID when available", () => {
    assert.equal(
      createClientId({
        randomUUID: () => "native-id"
      }),
      "native-id"
    );
  });

  test("uses getRandomValues when randomUUID is unavailable", () => {
    const id = createClientId({
      getRandomValues: (array) => {
        array.fill(0x11);
        return array;
      }
    });

    assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  });

  test("falls back without browser crypto", () => {
    const id = createClientId({});

    assert.match(id, /^fallback-[0-9a-z]+-[0-9a-z]+$/);
  });
});
