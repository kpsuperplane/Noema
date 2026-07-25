import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { BriefcaseBusiness, Folder, FolderCog, ListTodo } from "lucide-react";
import { ShellSidebar } from "../shell/ShellSidebar";
import type { ShellMenuItem, ShellMenuLevel } from "../shell/shellNavigation";
import type { WorkProject } from "./workTypes";

const allTasksItemId = "work.all" as const;
const personalWorkspaceItemId = "work.workspace.personal" as const;
const manageProjectsItemId = "work.manage-projects" as const;

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
  const menuLevel = React.useMemo<ShellMenuLevel>(() => ({
    levelId: "work",
    ariaLabel: "Task folders",
    title: "Tasks",
    activeItemId: projectId ? projectItemId(projectId) : allTasksItemId,
    items: [
      { kind: "item", item: { itemId: allTasksItemId, label: "All tasks", icon: ListTodo } },
      { kind: "group", label: "Workspaces" },
      { kind: "item", item: { itemId: personalWorkspaceItemId, label: "Personal", icon: BriefcaseBusiness } },
      { kind: "group", label: "Projects" },
      ...projects.map((project) => ({
        kind: "item" as const,
        item: {
          itemId: projectItemId(project.projectId),
          label: `${project.name}${project.archivedAt ? " (archived)" : ""}`,
          icon: Folder
        }
      })),
      { kind: "item", item: { itemId: manageProjectsItemId, label: "Manage projects", icon: FolderCog } }
    ]
  }), [projectId, projects]);

  const handleSelectItem = React.useCallback((item: ShellMenuItem) => {
    if (item.itemId === allTasksItemId || item.itemId === personalWorkspaceItemId) {
      onProjectChange(undefined);
    } else if (item.itemId === manageProjectsItemId) {
      onManageProjects();
    } else if (item.itemId.startsWith("work.project.")) {
      onProjectChange(item.itemId.slice("work.project.".length));
    }
  }, [onManageProjects, onProjectChange]);

  return (
    <div id="work-folder-rail" {...stylex.props(styles.rail, open && styles.open)}>
      <ShellSidebar menuLevel={menuLevel} onSelectItem={handleSelectItem} />
    </div>
  );
}

function projectItemId(projectId: string): `work.project.${string}` {
  return `work.project.${projectId}`;
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
  open: { transform: "translateX(0)", visibility: "visible", pointerEvents: "auto" }
});
