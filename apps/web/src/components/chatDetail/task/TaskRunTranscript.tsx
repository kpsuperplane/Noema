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

export type TaskRunTranscriptSnapshot = {
  entries: ReturnType<typeof taskRunItemsToTranscriptEntries>;
  error: string | null;
  pageInfo: { hasNextPage: boolean } | null;
  loadingOlder: boolean;
  olderPageError: string | null;
  loadOlder: () => void;
};

export function TaskRunTranscriptSource({
  run,
  liveItems = [],
  onSnapshot
}: {
  run: TaskRun;
  liveItems?: readonly TaskRunItem[];
  onSnapshot: (snapshot: TaskRunTranscriptSnapshot) => void;
}) {
  const data = useTaskRunTranscriptData(run, liveItems);

  React.useEffect(() => {
    onSnapshot({
      entries: data.entries,
      error: data.error ? "Agent transcript could not be loaded." : null,
      loadOlder: data.loadOlder,
      loadingOlder: data.loadingOlder,
      olderPageError: data.olderPageError,
      pageInfo: data.pageInfo ? { hasNextPage: data.pageInfo.hasNextPage } : null
    });
  }, [data, onSnapshot]);

  return null;
}

function useTaskRunTranscriptData(run: TaskRun, liveItems: readonly TaskRunItem[]) {
  const runId = run.id;
  const role = run.role;
  const { data, error, fetchMore } = useQuery(WorkTaskRunItemsDocument, {
    fetchPolicy: "cache-and-network",
    variables: { runId, first: 50 }
  });
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
  const entries = React.useMemo(() => taskRunItemsToTranscriptEntries(items), [items]);
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
    entries,
    error: error ? "Agent transcript could not be loaded." : null,
    loadingOlder,
    olderPageError,
    pageInfo,
    loadOlder,
  }), [entries, error, loadingOlder, olderPageError, pageInfo, loadOlder]);
}

function mapNode(node: RunItemNode, role: TaskRunRole): TaskRunItem {
  return mapTaskRunItem(node, role);
}
