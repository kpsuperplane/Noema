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

export function completeAssistantStreams(state: TranscriptWindowState): TranscriptWindowState {
  return {
    ...state,
    durableEntries: state.durableEntries.filter((entry) => entry.type !== "assistant_stream")
  };
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
  const incomingAssistantByStreamId = new Map(
    incoming.flatMap((entry) =>
      entry.type === "assistant"
        ? assistantReconciledStreamIds(entry).map((streamId) => [streamId, entry] as const)
        : []
    )
  );
  const incomingAssistantByTurnResponse = new Map(
    incoming.flatMap((entry) => {
      const key = assistantTurnResponseKey(entry);
      return entry.type === "assistant" && key ? [[key, entry]] : [];
    })
  );
  const incomingAssistantByTurnText = new Map(
    incoming.flatMap((entry) => {
      const key = assistantTurnTextKey(entry);
      return entry.type === "assistant" && key ? [[key, entry]] : [];
    })
  );
  const seenItemIds = new Set<string>();
  const seenAssistantTurnText = new Set<string>();

  return baseEntries.flatMap((entry) => {
    if (entry.type === "assistant_stream") {
      const replacement =
        incomingAssistantByStreamId.get(entry.streamId) ??
        incomingAssistantByTurnResponse.get(assistantTurnResponseKey(entry) ?? "") ??
        incomingAssistantByTurnText.get(assistantTurnTextKey(entry) ?? "");
      const replacementItemId = replacement ? transcriptEntryItemId(replacement) : undefined;
      if (!replacement) {
        return [entry];
      }
      if (replacementItemId) {
        if (seenItemIds.has(replacementItemId)) {
          return [];
        }
        seenItemIds.add(replacementItemId);
      }
      return [replacement];
    }
    const itemId = transcriptEntryItemId(entry);
    if (itemId !== undefined && seenItemIds.has(itemId)) {
      return [];
    }
    if (entry.type === "assistant") {
      const turnTextKey = assistantTurnTextKey(entry);
      if (turnTextKey) {
        const replacement = incomingAssistantByTurnText.get(turnTextKey);
        if (replacement) {
          if (seenAssistantTurnText.has(turnTextKey)) {
            return [];
          }
          seenAssistantTurnText.add(turnTextKey);
          const replacementItemId = transcriptEntryItemId(replacement);
          if (replacementItemId) {
            seenItemIds.add(replacementItemId);
          }
          return [replacement];
        }
        if (seenAssistantTurnText.has(turnTextKey)) {
          return [];
        }
        seenAssistantTurnText.add(turnTextKey);
      }
    }
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
  const durableEntryIds = new Set(durableEntries.map((entry) => entry.id));
  const durableItemIds = new Set(
    durableEntries.map(transcriptEntryItemId).filter((itemId) => itemId !== undefined)
  );
  return optimisticEntries.filter((entry) => {
    const itemId = transcriptEntryItemId(entry);
    return !durableEntryIds.has(entry.id) && (itemId === undefined || !durableItemIds.has(itemId));
  });
}

function transcriptEntryItemId(entry: TranscriptEntry): string | undefined {
  return "itemId" in entry ? entry.itemId : undefined;
}

function assistantTurnResponseKey(entry: TranscriptEntry): string | undefined {
  if ((entry.type !== "assistant" && entry.type !== "assistant_stream") || !entry.turnId) {
    return undefined;
  }
  return typeof entry.responseIndex === "number" ? `${entry.turnId}:${entry.responseIndex}` : undefined;
}

function assistantTurnTextKey(entry: TranscriptEntry): string | undefined {
  if (
    (entry.type !== "assistant" && entry.type !== "assistant_stream") ||
    !entry.turnId ||
    !entry.text.trim()
  ) {
    return undefined;
  }
  return `${entry.turnId}:${entry.text}`;
}

function assistantReconciledStreamIds(
  entry: Extract<TranscriptEntry, { type: "assistant" }>
): string[] {
  const streamIds = entry.streamId ? [entry.streamId] : [];
  if (!isRecord(entry.metadata) || !Array.isArray(entry.metadata.reconciled_stream_ids)) {
    return streamIds;
  }
  for (const streamId of entry.metadata.reconciled_stream_ids) {
    if (typeof streamId === "string" && streamId && !streamIds.includes(streamId)) {
      streamIds.push(streamId);
    }
  }
  return streamIds;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
