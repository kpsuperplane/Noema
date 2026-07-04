import type { TranscriptEntry } from "@/shared/types";

export type TranscriptPagePlacement = "latest" | "before" | "append";

export type TranscriptWindowState = {
  durableEntries: TranscriptEntry[];
  optimisticEntries: TranscriptEntry[];
  beforeCursor: string | null;
  hasMoreBefore: boolean;
};

export function emptyTranscriptWindow(): TranscriptWindowState {
  return {
    durableEntries: [],
    optimisticEntries: [],
    beforeCursor: null,
    hasMoreBefore: false
  };
}

export function transcriptWindowEntries(state: TranscriptWindowState): TranscriptEntry[] {
  return [...state.durableEntries, ...state.optimisticEntries];
}

export function appendOptimisticEntry(
  state: TranscriptWindowState,
  entry: TranscriptEntry
): TranscriptWindowState {
  return {
    ...state,
    optimisticEntries: [...state.optimisticEntries, entry]
  };
}

export function replaceOptimisticEntry(
  state: TranscriptWindowState,
  optimisticId: string,
  durableEntry: TranscriptEntry
): TranscriptWindowState {
  const optimisticIndex = state.optimisticEntries.findIndex((entry) => entry.id === optimisticId);
  const replacement = { ...durableEntry, id: optimisticId };
  if (optimisticIndex === -1) {
    return mergeDurableEntries(state, [replacement], {
      beforeCursor: state.beforeCursor,
      hasMoreBefore: state.hasMoreBefore,
      placement: "append"
    });
  }

  const nextOptimisticEntries = state.optimisticEntries.filter((_, index) => index !== optimisticIndex);
  return {
    ...state,
    durableEntries: mergeEntriesByItemId(state.durableEntries, [replacement], "append"),
    optimisticEntries: removeOptimisticEntriesWithDurableItemIds(nextOptimisticEntries, [replacement])
  };
}

export function mergeDurableEntries(
  state: TranscriptWindowState,
  entries: TranscriptEntry[],
  page: {
    beforeCursor: string | null;
    hasMoreBefore: boolean;
    placement: TranscriptPagePlacement;
  }
): TranscriptWindowState {
  const durableEntries = mergeEntriesByItemId(state.durableEntries, entries, page.placement);
  return {
    ...state,
    durableEntries,
    optimisticEntries: removeOptimisticEntriesWithDurableItemIds(state.optimisticEntries, durableEntries),
    beforeCursor: page.beforeCursor,
    hasMoreBefore: page.hasMoreBefore
  };
}

function mergeEntriesByItemId(
  current: TranscriptEntry[],
  incoming: TranscriptEntry[],
  placement: TranscriptPagePlacement
): TranscriptEntry[] {
  const baseEntries = placement === "append" ? [...current, ...incoming] : [...incoming, ...current];
  const incomingByItemId = new Map(
    incoming.flatMap((entry) => {
      const itemId = transcriptEntryItemId(entry);
      return itemId === undefined ? [] : [[itemId, entry]];
    })
  );
  const incomingAssistantStreamIds = new Set(
    incoming.flatMap((entry) => (entry.type === "assistant" && entry.streamId ? [entry.streamId] : []))
  );
  const seenItemIds = new Set<string>();

  return baseEntries.flatMap((entry) => {
    if (entry.type === "assistant_stream" && incomingAssistantStreamIds.has(entry.streamId)) {
      return [];
    }
    const itemId = transcriptEntryItemId(entry);
    if (itemId === undefined) {
      return [entry];
    }
    if (seenItemIds.has(itemId)) {
      return [];
    }
    seenItemIds.add(itemId);
    return [incomingByItemId.get(itemId) ?? entry];
  });
}

function removeOptimisticEntriesWithDurableItemIds(
  optimisticEntries: TranscriptEntry[],
  durableEntries: TranscriptEntry[]
): TranscriptEntry[] {
  const durableItemIds = new Set(
    durableEntries.map(transcriptEntryItemId).filter((itemId) => itemId !== undefined)
  );
  return optimisticEntries.filter((entry) => {
    const itemId = transcriptEntryItemId(entry);
    return itemId === undefined || !durableItemIds.has(itemId);
  });
}

function transcriptEntryItemId(entry: TranscriptEntry): string | undefined {
  return "itemId" in entry ? entry.itemId : undefined;
}
