import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { FolderCog, Plus } from "lucide-react";
import type { WorkProject, WorkView } from "./workTypes";

const tabs: ReadonlyArray<{ id: WorkView; label: string }> = [
  { id: "board", label: "Board" },
  { id: "list", label: "List" },
  { id: "needs-you", label: "Needs You" },
  { id: "activity", label: "Activity" },
  { id: "completed", label: "Completed" }
];

export function WorkHeader({ view, projectId, query, projects, onViewChange, onProjectChange, onQueryChange, onNewTask, onManageProjects }: { view: WorkView; projectId?: string; query: string; projects: readonly WorkProject[]; onViewChange: (view: WorkView, keyboard: boolean) => void; onProjectChange: (projectId?: string) => void; onQueryChange: (query: string) => void; onNewTask: () => void; onManageProjects: () => void }) {
  const showSearch = view === "list" || view === "completed";
  return (
    <header {...stylex.props(styles.header)}>
      <div {...stylex.props(styles.top)}>
        <div {...stylex.props(styles.titleBlock)}><h1 {...stylex.props(styles.pageTitle)}>Work</h1><p {...stylex.props(styles.subtitle)}>Queues, decisions, and durable evidence</p></div>
        <div {...stylex.props(styles.primaryActions)}><Button type="button" size="sm" variant="secondary" label="Manage projects" icon={<FolderCog aria-hidden="true" size={15} />} isIconOnly onClick={onManageProjects} /><Button type="button" size="sm" variant="primary" label="New task" icon={<Plus aria-hidden="true" size={15} />} onClick={onNewTask} /></div>
      </div>
      <div {...stylex.props(styles.controls)}>
        <div role="tablist" aria-label="Work views" {...stylex.props(styles.tabs)}>{tabs.map((tab, index) => <button key={tab.id} type="button" role="tab" aria-selected={view === tab.id} aria-controls={`work-panel-${tab.id}`} tabIndex={view === tab.id ? 0 : -1} data-selected={view === tab.id} {...stylex.props(styles.tab, view === tab.id && styles.selectedTab)} onClick={(event) => onViewChange(tab.id, event.detail === 0)} onKeyDown={(event) => { const next = event.key === "ArrowRight" ? tabs[(index + 1) % tabs.length] : event.key === "ArrowLeft" ? tabs[(index - 1 + tabs.length) % tabs.length] : event.key === "Home" ? tabs[0] : event.key === "End" ? tabs.at(-1) : null; if (next) { event.preventDefault(); onViewChange(next.id, true); } }}>{tab.label}</button>)}</div>
        <div aria-label="Work filters" {...stylex.props(styles.filters)}>
          <label><span {...stylex.props(styles.srOnly)}>Project</span><select value={projectId ?? ""} {...stylex.props(styles.control)} onChange={(event) => onProjectChange(event.currentTarget.value || undefined)}><option value="">All work</option>{projects.filter((project) => !project.archivedAt || view === "completed").map((project) => <option key={project.projectId} value={project.projectId}>{project.name}{project.archivedAt ? " (archived)" : ""}</option>)}</select></label>
          {showSearch ? <label {...stylex.props(styles.search)}><span {...stylex.props(styles.srOnly)}>Search tasks</span><input type="search" value={query} placeholder={view === "completed" ? "Search history" : "Search tasks"} {...stylex.props(styles.control, styles.searchInput)} onChange={(event) => onQueryChange(event.currentTarget.value)} /></label> : null}
        </div>
      </div>
    </header>
  );
}

const styles = stylex.create({
  header: { display: "grid", gap: 18 }, top: { display: "flex", flexWrap: "wrap", alignItems: "start", justifyContent: "space-between", gap: 14 }, titleBlock: { display: "grid", gap: 3 }, pageTitle: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 28, lineHeight: 1.1, letterSpacing: -0.5 }, subtitle: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }, primaryActions: { display: "flex", gap: 8 }, controls: { display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: 12, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)" }, tabs: { display: "flex", maxWidth: "100%", gap: 2, overflowX: "auto" }, tab: { position: "relative", minHeight: 40, flexShrink: 0, borderWidth: 0, backgroundColor: "transparent", paddingInline: 10, color: "var(--muted-foreground)", font: "inherit", fontSize: 13, fontWeight: 600, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: -2 } }, selectedTab: { color: "var(--foreground)", "::after": { content: "''", position: "absolute", right: 8, bottom: -1, left: 8, height: 2, borderRadius: 999, backgroundColor: "var(--pine-600)" } }, filters: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8, paddingBottom: 7 }, control: { minHeight: 34, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingInline: 10, color: "var(--foreground)", font: "inherit", fontSize: 12 }, search: { display: "grid" }, searchInput: { width: 180 }, srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" }
});
