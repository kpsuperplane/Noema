import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { X } from "lucide-react";
import {
  WorkArchiveProjectDocument,
  WorkCreateProjectDocument,
  WorkReopenProjectDocument,
  WorkUpdateProjectDocument
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { WorkProject } from "./workTypes";
import { isStaleCommandError } from "./semanticCommand";

export type ProjectEditor = "new" | string;

export type ProjectManagerController = {
  editor: ProjectEditor | null;
  selected: WorkProject | null;
  name: string;
  description: string;
  busy: boolean;
  stale: boolean;
  error: string | null;
  acknowledging: boolean;
  openCreate: () => void;
  openEdit: (projectId: string) => void;
  closeEditor: () => void;
  setName: (name: string) => void;
  setDescription: (description: string) => void;
  save: () => Promise<void>;
  toggleArchived: (projectId: string) => Promise<void>;
  acknowledgeLatest: () => Promise<void>;
};

export function useProjectManager({
  projects,
  onUpdated
}: {
  projects: readonly WorkProject[];
  onUpdated: () => void | Promise<void>;
}): ProjectManagerController {
  const [editor, setEditor] = React.useState<ProjectEditor | null>(null);
  const [selected, setSelected] = React.useState<WorkProject | null>(null);
  const [name, setName] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [requiresAcknowledgement, setRequiresAcknowledgement] = React.useState(false);
  const [acknowledging, setAcknowledging] = React.useState(false);
  const [create, createState] = useMutation(WorkCreateProjectDocument);
  const [update, updateState] = useMutation(WorkUpdateProjectDocument);
  const [archive, archiveState] = useMutation(WorkArchiveProjectDocument);
  const [reopen, reopenState] = useMutation(WorkReopenProjectDocument);
  const busy = createState.loading || updateState.loading || archiveState.loading || reopenState.loading;
  const liveSelected = selected ? projects.find((project) => project.projectId === selected.projectId) ?? null : null;
  const liveSelectedRef = React.useRef(liveSelected);
  const liveSelectionChanged = Boolean(selected && (!liveSelected || liveSelected.revision > selected.revision));
  const stale = requiresAcknowledgement || liveSelectionChanged;

  React.useEffect(() => {
    liveSelectedRef.current = liveSelected;
  }, [liveSelected]);

  const resetEditor = React.useCallback(() => {
    setEditor(null);
    setSelected(null);
    setName("");
    setDescription("");
    setError(null);
    setRequiresAcknowledgement(false);
  }, []);

  const openCreate = React.useCallback(() => {
    setEditor("new");
    setSelected(null);
    setName("");
    setDescription("");
    setError(null);
    setRequiresAcknowledgement(false);
  }, []);

  const openEdit = React.useCallback((projectId: string) => {
    const project = projects.find((candidate) => candidate.projectId === projectId);
    if (!project) return;
    setEditor(project.projectId);
    setSelected({ ...project });
    setName(project.name);
    setDescription(project.description);
    setError(null);
    setRequiresAcknowledgement(false);
  }, [projects]);

  const acknowledgeLatest = React.useCallback(async () => {
    setAcknowledging(true);
    try {
      await onUpdated();
      await nextFrame();
      const latest = liveSelectedRef.current;
      if (!selected || !latest || latest.revision <= selected.revision) {
        setError("The latest project version is not available yet. Retry when project updates reconnect.");
        return;
      }
      setSelected({ ...latest });
      setName(latest.name);
      setDescription(latest.description);
      setRequiresAcknowledgement(false);
      setError(null);
    } catch {
      setError("The latest project version could not be loaded. Retry when project updates reconnect.");
    } finally {
      setAcknowledging(false);
    }
  }, [onUpdated, selected]);

  const save = React.useCallback(async () => {
    if (stale || !editor) return;
    setError(null);
    try {
      if (selected) {
        const result = await update({
          variables: {
            input: {
              projectId: selected.projectId,
              expectedRevision: selected.revision,
              name: name.trim(),
              description: description.trim(),
              clientMutationId: createClientId()
            }
          }
        });
        if (result.data?.updateProject.project) {
          setSelected(result.data.updateProject.project);
        }
      } else {
        await create({
          variables: {
            input: {
              workspaceId: "workspace:personal",
              name: name.trim(),
              description: description.trim(),
              clientMutationId: createClientId()
            }
          }
        });
      }
      try {
        await onUpdated();
      } catch {
        // The committed mutation remains authoritative; the pane stays available for retrying.
      }
      if (!selected) resetEditor();
    } catch (caught) {
      if (isStaleCommandError(caught)) {
        try {
          await onUpdated();
        } catch {
          // The acknowledgement control uses the next available authoritative project page.
        }
        setRequiresAcknowledgement(true);
        setError("This project changed elsewhere. Review the latest version before saving your draft.");
      } else {
        setError(caught instanceof Error ? caught.message : "Project could not be saved.");
      }
    }
  }, [create, editor, name, onUpdated, resetEditor, selected, stale, update, description]);

  const toggleArchived = React.useCallback(async (projectId: string) => {
    const project = projects.find((candidate) => candidate.projectId === projectId);
    if (!project) return;
    const target = selected?.projectId === projectId ? selected : project;
    setError(null);
    try {
      const input = {
        projectId: target.projectId,
        expectedRevision: target.revision,
        clientMutationId: createClientId()
      };
      const updated = target.archivedAt
        ? (await reopen({ variables: { input } })).data?.reopenProject.project
        : (await archive({ variables: { input } })).data?.archiveProject.project;
      if (updated && selected?.projectId === projectId) {
        setSelected(updated);
        setName(updated.name);
        setDescription(updated.description);
      }
      try {
        await onUpdated();
      } catch {
        // The committed mutation remains authoritative; the pane stays available for retrying.
      }
    } catch (caught) {
      if (isStaleCommandError(caught)) {
        openEdit(projectId);
        setError("This project changed elsewhere. Review the latest version before updating it.");
      } else {
        openEdit(projectId);
        setError(caught instanceof Error ? caught.message : "Project could not be updated.");
      }
    }
  }, [archive, onUpdated, openEdit, projects, reopen, selected]);

  return {
    editor,
    selected,
    name,
    description,
    busy,
    stale,
    error,
    acknowledging,
    openCreate,
    openEdit,
    closeEditor: resetEditor,
    setName,
    setDescription,
    save,
    toggleArchived,
    acknowledgeLatest
  };
}

export function ProjectManagerPane({
  projects,
  manager
}: {
  projects: readonly WorkProject[];
  manager: ProjectManagerController;
}) {
  if (!manager.editor) return null;
  const project = manager.editor === "new"
    ? null
    : projects.find((candidate) => candidate.projectId === manager.editor) ?? manager.selected;

  return (
    <section
      aria-label={project ? `Edit ${project.name}` : "New project"}
      data-slot="work-project-manager"
      {...stylex.props(styles.root)}
    >
      <div {...stylex.props(styles.heading)}>
        <span {...stylex.props(styles.headingLabel)}>{project ? "Edit project" : "New project"}</span>
        <Button type="button" size="sm" variant="ghost" label="Done" icon={<X aria-hidden="true" size={14} />} isIconOnly onClick={manager.closeEditor} />
      </div>
      <form
        {...stylex.props(styles.form)}
        onSubmit={(event) => {
          event.preventDefault();
          void manager.save();
        }}
      >
        <label {...stylex.props(styles.field)}>
          <span>Name</span>
          <input required value={manager.name} {...stylex.props(styles.input)} onChange={(event) => manager.setName(event.currentTarget.value)} />
        </label>
        <label {...stylex.props(styles.field)}>
          <span>Description</span>
          <textarea rows={2} value={manager.description} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => manager.setDescription(event.currentTarget.value)} />
        </label>
        {manager.stale ? (
          <div role="alert" {...stylex.props(styles.stale)}>
            <span>This project changed elsewhere. Review the latest version before saving.</span>
            <Button type="button" size="sm" variant="secondary" label="Review latest" isLoading={manager.acknowledging} isDisabled={manager.busy || manager.acknowledging || !manager.selected} onClick={() => void manager.acknowledgeLatest()} />
          </div>
        ) : null}
        {manager.error ? <p role="alert" {...stylex.props(styles.error)}>{manager.error}</p> : null}
        <div {...stylex.props(styles.actions)}>
          <Button type="submit" size="sm" variant="primary" label={project ? "Save" : "Create"} isLoading={manager.busy} isDisabled={manager.busy || manager.stale || !manager.name.trim()} />
        </div>
      </form>
    </section>
  );
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
}

