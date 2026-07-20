import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { Transcript } from "@/components/Transcript";
import type { TranscriptEntry } from "@/shared/types";
import {
  TaskRunTranscriptSource,
  type TaskRunTranscriptSnapshot
} from "./TaskRunTranscript";
import {
  runDurationLabel,
  useTaskRunClock,
} from "./TaskRevisionTimeline";
import type { TaskDetail, TaskRevision, TaskRun } from "./taskTypes";

type TaskRunTimelineEntry = { revision: TaskRevision; run: TaskRun };

type TranscriptEvent =
  | { kind: "message"; id: string; occurredAt: string; entry: TranscriptEntry }
  | { kind: "run"; id: string; occurredAt?: string | null; revision: TaskRevision; run: TaskRun };

export function TaskTranscript({
  detail,
  liveRunItems
}: {
  detail: TaskDetail;
  liveRunItems?: ReadonlyMap<string, readonly import("./taskTypes").TaskRunItem[]>;
}) {
  const runs = React.useMemo(() => taskRunsInOrder(detail.revisions), [detail.revisions]);
  const [snapshots, setSnapshots] = React.useState<ReadonlyMap<string, TaskRunTranscriptSnapshot>>(
    () => new Map()
  );
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
    const requestAlreadyShown = messages.some((message) => message.body.trim() === detail.request.trim());
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
            text: detail.request
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
  }, [detail.createdAt, detail.messages, detail.request, detail.taskId, runs]);

  const entries = React.useMemo(() => {
    const next: TranscriptEntry[] = [];
    for (const event of events) {
      if (event.kind === "message") {
        next.push(event.entry);
        continue;
      }

      const snapshot = snapshots.get(event.run.id);
      const boundaries = runBoundaryEntries(event.run, event.revision, now);
      next.push(...boundaries.slice(0, 1));
      if (snapshot?.error && snapshot.entries.length === 0) {
        next.push({ id: `${event.run.id}:error`, source: "replay", type: "error", message: snapshot.error, recoverable: true });
      } else if (snapshot) {
        next.push(...snapshot.entries);
      }
      next.push(...boundaries.slice(1));
    }
    return next;
  }, [events, now, snapshots]);

  const hasMoreBefore = runs.some((entry) => snapshots.get(entry.run.id)?.pageInfo?.hasNextPage);
  const loadOlder = React.useCallback(() => {
    const oldestAvailable = runs.find((entry) => snapshots.get(entry.run.id)?.pageInfo?.hasNextPage);
    if (oldestAvailable) {
      snapshots.get(oldestAvailable.run.id)?.loadOlder();
    }
  }, [runs, snapshots]);
  const loadingOlder = runs.some((entry) => snapshots.get(entry.run.id)?.loadingOlder);
  const olderPageError = runs.map((entry) => snapshots.get(entry.run.id)?.olderPageError).find(Boolean) ?? null;

  return (
    <div data-slot="task-transcript" {...stylex.props(styles.root)}>
      {runs.map((run) => (
        <TaskRunTranscriptSource
          key={run.run.id}
          liveItems={liveRunItems?.get(run.run.id)}
          onSnapshot={(snapshot) => onSnapshot(run.run.id, snapshot)}
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
        onSubmitMultipleChoiceSelection={() => undefined}
        onToggleActivity={toggleActivity}
        pending={false}
        sentMessageScrollRequest={0}
        showActorAvatars={false}
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

function runBoundaryEntries(run: TaskRun, revision: TaskRevision, now: number): TranscriptEntry[] {
  const role = runRoleLabel(run);
  const revisionLabel = `R${revision.revision}`;
  const start = runBoundaryEntry(
    `run-start:${run.id}`,
    `run-start:${run.id}`,
    "task_run_start",
    run,
    `${role} spawned and running · ${revisionLabel}`,
    { presentation: { tone: "neutral" } }
  );
  if (!isTerminalRun(run)) {
    return [start];
  }

  const outcome = run.status === "completed"
    ? `${role} completed successfully`
    : run.status === "failed"
      ? `${role} completed with failure`
      : `${role} ${run.status}`;
  const duration = runDurationLabel(run, now);
  const durationSuffix = duration === "0s" ? "" : ` · ${duration}`;
  return [
    start,
    runBoundaryEntry(
      `run-end:${run.id}`,
      `run-end:${run.id}`,
      "task_run_end",
      run,
      `${outcome} · ${revisionLabel}${durationSuffix}`,
      { presentation: { tone: run.status === "completed" ? "neutral" : "error" } }
    )
  ];
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
