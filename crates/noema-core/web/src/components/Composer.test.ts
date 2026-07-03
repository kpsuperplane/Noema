import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { shouldSubmitFromPointerDown } from "./composerModel";

describe("shouldSubmitFromPointerDown", () => {
  test("submits primary touch activation before mobile focus changes can drop the click", () => {
    assert.equal(
      shouldSubmitFromPointerDown({
        ready: true,
        value: "hello",
        button: 0,
        isPrimary: true,
        pointerType: "touch"
      }),
      true
    );
  });

  test("leaves mouse activation on the click path", () => {
    assert.equal(
      shouldSubmitFromPointerDown({
        ready: true,
        value: "hello",
        button: 0,
        isPrimary: true,
        pointerType: "mouse"
      }),
      false
    );
  });

  test("ignores empty or non-primary pointer activation", () => {
    assert.equal(
      shouldSubmitFromPointerDown({
        ready: true,
        value: "   ",
        button: 0,
        isPrimary: true,
        pointerType: "touch"
      }),
      false
    );
    assert.equal(
      shouldSubmitFromPointerDown({
        ready: true,
        value: "hello",
        button: 1,
        isPrimary: true,
        pointerType: "touch"
      }),
      false
    );
    assert.equal(
      shouldSubmitFromPointerDown({
        ready: true,
        value: "hello",
        button: 0,
        isPrimary: false,
        pointerType: "touch"
      }),
      false
    );
  });
});
