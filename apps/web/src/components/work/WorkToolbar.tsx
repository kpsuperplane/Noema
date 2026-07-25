import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { FolderCog, Plus } from "lucide-react";
import type { WorkProject } from "./workTypes";

export function WorkToolbar({
  projectId,
  projects,
  onProjectChange,
  onNewTask,
  onManageProjects
}: {
  projectId?: string;
  projects: readonly WorkProject[];
  onProjectChange: (projectId?: string) => void;
  onNewTask: () => void;
  onManageProjects: () => void;
}) {
  return (
    <header {...stylex.props(styles.root)}>
      <h1 id="work-page-title" {...stylex.props(styles.srOnly)}>Work</h1>
      <div aria-label="Work filters and actions" {...stylex.props(styles.tools)}>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.srOnly)}>Project</span>
          <select
            value={projectId ?? ""}
            {...stylex.props(styles.control, styles.project)}
            onChange={(event) => onProjectChange(event.currentTarget.value || undefined)}
          >
            <option value="">All work</option>
            {projects.map((project) => (
              <option key={project.projectId} value={project.projectId}>
                {project.name}{project.archivedAt ? " (archived)" : ""}
              </option>
            ))}
          </select>
        </label>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          label="Manage projects"
          icon={<FolderCog aria-hidden="true" size={15} />}
          isIconOnly
          onClick={onManageProjects}
        />
        <Button
          type="button"
          size="sm"
          variant="primary"
          label="New task"
          icon={<Plus aria-hidden="true" size={15} />}
          onClick={onNewTask}
        />
      </div>
    </header>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    alignItems: "center",
    justifyContent: "flex-end",
    minHeight: "calc(44px + var(--shell-deck-header-height, 44px))",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)",
    paddingTop: "var(--shell-deck-header-height, 44px)",
    paddingInline: "var(--spacing-3)",
    "@media (max-width: 760px)": { paddingInline: "var(--spacing-2)" }
  },
  tools: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    justifyContent: "flex-end",
    gap: "var(--spacing-1)"
  },
  field: { minWidth: 0 },
  control: {
    minHeight: 30,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--noema-surface-card)",
    paddingInline: "var(--spacing-2)",
    color: "var(--noema-text-primary)",
    font: "inherit",
    fontSize: 12,
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 }
  },
  project: { maxWidth: 160 },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" }
});
