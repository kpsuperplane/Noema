import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { recordTaskEventCursor, taskEventCursor } from "./taskEventCursor";

describe("taskEventCursor", () => {
  test("starts from the durable beginning for a task without a received event", () => {
    assert.equal(taskEventCursor("task_cursor_initial"), "0");
  });

  test("resumes each task after its last received durable event", () => {
    recordTaskEventCursor("task_cursor_one", "41");
    recordTaskEventCursor("task_cursor_two", "9");

    assert.equal(taskEventCursor("task_cursor_one"), "41");
    assert.equal(taskEventCursor("task_cursor_two"), "9");

    recordTaskEventCursor("task_cursor_one", "42");
    assert.equal(taskEventCursor("task_cursor_one"), "42");
  });
});
