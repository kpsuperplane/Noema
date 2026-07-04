import { describe, test } from "node:test";
import assert from "node:assert/strict";
import {
  appendOptimisticEntry,
  mergeDurableEntries,
  replaceOptimisticEntry,
  transcriptWindowEntries,
  type TranscriptWindowState
} from "./window";
import type { TranscriptEntry } from "@/shared/types";

function userEntry(id: string, text: string, cursor?: string): TranscriptEntry {
  return { id, itemId: id, cursor, type: "user", text };
}

describe("transcript window model", () => {
  test("merges pages in server-returned order and dedupes by item id", () => {
    const initial: TranscriptWindowState = {
      durableEntries: [],
      optimisticEntries: [],
      beforeCursor: null,
      hasMoreBefore: true
    };

    const latest = mergeDurableEntries(
      initial,
      [userEntry("item:2", "two", "c2"), userEntry("item:3", "three", "c3")],
      {
        beforeCursor: "c2",
        hasMoreBefore: true,
        placement: "latest"
      }
    );
    const older = mergeDurableEntries(
      latest,
      [userEntry("item:1", "one", "c1"), userEntry("item:2", "two updated", "c2")],
      {
        beforeCursor: "c1",
        hasMoreBefore: false,
        placement: "before"
      }
    );

    assert.deepEqual(
      transcriptWindowEntries(older).map((entry) => ("text" in entry ? entry.text : entry.id)),
      ["one", "two updated", "three"]
    );
    assert.equal(older.beforeCursor, "c1");
    assert.equal(older.hasMoreBefore, false);
  });

  test("replaces optimistic user entry with durable client item", () => {
    const optimistic = appendOptimisticEntry(
      { durableEntries: [], optimisticEntries: [], beforeCursor: null, hasMoreBefore: false },
      { id: "client:1", type: "user", text: "hello" }
    );
    const replaced = replaceOptimisticEntry(optimistic, "client:1", userEntry("item:1", "hello", "c1"));

    assert.deepEqual(
      transcriptWindowEntries(replaced).map((entry) => entry.id),
      ["client:1"]
    );
    const replacedEntry = transcriptWindowEntries(replaced)[0];
    assert.ok(replacedEntry);
    assert.equal(replacedEntry.type, "user");
    assert.equal(replacedEntry.itemId, "item:1");
    assert.deepEqual(
      replaced.durableEntries.map((entry) => entry.id),
      ["client:1"]
    );
    assert.deepEqual(replaced.optimisticEntries, []);
  });

  test("removes optimistic entries when matching durable item is merged", () => {
    const current: TranscriptWindowState = {
      durableEntries: [userEntry("item:1", "one", "c1")],
      optimisticEntries: [{ ...userEntry("item:2", "optimistic two", "c2"), id: "client:2" }],
      beforeCursor: null,
      hasMoreBefore: false
    };
    const merged = mergeDurableEntries(current, [userEntry("item:2", "durable two", "c2")], {
      beforeCursor: null,
      hasMoreBefore: false,
      placement: "append"
    });

    assert.deepEqual(
      transcriptWindowEntries(merged).map((entry) => entry.id),
      ["item:1", "item:2"]
    );
    assert.deepEqual(merged.optimisticEntries, []);
  });

  test("append duplicate durable entries keep base order with incoming value", () => {
    const current: TranscriptWindowState = {
      durableEntries: [userEntry("item:1", "old one", "c1"), userEntry("item:2", "two", "c2")],
      optimisticEntries: [],
      beforeCursor: null,
      hasMoreBefore: false
    };
    const merged = mergeDurableEntries(current, [userEntry("item:1", "new one", "c1"), userEntry("item:3", "three", "c3")], {
      beforeCursor: null,
      hasMoreBefore: false,
      placement: "append"
    });

    assert.deepEqual(
      transcriptWindowEntries(merged).map((entry) => ("text" in entry ? entry.text : entry.id)),
      ["new one", "two", "three"]
    );
  });

  test("keeps live item after latest replay when subscription wins the race", () => {
    const liveFirst = mergeDurableEntries(
      { durableEntries: [], optimisticEntries: [], beforeCursor: null, hasMoreBefore: false },
      [userEntry("item:3", "three", "c3")],
      {
        beforeCursor: null,
        hasMoreBefore: false,
        placement: "append"
      }
    );
    const latestArrivesSecond = mergeDurableEntries(
      liveFirst,
      [userEntry("item:1", "one", "c1"), userEntry("item:2", "two", "c2")],
      {
        beforeCursor: "c1",
        hasMoreBefore: false,
        placement: "latest"
      }
    );

    assert.deepEqual(
      transcriptWindowEntries(latestArrivesSecond).map((entry) => ("text" in entry ? entry.text : entry.id)),
      ["one", "two", "three"]
    );
  });
});
