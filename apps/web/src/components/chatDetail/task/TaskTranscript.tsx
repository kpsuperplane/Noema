import * as React from "react";
import { useSubscription } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { TasksTaskRuntimeEventsDocument } from "@/generated/graphql";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { Transcript } from "@/components/Transcript";
import type { TranscriptEntry } from "@/shared/types";
import {
  TaskRunTranscriptSource,
  type TaskRunLatestEntryChange,
  type TaskRunTranscriptSnapshot
} from "./TaskRunTranscript";
import {
  runDurationLabel,
  useTaskRunClock,
} from "./TaskRevisionTimeline";
import type { TaskDetail, TaskRevision, TaskRun, TaskRunItem } from "./taskTypes";

type TaskRunTimelineEntry = { revision: TaskRevision; run: TaskRun };

type TranscriptEvent =
  | { kind: "message"; id: string; occurredAt: string; entry: TranscriptEntry }
  | { kind: "run"; id: string; occurredAt?: string | null; revision: TaskRevision; run: TaskRun };

type TaskTranscriptSourceValue = {
  refreshEvent: { runId: string; sequence: number } | null;
  runId: string | null;
  snapshot: TaskRunTranscriptSnapshot | null;
};

const TaskTranscriptSourceContext = React.createContext<TaskTranscriptSourceValue>({
  refreshEvent: null,
  runId: null,
  snapshot: null
});

export function TaskTranscriptSourceProvider({
  children,
  liveItems,
  onLatestRunEntryChange,
  run,
  taskId
}: {
  children: React.ReactNode;
  liveItems?: readonly TaskRunItem[];
  onLatestRunEntryChange?: TaskRunLatestEntryChange;
  run: TaskRun | null;
  taskId: string;
}) {
  const [refreshEvent, setRefreshEvent] = React.useState<{
    runId: string;
    sequence: number;
  } | null>(null);
  useSubscription(TasksTaskRuntimeEventsDocument, {
    variables: { taskId },
    onData: ({ data }) => {
      const runId = data.data?.taskRuntimeEvents.runId;
      if (runId) {
        setRefreshEvent((previous) => ({
          runId,
          sequence: (previous?.sequence ?? 0) + 1
        }));
      }
    }
  });
  const [sourceSnapshot, setSourceSnapshot] = React.useState<{
    runId: string;
    snapshot: TaskRunTranscriptSnapshot;
  } | null>(null);
  const onSnapshot = React.useCallback((runId: string, snapshot: TaskRunTranscriptSnapshot) => {
    setSourceSnapshot((previous) => (
      previous?.runId === runId && previous.snapshot === snapshot
        ? previous
        : { runId, snapshot }
    ));
  }, []);
  const runId = run?.id ?? null;
  const snapshot = sourceSnapshot?.runId === runId ? sourceSnapshot.snapshot : null;
  const value = React.useMemo(
    () => ({ refreshEvent, runId, snapshot }),
    [refreshEvent, runId, snapshot]
  );

  return (
    <TaskTranscriptSourceContext.Provider value={value}>
      {run ? (
        <TaskRunTranscriptSource
          key={run.id}
          liveItems={liveItems}
          onSnapshot={onSnapshot}
          onLatestRunEntryChange={onLatestRunEntryChange}
          refreshEvent={refreshEvent}
          run={run}
        />
      ) : null}
      {children}
    </TaskTranscriptSourceContext.Provider>
  );
}

