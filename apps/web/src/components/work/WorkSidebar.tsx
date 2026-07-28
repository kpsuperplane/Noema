import type { ComponentProps, ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";
import { DropdownMenu, type DropdownMenuButtonProps } from "@astryxdesign/core/DropdownMenu";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Archive, ArchiveRestore, Check, Folder, MoreHorizontal, Pencil, Plus, X } from "lucide-react";
import { ShellSidebar, shellSidebarStyles } from "@/components/shell/ShellSidebar";
import type {
  ShellMenuEntry,
  ShellMenuItem,
  ShellMenuLevel
} from "@/components/shell/shellNavigation";
import {
  type ProjectManagerController,
  useProjectManager
} from "./useProjectManager";
import type { WorkProject } from "./workTypes";

const DRAFT_PROJECT_ITEM_ID: `work.project.${string}` = "work.project.__draft__";

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
  const renderedMenuLevel = manager.editor?.kind === "create"
    ? withDraftProject(menuLevel)
    : menuLevel;

  const renderItemContent = (item: ShellMenuItem, defaultControl: ReactNode) => {
    if (item.itemId === "work.workspace.personal") {
      return (
        <>
          {defaultControl}
          {manager.editor ? null : (
            <IconButton
              type="button"
              variant="ghost"
              size="sm"
              label="New project"
              icon={<Plus aria-hidden="true" size={14} />}
              xstyle={iconButtonXStyle(styles.itemAction)}
              onClick={manager.openCreate}
            />
          )}
        </>
      );
    }

    if (item.itemId === DRAFT_PROJECT_ITEM_ID) {
      return <ProjectNameEditor manager={manager} isNew />;
    }

    if (!item.itemId.startsWith("work.project.")) {
      return defaultControl;
    }

    const projectId = item.itemId.slice("work.project.".length);
    const project = projects.find((candidate) => candidate.projectId === projectId);
    if (!project) return defaultControl;

    if (manager.editor?.kind === "rename" && manager.editor.projectId === projectId) {
      return <ProjectNameEditor manager={manager} />;
    }

    return (
      <>
        {defaultControl}
        <DropdownMenu
          button={dropdownButton({
            label: `Actions for ${project.name}`,
            icon: <MoreHorizontal aria-hidden="true" size={15} />
          })}
          hasChevron={false}
          placement="below"
          menuWidth={174}
          xstyle={dropdownXStyle(styles.projectMenu)}
          items={[
            {
              label: "Rename project",
              icon: <Pencil aria-hidden="true" size={14} />,
              onClick: () => manager.openRename(project.projectId),
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
      </>
    );
  };

  return (
    <div data-slot="work-sidebar" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.navigation)}>
        <ShellSidebar
          menuLevel={renderedMenuLevel}
          onSelectItem={onSelectItem}
          renderItemContent={renderItemContent}
        />
      </div>
    </div>
  );
}

function ProjectNameEditor({
  manager,
  isNew = false
}: {
  manager: ProjectManagerController;
  isNew?: boolean;
}) {
  const errorId = "work-project-name-error";
  return (
    <form
      aria-label={isNew ? "New project" : "Rename project"}
      {...stylex.props(styles.editor)}
      onSubmit={(event) => {
        event.preventDefault();
        void manager.save();
      }}
    >
      <span {...stylex.props(shellSidebarStyles.menuIcon)} aria-hidden="true">
        <Folder size={16} />
      </span>
      <input
        autoFocus
        required
        aria-label={isNew ? "Project name" : "New project name"}
        aria-invalid={manager.error ? "true" : undefined}
        aria-describedby={manager.error ? errorId : undefined}
        value={manager.name}
        {...stylex.props(styles.nameInput)}
        onChange={(event) => manager.setName(event.currentTarget.value)}
        onFocus={(event) => event.currentTarget.select()}
        onKeyDown={(event) => {
          if (event.key === "Escape") manager.closeEditor();
        }}
      />
      <IconButton
        type="submit"
        variant="ghost"
        size="sm"
        label={isNew ? "Create project" : "Save project name"}
        icon={isNew ? <Plus aria-hidden="true" size={14} /> : <Check aria-hidden="true" size={14} />}
        isLoading={manager.busy}
        isDisabled={manager.busy || !manager.name.trim()}
        xstyle={iconButtonXStyle(styles.editorAction)}
      />
      <IconButton
        type="button"
        variant="ghost"
        size="sm"
        label="Cancel"
        icon={<X aria-hidden="true" size={14} />}
        isDisabled={manager.busy}
        xstyle={iconButtonXStyle(styles.editorAction)}
        onClick={manager.closeEditor}
      />
      {manager.error ? (
        <span id={errorId} role="alert" {...stylex.props(styles.editorError)}>
          {manager.error}
        </span>
      ) : null}
    </form>
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
    padding: "var(--spacing-0)",
    marginInlineEnd: "var(--spacing-0-5)",
    color: "var(--muted-foreground)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 56%, transparent)",
        color: "var(--pine-700)"
      }
    }
  },
  projectMenu: {
    "--color-overlay-hover": "color-mix(in srgb, var(--pine-100) 60%, transparent)",
    "--color-overlay-pressed": "color-mix(in srgb, var(--pine-100) 82%, transparent)"
  },
  editor: {
    display: "grid",
    gridTemplateColumns: "18px minmax(0, 1fr) 26px 26px",
    alignItems: "center",
    gap: "var(--spacing-1)",
    width: "100%",
    minWidth: 0,
    paddingBlock: "var(--spacing-0-5)",
    paddingInlineStart: "var(--spacing-5)",
    paddingInlineEnd: "var(--spacing-0-5)"
  },
  nameInput: {
    boxSizing: "border-box",
    width: "100%",
    minWidth: 0,
    height: 26,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--background)",
    paddingInline: "var(--spacing-1-5)",
    color: "var(--foreground)",
    fontFamily: "inherit",
    fontSize: 13,
    outline: "none",
    ":focus-visible": {
      borderColor: "var(--pine-600)",
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 1
    }
  },
  editorAction: {
    minWidth: 26,
    minHeight: 26,
    height: 26,
    padding: "var(--spacing-0)"
  },
  editorError: {
    gridColumn: "2 / -1",
    paddingBlockEnd: "var(--spacing-0-5)",
    color: "var(--destructive)",
    fontSize: 10,
    lineHeight: 1.25
  }
});

function withDraftProject(menuLevel: ShellMenuLevel): ShellMenuLevel {
  const draft: ShellMenuEntry = {
    kind: "item",
    item: {
      itemId: DRAFT_PROJECT_ITEM_ID,
      label: "New project",
      icon: Folder,
      depth: 1
    }
  };
  return {
    ...menuLevel,
    items: menuLevel.items.flatMap((entry) =>
      entry.kind === "item" && entry.item.itemId === "work.workspace.personal"
        ? [entry, draft]
        : [entry]
    )
  };
}

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

function dropdownXStyle(...xstyle: unknown[]): ComponentProps<typeof DropdownMenu>["xstyle"] {
  return xstyle as unknown as ComponentProps<typeof DropdownMenu>["xstyle"];
}
