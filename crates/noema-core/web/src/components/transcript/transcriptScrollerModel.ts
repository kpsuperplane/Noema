export type TranscriptVirtualItem = {
  index: number;
  end: number;
};

export function shouldLoadBeforeFromVirtualItems({
  virtualItems,
  hasMoreBefore,
  loadingBefore,
  oldestEntryKey,
  requestedOldestKey,
  nearTopLoadArmed,
  userScrolledTowardStart
}: {
  virtualItems: readonly TranscriptVirtualItem[];
  hasMoreBefore: boolean;
  loadingBefore: boolean;
  oldestEntryKey: string | null;
  requestedOldestKey: string | null;
  nearTopLoadArmed: boolean;
  userScrolledTowardStart: boolean;
}) {
  const first = virtualItems[0];
  if (!first || !hasMoreBefore || loadingBefore || !oldestEntryKey || !userScrolledTowardStart) {
    return false;
  }
  if (first.index > 3) {
    return false;
  }
  if (!nearTopLoadArmed) {
    return false;
  }
  return requestedOldestKey !== oldestEntryKey;
}
