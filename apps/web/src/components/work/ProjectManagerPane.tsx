import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { FolderCog, Plus, X } from "lucide-react";
import {
  WorkArchiveProjectDocument,
  WorkCreateProjectDocument,
  WorkReopenProjectDocument,
  WorkUpdateProjectDocument
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { WorkProject } from "./workTypes";
import { isStaleCommandError } from "./semanticCommand";

export function ProjectManagerPane({
  projects,
  onUpdated
}: {
  projects: readonly WorkProject[];
  onUpdated: () => void | Promise<void>;
}) {
  const [open, setOpen] = React.useState(false);
  const [selectedId, setSelectedId] = React.useState("new");
  const [selected, setSelected] = React.useState<WorkProject | null>(null);
  const [name, setName] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [requiresAcknowledgement, setRequiresAcknowledgement] = React.useState(false);
  const [acknowledging, setAcknowledging] = React.useState(false);
  const autoOpenedRef = React.useRef(false);
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

  React.useEffect(() => {
    if (!autoOpenedRef.current && projects.length === 0) {
      setOpen(true);
      autoOpenedRef.current = true;
    }
  }, [projects.length]);

  const selectProject = React.useCallback((projectId: string) => {
    const project = projectId === "new"
      ? null
      : projects.find((candidate) => candidate.projectId === projectId) ?? null;
    setSelectedId(project?.projectId ?? "new");
    setSelected(project ? { ...project } : null);
    setName(project?.name ?? "");
    setDescription(project?.description ?? "");
    setError(null);
    setRequiresAcknowledgement(false);
  }, [projects]);

  const acknowledgeLatest = async () => {
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
      setRequiresAcknowledgement(false);
      setError(null);
    } catch {
      setError("The latest project version could not be loaded. Retry when project updates reconnect.");
    } finally {
      setAcknowledging(false);
    }
  };

  async function save() {
    if (stale) return;
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
        // The committed mutation remains authoritative; the pane keeps its returned snapshot.
      }
      if (!selected) {
        setSelectedId("new");
        setName("");
        setDescription("");
      }
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
  }

  async function toggleArchived() {
    if (!selected || stale) return;
    setError(null);
    try {
      const input = {
        projectId: selected.projectId,
        expectedRevision: selected.revision,
        clientMutationId: createClientId()
      };
      const project = selected.archivedAt
        ? (await reopen({ variables: { input } })).data?.reopenProject.project
        : (await archive({ variables: { input } })).data?.archiveProject.project;
      if (project) setSelected(project);
      try {
        await onUpdated();
      } catch {
        // The committed mutation remains authoritative; the pane keeps its returned snapshot.
      }
    } catch (caught) {
      if (isStaleCommandError(caught)) {
        try {
          await onUpdated();
        } catch {
          // The acknowledgement control uses the next available authoritative project page.
        }
        setRequiresAcknowledgement(true);
        setError("This project changed elsewhere. Review the latest version before updating it.");
      } else {
        setError(caught instanceof Error ? caught.message : "Project could not be updated.");
      }
    }
  }

  return (
    <section
      aria-label="Project management"
      data-slot="work-project-manager"
      {...stylex.props(styles.root, open && styles.rootOpen)}
    >
      <div {...stylex.props(styles.heading)}>
        <span {...stylex.props(styles.headingLabel)}>Projects</span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          label={open ? "Done" : projects.length > 0 ? "Manage projects" : "New project"}
          icon={open ? <X aria-hidden="true" size={14} /> : projects.length > 0 ? <FolderCog aria-hidden="true" size={14} /> : <Plus aria-hidden="true" size={14} />}
          onClick={() => setOpen((current) => !current)}
        />
      </div>
      {open ? (
        <form
          {...stylex.props(styles.form)}
          onSubmit={(event) => {
            event.preventDefault();
            void save();
          }}
        >
          <label {...stylex.props(styles.field)}>
            <span>Project</span>
            <select value={selectedId} {...stylex.props(styles.input)} onChange={(event) => selectProject(event.currentTarget.value)}>
              <option value="new">New project</option>
              {projects.map((project) => (
                <option key={project.projectId} value={project.projectId}>
                  {project.name}{project.archivedAt ? " (archived)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label {...stylex.props(styles.field)}>
            <span>Name</span>
            <input required value={name} {...stylex.props(styles.input)} onChange={(event) => setName(event.currentTarget.value)} />
          </label>
          <label {...stylex.props(styles.field)}>
            <span>Description</span>
            <textarea rows={2} value={description} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => setDescription(event.currentTarget.value)} />
          </label>
          {stale ? (
            <div role="alert" {...stylex.props(styles.stale)}>
              <span>This project changed elsewhere. Review the latest version before saving.</span>
              <Button type="button" size="sm" variant="secondary" label="Review latest" isLoading={acknowledging} isDisabled={busy || acknowledging || !liveSelected} onClick={() => void acknowledgeLatest()} />
            </div>
          ) : null}
          {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
          <div {...stylex.props(styles.actions)}>
            {selected ? <Button type="button" size="sm" variant={selected.archivedAt ? "secondary" : "destructive"} label={selected.archivedAt ? "Reopen" : "Archive"} isDisabled={busy || stale} onClick={() => void toggleArchived()} /> : null}
            <Button type="submit" size="sm" variant="primary" label={selected ? "Save" : "Create"} isLoading={busy} isDisabled={busy || stale || !name.trim()} />
          </div>
        </form>
      ) : null}
    </section>
  );
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
}

const styles = stylex.create({
  root: {
    flexShrink: 0,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    padding: "var(--spacing-2)"
  },
  rootOpen: {
    maxHeight: "55%",
    overflowY: "auto",
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
