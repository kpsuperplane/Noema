import type { ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";
import { DropdownMenu, type DropdownMenuButtonProps } from "@astryxdesign/core/DropdownMenu";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Archive, ArchiveRestore, MoreHorizontal, Pencil, Plus } from "lucide-react";
import { ShellSidebar } from "@/components/shell/ShellSidebar";
import type { ShellMenuItem, ShellMenuLevel } from "@/components/shell/shellNavigation";
import {
  ProjectManagerPane,
  useProjectManager
} from "./ProjectManagerPane";
import type { WorkProject } from "./workTypes";

export function WorkSidebar({
  menuLevel,
  projects,
  onSelectItem,
  onUpdated
}: {
  menuLevel: ShellMenuLevel;
  projects: readonly WorkProject[];
  onSelectItem: (item: ShellMenuItem) => void;
  onUpdated: () => void | Promise<void>;
}) {
  const manager = useProjectManager({ projects, onUpdated });

  const renderItemAction = (item: ShellMenuItem) => {
    if (item.itemId === "work.workspace.personal") {
      return (
        <IconButton
          type="button"
          variant="ghost"
          size="sm"
          label="New project"
          icon={<Plus aria-hidden="true" size={14} />}
          xstyle={iconButtonXStyle(styles.itemAction)}
          onClick={manager.openCreate}
        />
      );
    }

    if (!item.itemId.startsWith("work.project.")) {
      return null;
    }

    const projectId = item.itemId.slice("work.project.".length);
    const project = projects.find((candidate) => candidate.projectId === projectId);
    if (!project) return null;

    return (
      <DropdownMenu
        button={dropdownButton({
          label: `Actions for ${project.name}`,
          icon: <MoreHorizontal aria-hidden="true" size={15} />
        })}
        hasChevron={false}
        placement="below"
        menuWidth={174}
        items={[
          {
            label: "Edit project",
            icon: <Pencil aria-hidden="true" size={14} />,
            onClick: () => manager.openEdit(project.projectId),
            isDisabled: manager.busy
          },
          { type: "divider" },
          {
            label: project.archivedAt ? "Reopen project" : "Archive project",
            icon: project.archivedAt
              ? <ArchiveRestore aria-hidden="true" size={14} />
              : <Archive aria-hidden="true" size={14} />,
            onClick: () => void manager.toggleArchived(project.projectId),
            isDisabled: manager.busy
          }
        ]}
      />
    );
  };

  return (
    <div data-slot="work-sidebar" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.navigation)}>
        <ShellSidebar
          menuLevel={menuLevel}
          onSelectItem={onSelectItem}
          renderItemAction={renderItemAction}
        />
      </div>
      <ProjectManagerPane projects={projects} manager={manager} />
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    height: "100%",
    minHeight: 0,
    flexDirection: "column",
    overflow: "hidden"
  },
  navigation: {
    minHeight: 0,
    flex: 1
  },
  itemAction: {
    minWidth: 28,
    minHeight: 28,
    padding: 0,
    color: "var(--muted-foreground)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 56%, transparent)",
        color: "var(--pine-700)"
      }
    }
  }
});

function dropdownButton({
  label,
  icon
}: {
  label: string;
  icon: ReactNode;
}): DropdownMenuButtonProps {
  return {
    label,
    icon,
    isIconOnly: true,
    size: "sm",
    variant: "ghost",
    xstyle: iconButtonXStyle(styles.itemAction)
  };
}

function iconButtonXStyle(...xstyle: unknown[]): DropdownMenuButtonProps["xstyle"] {
  return xstyle as unknown as DropdownMenuButtonProps["xstyle"];
}
