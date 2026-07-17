import { describe, test } from "node:test";
import assert from "node:assert/strict";
import type { TranscriptEntry } from "@/shared/types";
import type { RenderTranscriptEntry } from "./renderModel";
import { transcriptScrollKey } from "./scrollModel";

describe("transcriptScrollKey", () => {
  test("fingerprints multiple-choice prompt entries", () => {
    const entries: RenderTranscriptEntry[] = [
      {
        kind: "entry",
        id: "prompt-1",
        entry: {
          id: "prompt-1",
          itemId: "prompt-1",
          type: "multiple_choice_prompt",
          item: {
            kind: "multiple_choice_prompt",
            prompt: "Pick one test fruit.",
            selection_mode: "PICK_ONE",
            options: [
              { id: "apple", label: "Apple" },
              { id: "banana", label: "Banana" }
            ]
          }
        } satisfies TranscriptEntry
      }
    ];

    assert.equal(transcriptScrollKey(entries).includes("prompt-1"), true);
  });

  test("fingerprints multiple-choice selection entries", () => {
    const entries: RenderTranscriptEntry[] = [
      {
        kind: "entry",
        id: "selection-1",
        entry: {
          id: "selection-1",
          itemId: "selection-1",
          type: "multiple_choice_selection",
          item: {
            kind: "multiple_choice_selection",
            prompt_item_id: "prompt-1",
            selection_mode: "PICK_MANY",
            selected_options: [
              { id: "apple", label: "Apple" },
              { id: "banana", label: "Banana" }
            ]
          }
        } satisfies TranscriptEntry
      }
    ];

    assert.equal(transcriptScrollKey(entries).includes("selection-1"), true);
  });
});
