import * as stylex from "@stylexjs/stylex";
import { TaskStaticSection } from "./TaskSection";
import type { TaskDetail } from "./taskTypes";

export function TaskDetails({ detail }: { detail: TaskDetail }) {
  const revision = detail.currentRevision ?? latestRevision(detail);
  const stage = taskStageLabel(detail);
  const provenance = [detail.createdBy, detail.sourceLabel].filter(Boolean).join(" · ");

  return (
    <TaskStaticSection id="task-details-title" title="Details">
      <dl {...stylex.props(styles.metadata)}>
        {detail.complexity ? <MetadataRow label="Complexity" value={capitalize(detail.complexity)} /> : null}
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

function latestRevision(detail: TaskDetail): number {
  return detail.revisions.reduce((latest, revision) => Math.max(latest, revision.revision), 0);
}

export function taskStageLabel(detail: TaskDetail): string {
  switch (detail.stageBehavior) {
    case "INTAKE":
      return "Inbox";
    case "DISPATCH":
      return "Queue";
    case "ACTIVE":
      return "Doing";
    case "HUMAN_GATE":
      return "Waiting";
    case "ACCEPTANCE":
      return "Done";
    case "TERMINAL_SUCCESS":
      return "Archive";
    case "TERMINAL_CANCELLED":
      return "Cancelled";
  }
}

export function taskStateHeading(detail: TaskDetail): string {
  switch (detail.stageBehavior) {
    case "INTAKE":
      return "Ready to queue";
    case "DISPATCH":
      return "Queued";
    case "ACTIVE":
      switch (detail.status) {
        case "reviewing": return "Under review";
        case "revision_requested": return "Revising";
        default: return "Working";
      }
    case "HUMAN_GATE":
      return detail.attention?.title ?? "Decision needed";
    case "ACCEPTANCE":
      return "Accept the result";
    case "TERMINAL_SUCCESS":
      return "Archive";
    case "TERMINAL_CANCELLED":
      return "Cancelled";
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
  metadata: {
    display: "grid",
    gap: "var(--spacing-2)",
    margin: 0,
    paddingTop: "var(--spacing-0-5)"
  },
  metadataRow: {
    display: "grid",
    gridTemplateColumns: "minmax(92px, 0.42fr) minmax(0, 1fr)",
    gap: "var(--spacing-3)",
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
