import { describe, expect, test } from "bun:test";
import type { TranscriptEntry } from "@/shared/types";
import { transcriptScrollKey } from "./scrollModel";

describe("transcriptScrollKey", () => {
  test("fingerprints multiple-choice prompt entries", () => {
    const entries = [
      {
        kind: "entry",
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

    expect(transcriptScrollKey(entries)).toContain("prompt-1");
  });

  test("fingerprints multiple-choice selection entries", () => {
    const entries = [
      {
        kind: "entry",
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

    expect(transcriptScrollKey(entries)).toContain("selection-1");
  });
});
