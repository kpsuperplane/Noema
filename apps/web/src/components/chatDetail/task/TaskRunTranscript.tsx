import { useQuery } from "@apollo/client/react";
import * as React from "react";
import { WorkTaskRunItemsDocument, type WorkTaskRunItemsQuery } from "@/generated/graphql";
import { Transcript } from "@/components/Transcript";
import { TranscriptSystemNotice } from "@/components/transcript/TranscriptSystemNotice";
import {
  mapTaskRunItem,
  mergeTaskRunItems,
  taskRunItemsToTranscriptEntries
} from "./taskRunItemMapper";
import type { TaskRunItem, TaskRunRole } from "./taskTypes";
import type { WorkTaskRun } from "@/components/work/workTypes";

type RunItemNode = WorkTaskRunItemsQuery["taskRunItems"]["edges"][number]["node"];

export function TaskRunTranscript({
  run,
  liveItems = []
}: {
  run: WorkTaskRun;
  liveItems?: readonly TaskRunItem[];
}) {
  const runId = run.runId;
  const role = runRole(run);
  const { data, error, loading, fetchMore } = useQuery(WorkTaskRunItemsDocument, {
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
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(() => new Set());
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
  const toggleActivity = React.useCallback((id: string) => {
    setExpandedActivities((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
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
    return <TranscriptSystemNotice label="Empty">No transcript items were recorded for this run.</TranscriptSystemNotice>;
  }

  return (
    <Transcript
      ariaLabel={`${roleLabel(role)} agent transcript`}
      agentStatus="IDLE"
      awaitingAssistantTurn={false}
      density="embedded"
      entries={entries}
      expandedActivities={expandedActivities}
      hasMoreTranscriptBefore={Boolean(pageInfo?.hasNextPage)}
      loadingOlderTranscript={loadingOlder}
      olderTranscriptPageError={olderPageError}
      onLoadOlderTranscript={loadOlder}
      onSubmitMultipleChoiceSelection={() => undefined}
      onToggleActivity={toggleActivity}
      pending={false}
      sentMessageScrollRequest={0}
      showActorAvatars={false}
    />
  );
}

function mapNode(node: RunItemNode, role: TaskRunRole): TaskRunItem {
  return mapTaskRunItem(node, role);
}

function runRole(run: WorkTaskRun): TaskRunRole {
  return run.kind.toLowerCase() as TaskRunRole;
}

function roleLabel(role: TaskRunRole): string {
  return role.charAt(0).toUpperCase() + role.slice(1);
}
