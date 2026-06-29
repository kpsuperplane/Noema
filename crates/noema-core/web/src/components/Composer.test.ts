import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  composerSubmitState,
  composerTextareaProps,
  isComposerTextareaDisabled,
  refocusComposerTextarea
} from "./Composer";

describe("composer disabled states", () => {
  test("keeps textarea and submit available while an agent turn is pending", () => {
    assert.equal(isComposerTextareaDisabled({ ready: true }), false);
    assert.deepEqual(composerSubmitState({ ready: true, value: "Interrupt", pending: true }), {
      disabled: false,
      label: "Sending message"
    });
  });

  test("disables submit only when unavailable or blank", () => {
    assert.equal(composerSubmitState({ ready: false, value: "Hello", pending: false }).disabled, true);
    assert.equal(composerSubmitState({ ready: true, value: "   ", pending: false }).disabled, true);
  });
});

describe("composer submit presentation", () => {
  test("uses accessible labels for the icon-only send button", () => {
    assert.equal(composerSubmitState({ ready: true, value: "Hello", pending: false }).label, "Send message");
    assert.equal(composerSubmitState({ ready: true, value: "Hello", pending: true }).label, "Sending message");
  });
});

describe("composer textarea presentation", () => {
  test("starts as one line with local compact sizing", () => {
    const props = composerTextareaProps();

    assert.equal(props.rows, 1);
    assert.match(props.className, /min-h-9/);
    assert.match(props.className, /max-h-40/);
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
