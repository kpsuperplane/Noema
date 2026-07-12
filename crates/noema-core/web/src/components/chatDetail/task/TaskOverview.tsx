import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Ban } from "lucide-react";
import { TaskStaticSection } from "./TaskSection";
import type { TaskDetail } from "./taskTypes";
import { TaskResumeControls } from "./TaskResumeControls";
import { TaskStatusBadge } from "./TaskStatusBadge";

export function TaskStatusSummary({
  detail,
  actionBusy,
  actionError,
  onCancel,
  onResume
}: {
  detail: TaskDetail;
  actionBusy?: "cancel" | "resume" | null;
  actionError?: string | null;
  onCancel?: (taskId: string) => void | Promise<void>;
  onResume?: (message?: string) => void | Promise<void>;
}) {
  const canCancel = Boolean(detail.canCancel ?? cancellableTaskStatus(detail.status));

  return (
    <section aria-label="Task status" {...stylex.props(styles.statusSection)}>
      <div {...stylex.props(styles.statusLine)}>
        <TaskStatusBadge status={detail.status} />
        <span {...stylex.props(styles.stage)}>{stageLabel(detail.status)}</span>
      </div>
      {canCancel && onCancel ? (
        <div {...stylex.props(styles.actions)}>
          <Button
            clickAction={() => onCancel(detail.taskId)}
            icon={<Ban aria-hidden="true" size={14} />}
            isDisabled={actionBusy === "resume"}
            isLoading={actionBusy === "cancel"}
            label="Cancel"
            size="sm"
            variant="ghost"
          />
        </div>
      ) : null}
      {actionError ? (
        <p role="alert" {...stylex.props(styles.actionError)}>
          {actionError}
        </p>
      ) : null}
      {onResume ? (
        <TaskResumeControls
          busy={actionBusy === "resume"}
          detail={detail}
          onResume={onResume}
        />
      ) : null}
    </section>
  );
}

export function TaskDetails({ detail }: { detail: TaskDetail }) {
  const revision = detail.currentRevision ?? latestRevision(detail);
  const stage = stageLabel(detail.status);
  const provenance = [detail.createdBy, detail.sourceLabel].filter(Boolean).join(" · ");

  return (
    <TaskStaticSection id="task-details-title" title="Task details">
      <dl {...stylex.props(styles.metadata)}>
        <MetadataRow label="Complexity" value={capitalize(detail.complexity)} />
        <MetadataRow label="Current stage" value={stage} />
        {revision > 0 ? <MetadataRow label="Revision" value={`${revision}`} /> : null}
        {detail.maxReviewRounds ? (
          <MetadataRow label="Review limit" value={`${detail.maxReviewRounds} rounds`} />
        ) : null}
        {detail.createdAt ? <MetadataRow label="Created" value={formatDate(detail.createdAt)} /> : null}
        {provenance ? <MetadataRow label="Created from" value={provenance} /> : null}
      </dl>
    </TaskStaticSection>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div {...stylex.props(styles.metadataRow)}>
      <dt {...stylex.props(styles.metadataLabel)}>{label}</dt>
      <dd {...stylex.props(styles.metadataValue)}>{value}</dd>
    </div>
  );
}

export function cancellableTaskStatus(status: TaskDetail["status"]): boolean {
  return status === "queued" || status === "executing" || status === "reviewing" || status === "revision_requested";
}

function latestRevision(detail: TaskDetail): number {
  return detail.revisions.reduce((latest, revision) => Math.max(latest, revision.revision), 0);
}

function stageLabel(status: TaskDetail["status"]): string {
  switch (status) {
    case "queued":
      return "Waiting for an executor";
    case "executing":
      return "Executor is working";
    case "reviewing":
      return "Reviewing the latest submission";
    case "revision_requested":
      return "Executor is addressing review feedback";
    case "waiting_for_human":
      return "Needs a human decision";
    case "completed":
      return "Approved and delivered";
    case "failed":
      return "Stopped with an error";
    case "cancelled":
      return "Work was cancelled";
  }
}

function formatDate(value: string): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

const styles = stylex.create({
  statusSection: {
    display: "grid",
    gap: 5,
    minWidth: 0,
    paddingBlock: 4,
    paddingInline: 8
  },
  statusLine: {
    display: "inline-flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 8
  },
  stage: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.35
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "end",
    gap: 6
  },
  actionError: {
    margin: 0,
    borderRadius: 7,
    backgroundColor: "color-mix(in srgb, var(--noema-red-100) 55%, transparent)",
    padding: 9,
    color: "var(--noema-red-700)",
    fontSize: 12,
    lineHeight: 1.4
  },
  metadata: {
    display: "grid",
    gap: 8,
    margin: 0,
    paddingTop: 2
  },
  metadataRow: {
    display: "grid",
    gridTemplateColumns: "minmax(92px, 0.42fr) minmax(0, 1fr)",
    gap: 10,
    alignItems: "baseline"
  },
  metadataLabel: {
    color: "var(--noema-text-muted)",
    fontSize: 12
  },
  metadataValue: {
    minWidth: 0,
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    overflowWrap: "anywhere"
  }
});
