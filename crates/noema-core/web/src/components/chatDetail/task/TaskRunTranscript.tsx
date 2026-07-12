import { useQuery } from "@apollo/client/react";
import * as React from "react";
import { TaskRunItemsDocument, type TaskRunItemsQuery } from "@/generated/graphql";
import { Transcript } from "@/components/Transcript";
import { TranscriptSystemNotice } from "@/components/transcript/TranscriptSystemNotice";
import {
  mapTaskRunItem,
  mergeTaskRunItems,
  taskRunItemsToTranscriptEntries
} from "./taskRunItemMapper";
import type { TaskRun, TaskRunItem } from "./taskTypes";

type RunItemNode = TaskRunItemsQuery["taskRunItems"]["items"][number];

export function TaskRunTranscript({
  run,
  liveItems = []
}: {
  run: TaskRun;
  liveItems?: readonly TaskRunItem[];
}) {
  const { data, error, loading, fetchMore } = useQuery(TaskRunItemsDocument, {
    fetchPolicy: "cache-and-network",
    variables: { runId: run.id, first: 50 }
  });
  const [olderItems, setOlderItems] = React.useState<readonly TaskRunItem[]>([]);
  const [pageInfoOverride, setPageInfoOverride] = React.useState<{
    endCursor: string | null;
    hasNextPage: boolean;
  } | null>(null);
  const [loadingOlder, setLoadingOlder] = React.useState(false);
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(() => new Set());
  const pageInfo = pageInfoOverride ?? data?.taskRunItems.pageInfo ?? null;

  const currentItems = React.useMemo(
    () => data?.taskRunItems.items.map((item) => mapNode(item, run)) ?? [],
    [data?.taskRunItems.items, run]
  );
  const items = React.useMemo(
    () => mergeTaskRunItems(olderItems, currentItems, liveItems),
    [currentItems, liveItems, olderItems]
  );
  const entries = React.useMemo(() => taskRunItemsToTranscriptEntries(items), [items]);
  const toggleActivity = React.useCallback((id: string) => {
    setExpandedActivities((previous) => {
      const next = new Set(previous);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);
  const loadOlder = React.useCallback(async () => {
    if (!pageInfo?.hasNextPage || loadingOlder) {
      return;
    }
    setLoadingOlder(true);
    try {
      const result = await fetchMore({
        variables: { runId: run.id, after: pageInfo.endCursor, first: 50 }
      });
      const next = result.data?.taskRunItems;
      if (!next) {
        return;
      }
      setOlderItems((previous) =>
        mergeTaskRunItems(next.items.map((item) => mapNode(item, run)), previous)
      );
      setPageInfoOverride(next.pageInfo);
    } finally {
      setLoadingOlder(false);
    }
  }, [fetchMore, loadingOlder, pageInfo, run]);

  if (loading && entries.length === 0) {
    return <TranscriptSystemNotice label="Loading">Agent transcript...</TranscriptSystemNotice>;
  }
  if (error && entries.length === 0) {
    return (
      <TranscriptSystemNotice role="alert" tone="error" label="Error">
        Agent transcript could not be loaded.
      </TranscriptSystemNotice>
    );
  }
  if (entries.length === 0) {
    return null;
  }

  return (
    <Transcript
      ariaLabel={`${runRoleLabel(run)} agent transcript`}
      agentStatus="IDLE"
      awaitingAssistantTurn={false}
      density="embedded"
      entries={entries}
      expandedActivities={expandedActivities}
      hasMoreTranscriptBefore={Boolean(pageInfo?.hasNextPage)}
      loadingOlderTranscript={loadingOlder}
      olderTranscriptPageError={null}
      onLoadOlderTranscript={loadOlder}
      onSubmitMultipleChoiceSelection={() => undefined}
      onToggleActivity={toggleActivity}
      pending={false}
      sentMessageScrollRequest={0}
    />
  );
}

function mapNode(node: RunItemNode, run: TaskRun): TaskRunItem {
  return mapTaskRunItem(node, run.role);
}

function runRoleLabel(run: TaskRun): string {
  return run.role === "reviewer" ? "Reviewer" : "Executor";
}
