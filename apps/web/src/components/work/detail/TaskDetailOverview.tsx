import { Markdown } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { AttentionBadge } from "../AttentionBadge";
import { relativeTime, sentenceCase, timestampLabel } from "../workModel";
import { StageBadge } from "../StageBadge";
import type { WorkProject, WorkTaskDetail } from "../workTypes";
import { TaskActions } from "../TaskActions";

export function TaskDetailOverview({ task, projects = [], onUpdated }: { task: WorkTaskDetail; projects?: readonly WorkProject[]; onUpdated: () => void | Promise<void> }) {
  const configUnavailable = task.activeGate?.recoveryReason === "CONFIGURATION_UNAVAILABLE";
  return (
    <section aria-labelledby="task-detail-title" {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.kicker)}><StageBadge name={task.stage.name} behavior={task.stage.behavior} />{task.project ? <span>{task.project.name}</span> : <span>No project</span>}<span>Updated {relativeTime(task.updatedAt)}</span></div>
      <div {...stylex.props(styles.heading)}><div {...stylex.props(styles.headingCopy)}><h1 id="task-detail-title" {...stylex.props(styles.title)}>{task.title}</h1><p {...stylex.props(styles.meta)}>Created {timestampLabel(task.createdAt)} · {sentenceCase(task.source.sourceKind)}</p></div><TaskActions task={task} validActions={task.validActions} projects={projects} onUpdated={onUpdated} /></div>
      {task.description ? <div {...stylex.props(styles.description)}><Markdown density="default" headingLevelStart={2}>{task.description}</Markdown></div> : null}
      <div {...stylex.props(styles.projections)}>
        {task.currentRun ? <div {...stylex.props(styles.projection)}><strong {...stylex.props(styles.projectionLabel)}>Current run</strong><span {...stylex.props(styles.projectionValue)}>{task.currentRun.activityLabel}</span><small {...stylex.props(styles.projectionMeta)}>Attempt {task.currentRun.attemptIndex + 1}</small></div> : <div {...stylex.props(styles.projection)}><strong {...stylex.props(styles.projectionLabel)}>Current run</strong><span {...stylex.props(styles.projectionValue)}>No worker active</span></div>}
        {task.attention ? <div {...stylex.props(styles.projection)}><strong {...stylex.props(styles.projectionLabel)}>Attention</strong><AttentionBadge kind={task.attention.kind} label={task.attention.title} /><span {...stylex.props(styles.projectionValue)}>{task.attention.summary}</span></div> : <div {...stylex.props(styles.projection)}><strong {...stylex.props(styles.projectionLabel)}>Attention</strong><span {...stylex.props(styles.projectionValue)}>Nothing needed</span></div>}
      </div>
      {configUnavailable ? <div role="status" {...stylex.props(styles.configuration)}>Execution configuration is unavailable. <Link to="/settings/agents" {...stylex.props(styles.configurationLink)}>Review Task Executor settings</Link>.</div> : null}
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 16, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 16, backgroundColor: "var(--surface-card)", padding: 20 }, kicker: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8, color: "var(--muted-foreground)", fontSize: 11 }, heading: { display: "flex", flexWrap: "wrap", alignItems: "start", justifyContent: "space-between", gap: 16 }, headingCopy: { display: "grid", gap: 4, minWidth: 0 }, title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 26, lineHeight: 1.15, overflowWrap: "anywhere" }, meta: { margin: 0, color: "var(--muted-foreground)", fontSize: 12 }, description: { maxWidth: 820, color: "var(--text-secondary)", fontSize: 14, lineHeight: 1.6 }, projections: { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 10, "@media (max-width: 620px)": { gridTemplateColumns: "1fr" } }, projection: { display: "grid", justifyItems: "start", gap: 5, borderRadius: 10, backgroundColor: "var(--paper-100)", padding: 12 }, projectionLabel: { fontSize: 11, textTransform: "uppercase", letterSpacing: 0.4, color: "var(--muted-foreground)" }, projectionValue: { fontSize: 13, color: "var(--foreground)" }, projectionMeta: { color: "var(--muted-foreground)", fontSize: 11 }, configuration: { borderRadius: 9, backgroundColor: "var(--clay-50)", padding: 10, color: "var(--clay-600)", fontSize: 12 }, configurationLink: { color: "inherit", fontWeight: 700 }
});
