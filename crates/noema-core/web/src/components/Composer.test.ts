import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { composerSubmitState, isComposerTextareaDisabled, refocusComposerTextarea } from "./Composer";

describe("composer disabled states", () => {
  test("keeps textarea and submit available while an agent turn is pending", () => {
    assert.equal(isComposerTextareaDisabled({ ready: true }), false);
    assert.deepEqual(composerSubmitState({ ready: true, value: "Interrupt", pending: true }), {
      disabled: false,
      label: "Sending"
    });
  });

  test("disables submit only when unavailable or blank", () => {
    assert.equal(composerSubmitState({ ready: false, value: "Hello", pending: false }).disabled, true);
    assert.equal(composerSubmitState({ ready: true, value: "   ", pending: false }).disabled, true);
  });
});

describe("refocusComposerTextarea", () => {
  test("schedules focus after submit handling", () => {
    let scheduled = false;
    let focused = false;

    refocusComposerTextarea(
      { focus: () => (focused = true) },
      (callback) => {
        scheduled = true;
        assert.equal(focused, false);
        callback();
      }
    );

    assert.equal(scheduled, true);
    assert.equal(focused, true);
  });
});