const styles = stylex.create({
  root: {
    flexShrink: 0,
    maxHeight: "55%",
    overflowY: "auto",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    padding: "var(--spacing-2)",
    scrollbarWidth: "thin"
  },
  heading: {
    display: "flex",
    minHeight: 30,
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)"
  },
  headingLabel: {
    color: "var(--muted-foreground)",
    fontSize: 11,
    fontWeight: 600,
    letterSpacing: "0.04em",
    textTransform: "uppercase"
  },
  form: {
    display: "grid",
    gap: "var(--spacing-2)",
    paddingTop: "var(--spacing-2)"
  },
  field: {
    display: "grid",
    gap: 4,
    color: "var(--muted-foreground)",
    fontSize: 11,
    fontWeight: 600
  },
  input: {
    boxSizing: "border-box",
    width: "100%",
    minHeight: 30,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--background)",
    paddingInline: 8,
    color: "var(--foreground)",
    font: "inherit",
    fontSize: 12,
    fontWeight: 400,
    outline: "none",
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 1
    }
  },
  textarea: {
    minHeight: 48,
    paddingBlock: 7,
    resize: "vertical"
  },
  stale: {
    display: "grid",
    gap: 6,
    borderRadius: 6,
    backgroundColor: "var(--clay-50)",
    padding: 8,
    color: "var(--clay-700)",
    fontSize: 11,
    lineHeight: 1.35
  },
  error: {
    margin: 0,
    color: "var(--destructive)",
    fontSize: 11,
    lineHeight: 1.35
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: 6
  }
});
