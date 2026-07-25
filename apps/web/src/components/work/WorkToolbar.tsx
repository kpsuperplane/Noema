import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { FolderCog, Plus, Search } from "lucide-react";
import type { WorkProject } from "./workTypes";

export function WorkToolbar({
  projectId,
  projects,
  queryDraft,
  terminal,
  onProjectChange,
  onQueryChange,
  onTerminalChange,
  onNewTask,
  onManageProjects
}: {
  projectId?: string;
  projects: readonly WorkProject[];
  queryDraft: string;
  terminal: "all" | "completed" | "cancelled";
  onProjectChange: (projectId?: string) => void;
  onQueryChange: (query: string) => void;
  onTerminalChange: (value: "all" | "completed" | "cancelled") => void;
  onNewTask: () => void;
  onManageProjects: () => void;
}) {
  return (
    <header {...stylex.props(styles.root)}>
      <h1 id="work-page-title" {...stylex.props(styles.srOnly)}>Tasks</h1>
      <div aria-label="Task filters and actions" {...stylex.props(styles.tools)}>
        <div role="group" aria-label="Task filters" {...stylex.props(styles.filters)}>
          <label {...stylex.props(styles.field)}>
            <span {...stylex.props(styles.srOnly)}>Project</span>
            <select
              value={projectId ?? ""}
              {...stylex.props(styles.control, styles.project)}
              onChange={(event) => onProjectChange(event.currentTarget.value || undefined)}
            >
              <option value="">All tasks</option>
              {projects.map((project) => (
                <option key={project.projectId} value={project.projectId}>
                  {project.name}{project.archivedAt ? " (archived)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label {...stylex.props(styles.search)}>
            <Search aria-hidden="true" size={13} />
            <span {...stylex.props(styles.srOnly)}>Search history</span>
            <input type="search" value={queryDraft} placeholder="Search history" {...stylex.props(styles.searchInput)} onChange={(event) => onQueryChange(event.currentTarget.value)} />
          </label>
          <label {...stylex.props(styles.field)}>
            <span {...stylex.props(styles.srOnly)}>History status</span>
            <select value={terminal} {...stylex.props(styles.control, styles.terminal)} onChange={(event) => onTerminalChange(event.currentTarget.value as typeof terminal)}>
              <option value="all">Done and cancelled</option><option value="completed">Done</option><option value="cancelled">Cancelled</option>
            </select>
          </label>
        </div>
        <div {...stylex.props(styles.actions)}>
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
      </div>
    </header>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    minHeight: "calc(44px + var(--shell-deck-header-height, 44px))",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)",
    paddingTop: "var(--shell-deck-header-height, 44px)",
    paddingInline: "var(--spacing-3)",
    "@media (max-width: 760px)": { paddingBottom: "var(--spacing-2)", paddingInline: "var(--spacing-2)" }
  },
  tools: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    flex: 1,
    justifyContent: "space-between",
    gap: "var(--spacing-2)",
    "@media (max-width: 760px)": { flexWrap: "wrap" }
  },
  filters: { display: "flex", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)", "@media (max-width: 760px)": { width: "100%", overflowX: "auto" } },
  actions: { display: "flex", flexShrink: 0, alignItems: "center", gap: "var(--spacing-1)", marginInlineStart: "auto" },
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
  terminal: { maxWidth: 160 },
  search: { display: "grid", gridTemplateColumns: "auto minmax(0, 1fr)", width: 176, minHeight: 30, flexShrink: 0, alignItems: "center", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 6, backgroundColor: "var(--noema-surface-card)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-muted)" },
  searchInput: { minWidth: 0, width: "100%", borderWidth: 0, outline: "none", backgroundColor: "transparent", padding: 0, color: "var(--noema-text-primary)", font: "inherit", fontSize: 12 },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" }
});