export function TaskTranscript({
  detail,
  liveRunItems,
  onOpenDetail,
  onLatestRunEntryChange
}: {
  detail: TaskDetail;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  onOpenDetail: (target: ChatDetailTarget) => void;
  onLatestRunEntryChange?: TaskRunLatestEntryChange;
}) {
  const runs = React.useMemo(() => taskRunsInOrder(detail.revisions), [detail.revisions]);
  const sharedSource = React.useContext(TaskTranscriptSourceContext);
  const [snapshots, setSnapshots] = React.useState<ReadonlyMap<string, TaskRunTranscriptSnapshot>>(
    () => new Map()
  );
  const resolvedSnapshots = React.useMemo(() => {
    if (!sharedSource.runId || !sharedSource.snapshot) return snapshots;
    const next = new Map(snapshots);
    next.set(sharedSource.runId, sharedSource.snapshot);
    return next;
  }, [sharedSource.runId, sharedSource.snapshot, snapshots]);
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(() => new Set());
  const now = useTaskRunClock(runs.some(({ run }) => run.status === "running"));
  const toggleActivity = React.useCallback((id: string) => {
    setExpandedActivities((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
  const onSnapshot = React.useCallback((runId: string, snapshot: TaskRunTranscriptSnapshot) => {
    setSnapshots((previous) => {
      const current = previous.get(runId);
      if (current === snapshot) {
        return previous;
      }
      const next = new Map(previous);
      next.set(runId, snapshot);
      return next;
    });
  }, []);

  const events = React.useMemo(() => {
    const messages = detail.messages ?? [];
    const requestAlreadyShown = messages.some((message) => message.body.trim() === detail.capturedRequest.trim());
    const next: TranscriptEvent[] = requestAlreadyShown
      ? messages.map((message) => ({
        kind: "message" as const,
        id: message.id,
        occurredAt: message.createdAt,
        entry: { id: message.id, source: "replay" as const, type: "user" as const, text: message.body }
      }))
      : [
        {
          kind: "message" as const,
          id: `task-request:${detail.taskId}`,
          occurredAt: detail.createdAt ?? "",
          entry: {
            id: `task-request:${detail.taskId}`,
            source: "replay" as const,
            type: "user" as const,
            text: detail.capturedRequest
          }
        },
        ...messages.map((message) => ({
          kind: "message" as const,
          id: message.id,
          occurredAt: message.createdAt,
          entry: { id: message.id, source: "replay" as const, type: "user" as const, text: message.body }
        }))
      ];

    for (const { revision, run } of runs) {
      next.push({
        kind: "run",
        id: run.id,
        occurredAt: run.createdAt,
        revision,
        run
      });
    }

    return next.sort((left, right) => parseTimestamp(left.occurredAt) - parseTimestamp(right.occurredAt) || left.id.localeCompare(right.id));
  }, [detail.capturedRequest, detail.createdAt, detail.messages, detail.taskId, runs]);

  const entries = React.useMemo(() => {
    const next: TranscriptEntry[] = [];
    for (const event of events) {
      if (event.kind === "message") {
        next.push(event.entry);
        continue;
      }
      const snapshot = resolvedSnapshots.get(event.run.id);
      const boundaries = runBoundaryEntries(event.run, now);
      next.push(...boundaries.slice(0, 1));
      if (snapshot?.error && snapshot.entries.length === 0) {
        next.push({ id: `${event.run.id}:error`, source: "replay", type: "error", message: snapshot.error, recoverable: true });
      } else if (snapshot) {
        next.push(...snapshot.entries);
      }
      next.push(...boundaries.slice(1));
    }
    return next;
  }, [events, now, resolvedSnapshots]);

  const hasMoreBefore = runs.some((entry) => resolvedSnapshots.get(entry.run.id)?.pageInfo?.hasNextPage);
  const loadOlder = React.useCallback(() => {
    const oldestAvailable = runs.find((entry) => resolvedSnapshots.get(entry.run.id)?.pageInfo?.hasNextPage);
    if (oldestAvailable) {
      resolvedSnapshots.get(oldestAvailable.run.id)?.loadOlder();
    }
  }, [resolvedSnapshots, runs]);
  const loadingOlder = runs.some((entry) => resolvedSnapshots.get(entry.run.id)?.loadingOlder);
  const olderPageError = runs.map((entry) => resolvedSnapshots.get(entry.run.id)?.olderPageError).find(Boolean) ?? null;

  return (
    <div data-slot="task-transcript" {...stylex.props(styles.root)}>
      {runs.filter((run) => run.run.id !== sharedSource.runId).map((run) => (
        <TaskRunTranscriptSource
          key={run.run.id}
          liveItems={liveRunItems?.get(run.run.id)}
          onSnapshot={onSnapshot}
          onLatestRunEntryChange={onLatestRunEntryChange}
          refreshEvent={sharedSource.refreshEvent}
          run={run.run}
        />
      ))}
      <Transcript
        ariaLabel="Task transcript"
        agentStatus="IDLE"
        awaitingAssistantTurn={false}
        density="embedded"
        entries={entries}
        expandedActivities={expandedActivities}
        hasMoreTranscriptBefore={hasMoreBefore}
        loadingOlderTranscript={loadingOlder}
        olderTranscriptPageError={olderPageError}
        onLoadOlderTranscript={loadOlder}
        onOpenDetail={onOpenDetail}
        onSubmitMultipleChoiceSelection={() => undefined}
        onToggleActivity={toggleActivity}
        pending={false}
        sentMessageScrollRequest={0}
        showActorAvatars={false}
        showTypingIndicator={runs.some(({ run }) => run.status === "running" || run.status === "leased")}
        collapseConsecutiveToolCalls
      />
    </div>
  );
}

function taskRunsInOrder(revisions: readonly TaskRevision[]): TaskRunTimelineEntry[] {
  return revisions
    .flatMap((revision) => [
      ...revision.executors.map((run) => ({ revision, run })),
      ...revision.reviewers.map((run) => ({ revision, run }))
    ])
    .sort((left, right) => parseTimestamp(left.run.createdAt) - parseTimestamp(right.run.createdAt) || left.revision.revision - right.revision.revision || left.run.attemptIndex - right.run.attemptIndex);
}

function runBoundaryEntries(run: TaskRun, now: number): TranscriptEntry[] {
  const role = runRoleLabel(run);
  const start = runBoundaryEntry(
    `run-start:${run.id}`,
    `run-start:${run.id}`,
    "task_run_start",
    run,
    role,
    { presentation: { tone: "neutral" }, instance_name: run.instanceName }
  );
  if (!isTerminalRun(run)) {
    return [start];
  }
  if (run.status === "completed") {
    return [start];
  }

  const outcome = runStatusLabel(run);
  const duration = runDurationLabel(run, now);
  const durationSuffix = duration === "0s" ? "" : ` · ${duration}`;
  return [
    start,
    runBoundaryEntry(
      `run-end:${run.id}`,
      `run-end:${run.id}`,
      "task_run_end",
      run,
      `${role} · ${outcome}${durationSuffix}`,
      { presentation: { tone: "error" }, instance_name: run.instanceName }
    )
  ];
}

function runStatusLabel(run: TaskRun): string {
  switch (run.status) {
    case "queued":
      return "Queued";
    case "leased":
    case "running":
      return "Running";
    case "waiting_for_approval":
      return "Waiting for approval";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    case "cancelled":
      return "Cancelled";
    case "interrupted":
      return "Interrupted";
    default:
      return "Finished";
  }
}

function runBoundaryEntry(
  id: string,
  itemId: string,
  activityKind: "task_run_start" | "task_run_end",
  run: TaskRun,
  message: string,
  metadata: Record<string, unknown>
): TranscriptEntry {
  return {
    id,
    source: "replay",
    type: "activity",
    item: {
      kind: "activity",
      id: itemId,
      activity_kind: activityKind,
      status: runActivityStatus(run),
      title: message,
      summary: null,
      metadata
    }
  };
}

function runRoleLabel(run: TaskRun): string {
  switch (run.role) {
    case "planner":
      return "Planner";
    case "executor":
      return "Executor";
    case "reviewer":
      return "Reviewer";
  }
}

function isTerminalRun(run: TaskRun): boolean {
  return run.status === "completed" || run.status === "failed" || run.status === "cancelled" || run.status === "interrupted";
}

function runActivityStatus(run: TaskRun): "STARTED" | "COMPLETED" | "FAILED" {
  if (run.status === "completed") return "COMPLETED";
  if (run.status === "failed" || run.status === "cancelled" || run.status === "interrupted") return "FAILED";
  return "STARTED";
}

function parseTimestamp(value?: string | null): number {
  if (!value) return Number.MAX_SAFE_INTEGER;
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? Number.MAX_SAFE_INTEGER : timestamp;
}

const styles = stylex.create({
  root: {
    display: "contents"
  }
});
