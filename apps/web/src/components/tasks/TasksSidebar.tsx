import * as React from "react";
import type { ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";
import { DropdownMenu, type DropdownMenuButtonProps } from "@astryxdesign/core/DropdownMenu";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import { Archive, ArchiveRestore, Folder, MoreHorizontal, Pencil, Plus } from "lucide-react";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { ShellSidebar } from "@/components/shell/ShellSidebar";
import type {
  ShellMenuEntry,
  ShellMenuItem,
  ShellMenuLevel
} from "@/components/shell/shellNavigation";
import {
  type ProjectManagerController,
  useProjectManager
} from "./useProjectManager";
import type { TasksProject } from "./tasksTypes";

const ARCHIVED_PROJECTS_ITEM_ID = "tasks.projects.archived";

export function TasksSidebar({
  menuLevel,
  projects,
  onSelectItem,
  onUpdated
}: {
  menuLevel: ShellMenuLevel;
  projects: readonly TasksProject[];
  onSelectItem: (item: ShellMenuItem) => void;
  onUpdated: () => void | Promise<void>;
}) {
  const manager = useProjectManager({ projects, onUpdated });
  const [archivedExpanded, setArchivedExpanded] = React.useState(false);

  const renderedMenuLevel = withArchivedProjects(
    menuLevel,
    projects,
    archivedExpanded
  );

  const handleSelectItem = (item: ShellMenuItem) => {
    if (item.itemId === ARCHIVED_PROJECTS_ITEM_ID) {
      setArchivedExpanded((expanded) => !expanded);
      return;
    }
    onSelectItem(item);
  };

  const renderItemContent = (item: ShellMenuItem, defaultControl: ReactNode) => {
    if (item.itemId === "tasks.workspace.personal") {
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
              xstyle={styles.itemAction}
              onClick={manager.openCreate}
            />
          )}
        </>
      );
    }

    if (!item.itemId.startsWith("tasks.project.")) {
      return defaultControl;
    }

    const projectId = item.itemId.slice("tasks.project.".length);
    const project = projects.find((candidate) => candidate.projectId === projectId);
    if (!project) return defaultControl;

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
          className={stylex.props(styles.projectMenu).className}
          items={[
            {
              label: "Edit project",
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
    <div data-slot="tasks-sidebar" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.navigation)}>
        <ShellSidebar
          menuLevel={renderedMenuLevel}
          onSelectItem={handleSelectItem}
          renderItemContent={renderItemContent}
        />
      </div>
      <ProjectDialog manager={manager} />
    </div>
  );
}

function ProjectDialog({ manager }: { manager: ProjectManagerController }) {
  if (!manager.editor) return null;
  const isNew = manager.editor.kind === "create";
  return (
    <Dialog isOpen onOpenChange={(open) => !open && manager.closeEditor()} purpose="form" width={480} aria-label={isNew ? "New project" : "Edit project"}>
      <Layout height="auto" header={<DialogHeader title={isNew ? "New project" : "Edit project"} subtitle="A project folder becomes the default working directory for its tasks." onOpenChange={(open) => !open && manager.closeEditor()} />} content={<LayoutContent>
        <VStack as="form" gap={3} onSubmit={(event) => { event.preventDefault(); void manager.save(); }}>
          <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Name</span><input data-autofocus required value={manager.name} {...stylex.props(styles.input)} onChange={(event) => manager.setName(event.currentTarget.value)} /></VStack>
          <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Project folder (optional)</span><input value={manager.folder} placeholder="/absolute/path/to/project" {...stylex.props(styles.input)} onChange={(event) => manager.setFolder(event.currentTarget.value)} /><span {...stylex.props(styles.hint)}>Must be an absolute path in the selected executor's filesystem.</span></VStack>
          {manager.error ? <p role="alert" {...stylex.props(styles.editorError)}>{manager.error}</p> : null}
          <HStack gap={2} justify="end"><Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={manager.busy} onClick={manager.closeEditor} /><Button type="submit" size="sm" variant="primary" label={isNew ? "Create project" : "Save"} isLoading={manager.busy} isDisabled={manager.busy || !manager.name.trim()} /></HStack>
        </VStack>
      </LayoutContent>} />
    </Dialog>
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
  field: { color: "var(--foreground)", fontSize: 13, fontWeight: 600 },
  input: {
    width: "100%",
    minHeight: 38,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 8,
    backgroundColor: "var(--background)",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)",
    color: "var(--foreground)",
    font: "inherit",
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 2
    }
  },
  editorError: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.4
  },
  hint: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 400 }
});

function withArchivedProjects(
  menuLevel: ShellMenuLevel,
  projects: readonly TasksProject[],
  expanded: boolean
): ShellMenuLevel {
  const archivedProjects = projects.filter((project) => project.archivedAt);
  if (!expanded || archivedProjects.length === 0) {
    return {
      ...menuLevel,
      items: menuLevel.items.map((entry) =>
        entry.kind === "item" && entry.item.itemId === ARCHIVED_PROJECTS_ITEM_ID
          ? { ...entry, item: { ...entry.item, ariaExpanded: false } }
          : entry
      )
    };
  }

  const archivedEntries: ShellMenuEntry[] = archivedProjects.map((project) => ({
    kind: "item",
    item: {
      itemId: `tasks.project.${project.projectId}`,
      label: project.name,
      route: { kind: "tasks", projectId: project.projectId },
      icon: Folder,
      depth: 1,
      pinned: true
    }
  }));

  return {
    ...menuLevel,
    items: menuLevel.items.flatMap((entry) => {
      if (entry.kind !== "item" || entry.item.itemId !== ARCHIVED_PROJECTS_ITEM_ID) {
        return [entry];
      }

      const toggleEntry: ShellMenuEntry = {
        ...entry,
        item: { ...entry.item, ariaExpanded: expanded }
      };
      return [toggleEntry, ...archivedEntries];
    })
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
    xstyle: styles.itemAction
  };
}
