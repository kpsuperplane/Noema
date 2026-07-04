import type { TranscriptEntry } from "@/shared/types";

export type TranscriptPagePlacement = "latest" | "before" | "append";

export type TranscriptWindowState = {
  durableEntries: TranscriptEntry[];
  optimisticEntries: TranscriptEntry[];
  beforeCursor: string | null;
  hasMoreBefore: boolean;
};

export const emptyTranscriptWindow: TranscriptWindowState = {
  durableEntries: [],
  optimisticEntries: [],
  beforeCursor: null,
  hasMoreBefore: false
};

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
  if (optimisticIndex === -1) {
    return mergeDurableEntries(state, [durableEntry], {
      beforeCursor: state.beforeCursor,
      hasMoreBefore: state.hasMoreBefore,
      placement: "append"
    });
  }

  return {
    ...state,
    optimisticEntries: state.optimisticEntries.map((entry, index) =>
      index === optimisticIndex ? { ...durableEntry, id: optimisticId } : entry
    )
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
  return {
    ...state,
    durableEntries: mergeEntriesByItemId(state.durableEntries, entries, page.placement),
    beforeCursor: page.beforeCursor,
    hasMoreBefore: page.hasMoreBefore
  };
}

function mergeEntriesByItemId(
  current: TranscriptEntry[],
  incoming: TranscriptEntry[],
  placement: TranscriptPagePlacement
): TranscriptEntry[] {
  const incomingItemIds = new Set(incoming.map(transcriptEntryItemId).filter((itemId) => itemId !== undefined));
  const currentWithoutIncomingDuplicates = current.filter((entry) => {
    const itemId = transcriptEntryItemId(entry);
    return itemId === undefined || !incomingItemIds.has(itemId);
  });

  return placement === "append"
    ? [...currentWithoutIncomingDuplicates, ...incoming]
    : [...incoming, ...currentWithoutIncomingDuplicates];
}

function transcriptEntryItemId(entry: TranscriptEntry): string | undefined {
  return "itemId" in entry ? entry.itemId : undefined;
}
