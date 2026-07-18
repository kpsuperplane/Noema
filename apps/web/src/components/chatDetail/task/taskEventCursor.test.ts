import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { recordTaskEventCursor, taskEventCursor } from "./taskEventCursor";

describe("taskEventCursor", () => {
  test("starts without a cursor for a task without a received event", () => {
    assert.equal(taskEventCursor("task_cursor_initial"), undefined);
  });

  test("resumes each task after its last received durable event", () => {
    recordTaskEventCursor("task_cursor_one", "d29yazox");
    recordTaskEventCursor("task_cursor_two", "d29yazoy");

    assert.equal(taskEventCursor("task_cursor_one"), "d29yazox");
    assert.equal(taskEventCursor("task_cursor_two"), "d29yazoy");

    recordTaskEventCursor("task_cursor_one", "d29yazo0Mg");
    assert.equal(taskEventCursor("task_cursor_one"), "d29yazo0Mg");
  });
});
