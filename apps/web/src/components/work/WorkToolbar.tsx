import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { FolderCog, Plus, Search } from "lucide-react";
import type { WorkProject, WorkView } from "./workTypes";

const views: ReadonlyArray<{ id: WorkView; label: string }> = [
  { id: "board", label: "Board" },
  { id: "list", label: "List" },
  { id: "needs-you", label: "Needs You" },
  { id: "activity", label: "Activity" },
  { id: "completed", label: "Completed" }
];

export function WorkToolbar({
  view,
  projectId,
  query,
  projects,
  onViewChange,
  onProjectChange,
  onQueryChange,
  onNewTask,
  onManageProjects
}: {
  view: WorkView;
  projectId?: string;
  query: string;
  projects: readonly WorkProject[];
  onViewChange: (view: WorkView) => void;
  onProjectChange: (projectId?: string) => void;
  onQueryChange: (query: string) => void;
  onNewTask: () => void;
  onManageProjects: () => void;
}) {
  const searchable = view === "list" || view === "completed";
  const tabRefs = React.useRef<Array<HTMLButtonElement | null>>([]);
  return (
    <header {...stylex.props(styles.root)}>
      <h1 id="work-page-title" {...stylex.props(styles.srOnly)}>Work</h1>
      <div role="tablist" aria-label="Work views" {...stylex.props(styles.tabs)}>
        {views.map((item, index) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            ref={(node) => { tabRefs.current[index] = node; }}
            aria-controls={`work-panel-${item.id}`}
            aria-selected={item.id === view}
            tabIndex={item.id === view ? 0 : -1}
            {...stylex.props(styles.tab, item.id === view && styles.tabSelected)}
            onClick={() => onViewChange(item.id)}
            onKeyDown={(event) => {
              const nextIndex = event.key === "ArrowRight"
                ? (index + 1) % views.length
                : event.key === "ArrowLeft"
                  ? (index - 1 + views.length) % views.length
                  : event.key === "Home"
                    ? 0
                    : event.key === "End"
                      ? views.length - 1
                      : null;
              if (nextIndex === null) return;
              event.preventDefault();
              onViewChange(views[nextIndex].id);
              window.requestAnimationFrame(() => tabRefs.current[nextIndex]?.focus());
            }}
          >
            {item.label}
          </button>
        ))}
      </div>
      <div aria-label="Work filters and actions" {...stylex.props(styles.tools)}>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.srOnly)}>Project</span>
          <select
            value={projectId ?? ""}
            {...stylex.props(styles.control, styles.project)}
            onChange={(event) => onProjectChange(event.currentTarget.value || undefined)}
          >
            <option value="">All work</option>
            {projects
              .filter((project) => !project.archivedAt || view === "completed")
              .map((project) => (
                <option key={project.projectId} value={project.projectId}>
                  {project.name}{project.archivedAt ? " (archived)" : ""}
                </option>
              ))}
          </select>
        </label>
        {searchable ? (
          <label {...stylex.props(styles.search)}>
            <Search aria-hidden="true" size={13} />
            <span {...stylex.props(styles.srOnly)}>Search tasks</span>
            <input
              type="search"
              value={query}
              placeholder={view === "completed" ? "Search history" : "Search tasks"}
              {...stylex.props(styles.searchInput)}
              onChange={(event) => onQueryChange(event.currentTarget.value)}
            />
          </label>
        ) : null}
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
    display: "grid",
    gridTemplateColumns: "minmax(0, auto) minmax(0, 1fr)",
    alignItems: "center",
    minHeight: "calc(44px + var(--shell-deck-header-height, 44px))",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)",
    paddingTop: "var(--shell-deck-header-height, 44px)",
    paddingInline: 12,
    "@media (max-width: 1240px)": {
      gridTemplateColumns: "minmax(0, 1fr)",
      paddingInline: 8
    }
  },
  tabs: { display: "flex", minWidth: 0, overflowX: "auto", scrollbarWidth: "none" },
  tab: {
    position: "relative",
    minHeight: 43,
    flexShrink: 0,
    borderWidth: 0,
    backgroundColor: "transparent",
    paddingInline: 9,
    color: "var(--noema-text-muted)",
    font: "inherit",
    fontSize: 12,
    fontWeight: 650,
    cursor: "pointer",
    ":hover": { color: "var(--noema-text-primary)", backgroundColor: "var(--noema-surface-hover)" },
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: -2 }
  },
  tabSelected: {
    color: "var(--noema-text-primary)",
    "::after": { content: "''", position: "absolute", right: 8, bottom: -1, left: 8, height: 2, backgroundColor: "var(--noema-pine-600)" }
  },
  tools: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    justifyContent: "flex-end",
    gap: 5,
    "@media (max-width: 1240px)": {
      justifyContent: "start",
      borderTopWidth: 1,
      borderTopStyle: "solid",
      borderTopColor: "var(--noema-border-subtle)",
      paddingBlock: 5
    }
  },
  field: { minWidth: 0 },
  control: {
    minHeight: 30,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--noema-surface-card)",
    paddingInline: 7,
    color: "var(--noema-text-primary)",
    font: "inherit",
    fontSize: 12,
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 }
  },
  project: { maxWidth: 160 },
  search: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr)",
    width: 176,
    minHeight: 30,
    alignItems: "center",
    gap: 5,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 6,
    paddingInline: 7,
    color: "var(--noema-text-muted)",
    "@media (max-width: 480px)": { width: "min(42vw, 160px)" }
  },
  searchInput: {
    minWidth: 0,
    width: "100%",
    borderWidth: 0,
    outline: "none",
    backgroundColor: "transparent",
    padding: 0,
    color: "var(--noema-text-primary)",
    font: "inherit",
    fontSize: 12
  },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" }
});
