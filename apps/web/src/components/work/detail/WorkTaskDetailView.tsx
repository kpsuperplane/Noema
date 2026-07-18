import * as stylex from "@stylexjs/stylex";
import type { WorkProject, WorkTaskDetail } from "../workTypes";
import { TaskActivitySection } from "./TaskActivitySection";
import { TaskArtifactsSection } from "./TaskArtifactsSection";
import { TaskAttentionHistory } from "./TaskAttentionHistory";
import { TaskContractHistory } from "./TaskContractHistory";
import { TaskContractSection } from "./TaskContractSection";
import { TaskDetailOverview } from "./TaskDetailOverview";
import { TaskEvidenceSection } from "./TaskEvidenceSection";
import { TaskRunsSection } from "./TaskRunsSection";

export function WorkTaskDetailView({ task, compact, projects, onUpdated }: { task: WorkTaskDetail; compact: boolean; projects: readonly WorkProject[]; onUpdated: () => void | Promise<void> }) {
  return (
    <div data-compact={compact} {...stylex.props(styles.layout, compact && styles.compact)}>
      <TaskDetailOverview task={task} projects={projects} onUpdated={onUpdated} />
      <TaskContractSection contract={task.currentContract} />
      <TaskAttentionHistory task={task} />
      <TaskContractHistory task={task} />
      <TaskRunsSection task={task} />
      <TaskEvidenceSection task={task} />
      <TaskArtifactsSection task={task} />
      <TaskActivitySection task={task} />
    </div>
  );
}

const styles = stylex.create({
  layout: { display: "grid", gap: 22, width: "min(100%, 1120px)", marginInline: "auto", paddingBottom: 36 },
  compact: { gap: 16, width: "100%", paddingBottom: 12 }
});
