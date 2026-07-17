import { describe, test } from "node:test";
import assert from "node:assert/strict";
import {
  transcriptBottomAnchorOffset,
  shouldLoadBeforeFromVirtualItems,
  type TranscriptVirtualItem
} from "./transcriptScrollerModel";

describe("shouldLoadBeforeFromVirtualItems", () => {
  test("does not load older messages on the initial bottom-anchored layout", () => {
    const virtualItems: TranscriptVirtualItem[] = [
      { index: 0, end: 96 },
      { index: 1, end: 192 },
      { index: 2, end: 288 }
    ];

    assert.equal(
      shouldLoadBeforeFromVirtualItems({
        virtualItems,
        hasMoreBefore: true,
        loadingBefore: false,
        oldestEntryKey: "item:oldest",
        requestedOldestKey: null,
        nearTopLoadArmed: true,
        userScrolledTowardStart: false
      }),
      false
    );
  });

  test("loads older messages when the user reaches the top after scrolling", () => {
    const virtualItems: TranscriptVirtualItem[] = [
      { index: 0, end: 96 },
      { index: 1, end: 192 },
      { index: 2, end: 288 }
    ];

    assert.equal(
      shouldLoadBeforeFromVirtualItems({
        virtualItems,
        hasMoreBefore: true,
        loadingBefore: false,
        oldestEntryKey: "item:oldest",
        requestedOldestKey: null,
        nearTopLoadArmed: true,
        userScrolledTowardStart: true
      }),
      true
    );
  });

  test("does not load older messages for scroll intent away from the top", () => {
    assert.equal(
      shouldLoadBeforeFromVirtualItems({
        virtualItems: [{ index: 0, end: 96 }],
        hasMoreBefore: true,
        loadingBefore: false,
        oldestEntryKey: "item:oldest",
        requestedOldestKey: null,
        nearTopLoadArmed: true,
        userScrolledTowardStart: false
      }),
      false
    );
  });
});

describe("transcriptBottomAnchorOffset", () => {
  test("offsets short transcript rows to the bottom of the usable viewport", () => {
    assert.equal(
      transcriptBottomAnchorOffset({
        availableHeight: 640,
        totalSize: 320
      }),
      320
    );
  });

  test("does not offset transcript rows once they exceed the usable viewport", () => {
    assert.equal(
      transcriptBottomAnchorOffset({
        availableHeight: 320,
        totalSize: 640
      }),
      0
    );
  });
});
