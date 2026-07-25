import * as stylex from "@stylexjs/stylex";
import { BriefcaseBusiness, Folder, FolderCog, ListTodo } from "lucide-react";
import { shellSidebarStyles } from "../shell/ShellSidebar";
import type { WorkProject } from "./workTypes";

export function WorkFolderRail({
  projectId,
  projects,
  open,
  onProjectChange,
  onManageProjects
}: {
  projectId?: string;
  projects: readonly WorkProject[];
  open: boolean;
  onProjectChange: (projectId?: string) => void;
  onManageProjects: () => void;
}) {
  return (
    <nav id="work-folder-rail" aria-label="Task folders" {...stylex.props(styles.rail, shellSidebarStyles.nav, open && styles.open)}>
      <div {...stylex.props(shellSidebarStyles.sideNavBody, styles.body)}>
        <button
          type="button"
          aria-current={!projectId ? "page" : undefined}
          {...stylex.props(shellSidebarStyles.menuButton, !projectId && shellSidebarStyles.menuButtonActive)}
          onClick={() => onProjectChange(undefined)}
        >
          <span {...stylex.props(shellSidebarStyles.menuIcon)} aria-hidden="true"><ListTodo size={16} /></span>
          <span {...stylex.props(shellSidebarStyles.menuLabel)}>All tasks</span>
        </button>

        <div {...stylex.props(shellSidebarStyles.groupLabel)}>Workspaces</div>
        <div {...stylex.props(styles.workspace)}>
          <span {...stylex.props(shellSidebarStyles.menuIcon)} aria-hidden="true"><BriefcaseBusiness size={16} /></span>
          <span {...stylex.props(shellSidebarStyles.menuLabel)}>Personal</span>
        </div>

        <div {...stylex.props(shellSidebarStyles.groupLabel)}>Projects</div>
        {projects.length ? projects.map((project) => (
          <button
            key={project.projectId}
            type="button"
            aria-current={project.projectId === projectId ? "page" : undefined}
            {...stylex.props(shellSidebarStyles.menuButton, project.projectId === projectId && shellSidebarStyles.menuButtonActive)}
            onClick={() => onProjectChange(project.projectId)}
          >
            <span {...stylex.props(shellSidebarStyles.menuIcon)} aria-hidden="true"><Folder size={16} /></span>
            <span {...stylex.props(shellSidebarStyles.menuLabel)}>{project.name}{project.archivedAt ? " (archived)" : ""}</span>
          </button>
        )) : (
          <span {...stylex.props(styles.empty)}>No projects yet</span>
        )}
      </div>
      <button
        type="button"
        {...stylex.props(shellSidebarStyles.menuButton, styles.manage)}
        onClick={onManageProjects}
      >
        <span {...stylex.props(shellSidebarStyles.menuIcon)} aria-hidden="true"><FolderCog size={16} /></span>
        <span {...stylex.props(shellSidebarStyles.menuLabel)}>Manage projects</span>
      </button>
    </nav>
  );
}

const styles = stylex.create({
  rail: {
    position: "absolute",
    top: 0,
    bottom: 0,
    left: 0,
    zIndex: 6,
    width: "min(280px, 82vw)",
    borderRightWidth: 1,
    borderRightStyle: "solid",
    borderRightColor: "var(--noema-border-subtle)",
    backgroundColor: "var(--noema-surface-card)",
    boxShadow: "8px 0 24px color-mix(in srgb, black 10%, transparent)",
    transform: "translateX(-100%)",
    transitionDuration: "var(--motion-spring-surface-duration)",
    transitionProperty: "transform, visibility",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    visibility: "hidden",
    pointerEvents: "none",
    "@media (min-width: 1180px)": {
      position: "relative",
      zIndex: 1,
      gridColumn: 1,
      gridRow: 1,
      width: "auto",
      boxShadow: "none",
      transform: "none",
      visibility: "visible",
      pointerEvents: "auto"
    }
  },
  body: {
    paddingTop: "calc(var(--shell-deck-header-height, 44px) + var(--spacing-2))"
  },
  workspace: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-2)",
    height: 34,
    paddingInline: "var(--spacing-2)",
    color: "var(--noema-text-muted)",
    fontSize: 14
  },
  empty: {
    paddingInline: 10,
    color: "var(--noema-text-muted)",
    fontSize: 12
  },
  manage: {
    marginTop: "auto",
    flexShrink: 0
  },
  open: { transform: "translateX(0)", visibility: "visible", pointerEvents: "auto" }
});
