import { Link } from "@tanstack/react-router";
import * as stylex from "@stylexjs/stylex";
import { Clock3 } from "lucide-react";
import { normalizeWorkSearch, type WorkAttention, type WorkProject, type WorkTask } from "./workTypes";
import { AttentionBadge } from "./AttentionBadge";
import { relativeTime, taskRunLabel } from "./workModel";
import { StageBadge } from "./StageBadge";
import { TaskActions } from "./TaskActions";

type WorkTaskCardData = WorkTask | WorkAttention["task"];

export function WorkTaskCard({
  task,
  projects,
  showStage = true,
  onUpdated
}: {
  task: WorkTaskCardData;
  projects?: readonly WorkProject[];
  showStage?: boolean;
  onUpdated?: () => void | Promise<void>;
}) {
  const attention = "attention" in task ? task.attention : null;
  const runLabel = taskRunLabel(task as WorkTask);
  return (
    <article {...stylex.props(styles.card)}>
      <Link
        to="/work/tasks/$taskId"
        params={{ taskId: task.taskId }}
        search={(current) => normalizeWorkSearch(current)}
        {...stylex.props(styles.mainLink)}
      >
        <div {...stylex.props(styles.titleRow)}>
          <h3 {...stylex.props(styles.title)}>{task.title}</h3>
          {task.project ? <span {...stylex.props(styles.project)}>{task.project.name}</span> : null}
        </div>
        {task.descriptionPreview ? <p {...stylex.props(styles.description)}>{task.descriptionPreview}</p> : null}
        <div {...stylex.props(styles.meta)}>
          {showStage ? <StageBadge name={task.stage.name} behavior={task.stage.behavior} /> : null}
          {runLabel ? <span {...stylex.props(styles.run)}>{runLabel}</span> : null}
          <span {...stylex.props(styles.age)} title={task.updatedAt}>
            <Clock3 aria-hidden="true" size={12} /> updated {relativeTime(task.updatedAt)}
          </span>
        </div>
        {attention ? <AttentionBadge kind={attention.kind} label={attention.title} /> : null}
      </Link>
      <TaskActions
        compact
        task={task}
        validActions={task.validActions}
        projects={projects}
        onUpdated={onUpdated}
      />
    </article>
  );
}

const styles = stylex.create({
  card: {
    display: "grid",
    gap: 12,
    minWidth: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 12,
    backgroundColor: "var(--surface-card)",
    padding: 14,
    boxShadow: "0 1px 0 color-mix(in srgb, black 4%, transparent)",
    transition: "border-color 140ms ease, background-color 140ms ease",
    ":focus-within": { borderColor: "color-mix(in srgb, var(--pine-500) 45%, var(--border-subtle))" },
    ":hover": { "@media (hover: hover)": { borderColor: "color-mix(in srgb, var(--pine-500) 28%, var(--border-subtle))" } }
  },
  mainLink: {
    display: "grid",
    gap: 9,
    minWidth: 0,
    color: "inherit",
    textDecoration: "none",
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 4, borderRadius: 8 }
  },
  titleRow: { display: "flex", minWidth: 0, alignItems: "start", justifyContent: "space-between", gap: 8 },
  title: { minWidth: 0, margin: 0, fontFamily: "var(--font-heading)", fontSize: 15, fontWeight: 700, lineHeight: 1.3, color: "var(--foreground)" },
  project: { flexShrink: 0, maxWidth: 110, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: 11, color: "var(--muted-foreground)" },
  description: { display: "-webkit-box", margin: 0, overflow: "hidden", WebkitBoxOrient: "vertical", WebkitLineClamp: 2, fontSize: 12, lineHeight: 1.45, color: "var(--muted-foreground)" },
  meta: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: 7 },
  run: { fontSize: 11, fontWeight: 600, color: "var(--pine-700)" },
  age: { display: "inline-flex", alignItems: "center", gap: 4, fontSize: 11, color: "var(--muted-foreground)" }
});
