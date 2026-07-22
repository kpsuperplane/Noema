import { useQuery } from "@apollo/client/react";
import * as React from "react";
import { WorkTaskRunItemsDocument, type WorkTaskRunItemsQuery } from "@/generated/graphql";
import {
  mapTaskRunItem,
  mergeTaskRunItems,
  taskRunItemsToTranscriptEntries
} from "./taskRunItemMapper";
import type { TaskRunItem, TaskRunRole } from "./taskTypes";
import type { TaskRun } from "./taskTypes";

type RunItemNode = WorkTaskRunItemsQuery["taskRunItems"]["edges"][number]["node"];
const EMPTY_RUN_ITEMS: readonly TaskRunItem[] = [];

export type TaskRunTranscriptSnapshot = {
  entries: ReturnType<typeof taskRunItemsToTranscriptEntries>;
  latestItem: TaskRunItem | null;
  error: string | null;
  pageInfo: { hasNextPage: boolean } | null;
  loadingOlder: boolean;
  olderPageError: string | null;
  loadOlder: () => void;
};

export function TaskRunTranscriptSource({
  run,
  liveItems = EMPTY_RUN_ITEMS,
  onSnapshot,
  onLatestRunItemChange,
  refreshEvent
}: {
  run: TaskRun;
  liveItems?: readonly TaskRunItem[];
  onSnapshot: (runId: string, snapshot: TaskRunTranscriptSnapshot) => void;
  onLatestRunItemChange?: (runId: string, item: TaskRunItem | null) => void;
  refreshEvent?: { runId: string; sequence: number } | null;
}) {
  const data = useTaskRunTranscriptData(run, liveItems, refreshEvent);
  const publishedItemIdsRef = React.useRef<ReadonlySet<string> | null>(null);
  const arrivalBaselineRef = React.useRef<ReadonlySet<string> | null>(null);
  const arrivalItemIdsRef = React.useRef<ReadonlySet<string>>(new Set());
  const handledRefreshSequenceRef = React.useRef<number | null>(null);

  React.useEffect(() => {
    if (
      refreshEvent?.runId === run.id &&
      refreshEvent.sequence !== handledRefreshSequenceRef.current
    ) {
      handledRefreshSequenceRef.current = refreshEvent.sequence;
      arrivalBaselineRef.current ??= publishedItemIdsRef.current;
    }

    const baseline = arrivalBaselineRef.current;
    if (baseline) {
      const arrivals = data.arrivalCandidates.filter((item) => !baseline.has(item.id));
      if (arrivals.length > 0) {
        arrivalItemIdsRef.current = new Set([
          ...arrivalItemIdsRef.current,
          ...arrivals.map((item) => item.id)
        ]);
        arrivalBaselineRef.current = null;
      }
    }

    onSnapshot(run.id, {
      entries: taskRunItemsToTranscriptEntries(data.items, arrivalItemIdsRef.current),
      error: data.error ? "Agent transcript could not be loaded." : null,
      latestItem: data.latestItem,
      loadOlder: data.loadOlder,
      loadingOlder: data.loadingOlder,
      olderPageError: data.olderPageError,
      pageInfo: data.pageInfo ? { hasNextPage: data.pageInfo.hasNextPage } : null
    });
    onLatestRunItemChange?.(run.id, data.latestItem);
    publishedItemIdsRef.current = new Set(data.items.map((item) => item.id));
  }, [data, onLatestRunItemChange, onSnapshot, refreshEvent, run.id]);

  return null;
}

function useTaskRunTranscriptData(
  run: TaskRun,
  liveItems: readonly TaskRunItem[],
  refreshEvent: { runId: string; sequence: number } | null | undefined
) {
  const runId = run.id;
  const role = run.role;
  const { data, error, fetchMore, refetch } = useQuery(WorkTaskRunItemsDocument, {
    fetchPolicy: "cache-and-network",
    variables: { runId, first: 50 }
  });
  const refetchInFlight = React.useRef(false);
  const refetchPending = React.useRef(false);
  const refresh = React.useCallback(() => {
    const run = () => {
      if (refetchInFlight.current) {
        refetchPending.current = true;
        return;
      }
      refetchInFlight.current = true;
      void refetch().finally(() => {
        refetchInFlight.current = false;
        if (refetchPending.current) {
          refetchPending.current = false;
          run();
        }
      });
    };
    run();
  }, [refetch]);
  React.useEffect(() => {
    if (refreshEvent?.runId === runId) refresh();
  }, [refresh, refreshEvent, runId]);
  const [olderItems, setOlderItems] = React.useState<readonly TaskRunItem[]>([]);
  const [pageInfoOverride, setPageInfoOverride] = React.useState<{
    endCursor: string | null;
    hasNextPage: boolean;
  } | null>(null);
  const [loadingOlder, setLoadingOlder] = React.useState(false);
  const [olderPageError, setOlderPageError] = React.useState<string | null>(null);
  const pageInfo = pageInfoOverride ?? data?.taskRunItems.pageInfo ?? null;
  const currentItems = React.useMemo(
    () => data?.taskRunItems.edges.map((edge) => mapNode(edge.node, role)) ?? [],
    [data?.taskRunItems.edges, role]
  );
  const items = React.useMemo(
    () => mergeTaskRunItems(olderItems, currentItems, liveItems),
    [currentItems, liveItems, olderItems]
  );
  const arrivalCandidates = React.useMemo(
    () => mergeTaskRunItems(currentItems, liveItems),
    [currentItems, liveItems]
  );
  const loadOlder = React.useCallback(async () => {
    if (!pageInfo?.hasNextPage || loadingOlder) return;
    setLoadingOlder(true);
    setOlderPageError(null);
    try {
      const result = await fetchMore({
        variables: { runId, after: pageInfo.endCursor, first: 50 }
      });
      const next = result.data?.taskRunItems;
      if (!next) return;
      setOlderItems((previous) =>
        mergeTaskRunItems(next.edges.map((edge) => mapNode(edge.node, role)), currentItems, previous)
      );
      setPageInfoOverride(next.pageInfo);
    } catch (caught) {
      setOlderPageError(
        caught instanceof Error ? caught.message : "Older transcript items could not be loaded."
      );
    } finally {
      setLoadingOlder(false);
    }
  }, [currentItems, fetchMore, loadingOlder, pageInfo, role, runId]);

  return React.useMemo(() => ({
    arrivalCandidates,
    error: error ? "Agent transcript could not be loaded." : null,
    items,
    latestItem: items.at(-1) ?? null,
    loadingOlder,
    olderPageError,
    pageInfo,
    loadOlder,
  }), [arrivalCandidates, error, items, loadingOlder, olderPageError, pageInfo, loadOlder]);
}

function mapNode(node: RunItemNode, role: TaskRunRole): TaskRunItem {
  return mapTaskRunItem(node, role);
}
