import * as React from "react";
import { useSubscription } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { WorkTaskRuntimeEventsDocument } from "@/generated/graphql";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
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
import type { TaskArtifact, TaskDetail, TaskRevision, TaskRun, TaskSubmission } from "./taskTypes";

type TaskRunTimelineEntry = { revision: TaskRevision; run: TaskRun };

type TranscriptEvent =
  | { kind: "message"; id: string; occurredAt: string; entry: TranscriptEntry }
  | { kind: "run"; id: string; occurredAt?: string | null; revision: TaskRevision; run: TaskRun };

export function TaskTranscript({
  detail,
  liveRunItems,
  onOpenDetail,
  onLatestRunItemChange
}: {
  detail: TaskDetail;
  liveRunItems?: ReadonlyMap<string, readonly import("./taskTypes").TaskRunItem[]>;
  onOpenDetail: (target: ChatDetailTarget) => void;
  onLatestRunItemChange?: (runId: string, item: import("./taskTypes").TaskRunItem | null) => void;
}) {
  const runs = React.useMemo(() => taskRunsInOrder(detail.revisions), [detail.revisions]);
  const [refreshEvent, setRefreshEvent] = React.useState<{
    runId: string;
    sequence: number;
  } | null>(null);
  useSubscription(WorkTaskRuntimeEventsDocument, {
    variables: { taskId: detail.taskId },
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
      const snapshot = snapshots.get(event.run.id);
      const boundaries = runBoundaryEntries(event.run, now);
      next.push(...boundaries.slice(0, 1));
      if (snapshot?.error && snapshot.entries.length === 0) {
        next.push({ id: `${event.run.id}:error`, source: "replay", type: "error", message: snapshot.error, recoverable: true });
      } else if (snapshot) {
        next.push(...snapshot.entries);
      }
      next.push(...boundaries.slice(1));
      if (event.revision.submission?.executorRunId === event.run.id) {
        next.push(...submissionTranscriptEntries(event.revision.submission));
      }
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
          onSnapshot={onSnapshot}
          onLatestRunItemChange={onLatestRunItemChange}
          refreshEvent={refreshEvent}
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

function submissionTranscriptEntries(submission: TaskSubmission): TranscriptEntry[] {
  const entries: TranscriptEntry[] = [];
  const result = submission.result?.trim() || submission.summary?.trim();
  if (result) {
    entries.push({
      id: `submission-result:${submission.id}`,
      source: "replay",
      type: "assistant",
      debugScope: { kind: "TASK_RUN", scopeId: submission.executorRunId },
      text: result
    });
  }
  entries.push(...(submission.artifacts ?? []).map((artifact) => submissionArtifactEntry(submission.id, artifact)));
  return entries;
}

function submissionArtifactEntry(submissionId: string, artifact: TaskArtifact): TranscriptEntry {
  return {
    id: `submission-artifact:${submissionId}:${artifact.id}`,
    source: "replay",
    type: "artifact",
    item: {
      kind: "artifact_reference",
      artifact_id: artifact.id,
      artifact_version_id: artifact.versionId ?? null,
      title: artifact.title,
      artifact_kind: artifact.kind ?? "artifact",
      storage_kind: artifact.storageKind ?? "local_file",
      external_url: artifact.externalUrl ?? null,
      download_url: artifact.downloadUrl ?? null,
      media_type: artifact.mediaType ?? null
    }
  };
}

function runBoundaryEntries(run: TaskRun, now: number): TranscriptEntry[] {
  const role = runRoleLabel(run);
  const start = runBoundaryEntry(
    `run-start:${run.id}`,
    `run-start:${run.id}`,
    "task_run_start",
    run,
    `${role} · Running`,
    { presentation: { tone: "neutral" }, instance_name: run.instanceName }
  );
  if (!isTerminalRun(run)) {
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
      { presentation: { tone: run.status === "completed" ? "neutral" : "error" }, instance_name: run.instanceName }
    )
  ];
}

function runStatusLabel(run: TaskRun): string {
  switch (run.status) {
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
  const name = `${run.instanceName} · `;
  switch (run.role) {
    case "planner":
      return `${name}Planner`;
    case "executor":
      return `${name}Executor`;
    case "reviewer":
      return `${name}Reviewer`;
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
