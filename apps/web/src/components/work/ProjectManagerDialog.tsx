import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";
import { WorkArchiveProjectDocument, WorkCreateProjectDocument, WorkReopenProjectDocument, WorkUpdateProjectDocument } from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { WorkProject } from "./workTypes";
import { isStaleCommandError } from "./semanticCommand";

export function ProjectManagerDialog({ open, projects, onOpenChange, onUpdated }: { open: boolean; projects: readonly WorkProject[]; onOpenChange: (open: boolean) => void; onUpdated: () => void | Promise<void> }) {
  const [selectedId, setSelectedId] = React.useState("new");
  const [selected, setSelected] = React.useState<WorkProject | null>(null);
  const [name, setName] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [requiresAcknowledgement, setRequiresAcknowledgement] = React.useState(false);
  const [acknowledging, setAcknowledging] = React.useState(false);
  const wasOpenRef = React.useRef(false);
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
    if (open && !wasOpenRef.current) {
      const latest = selectedId === "new"
        ? null
        : projects.find((project) => project.projectId === selectedId) ?? null;
      setSelected(latest);
      setSelectedId(latest?.projectId ?? "new");
      setName(latest?.name ?? "");
      setDescription(latest?.description ?? "");
      setError(null);
      setRequiresAcknowledgement(false);
    }
    wasOpenRef.current = open;
  }, [open, projects, selectedId]);

  const selectProject = (project: WorkProject | null) => {
    setSelectedId(project?.projectId ?? "new");
    setSelected(project ? { ...project } : null);
    setName(project?.name ?? "");
    setDescription(project?.description ?? "");
    setError(null);
    setRequiresAcknowledgement(false);
  };

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
        const result = await update({ variables: { input: { projectId: selected.projectId, expectedRevision: selected.revision, name: name.trim(), description: description.trim(), clientMutationId: createClientId() } } });
        if (result.data?.updateProject.project) setSelected(result.data.updateProject.project);
      } else {
        await create({ variables: { input: { workspaceId: "workspace:personal", name: name.trim(), description: description.trim(), clientMutationId: createClientId() } } });
      }
      try {
        await onUpdated();
      } catch {
        // The committed mutation remains authoritative; the dialog keeps its returned snapshot.
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
      const input = { projectId: selected.projectId, expectedRevision: selected.revision, clientMutationId: createClientId() };
      const project = selected.archivedAt
        ? (await reopen({ variables: { input } })).data?.reopenProject.project
        : (await archive({ variables: { input } })).data?.archiveProject.project;
      if (project) setSelected(project);
      try {
        await onUpdated();
      } catch {
        // The committed mutation remains authoritative; the dialog keeps its returned snapshot.
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
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={640} aria-label="Manage projects">
      <div {...stylex.props(styles.dialog)}><DialogHeader title="Projects" subtitle="Projects organize work; archiving one never cancels its tasks." onOpenChange={onOpenChange} />
        <div {...stylex.props(styles.layout)}>
          <nav aria-label="Projects" {...stylex.props(styles.list)}><button type="button" data-selected={selectedId === "new"} {...stylex.props(styles.projectButton, selectedId === "new" && styles.selectedButton)} onClick={() => selectProject(null)}>New project</button>{projects.map((project) => <button key={project.projectId} type="button" data-selected={selectedId === project.projectId} {...stylex.props(styles.projectButton, selectedId === project.projectId && styles.selectedButton)} onClick={() => selectProject(project)}><span>{project.name}</span>{project.archivedAt ? <small {...stylex.props(styles.archived)}>Archived</small> : null}</button>)}</nav>
          <form {...stylex.props(styles.form)} onSubmit={(event) => { event.preventDefault(); void save(); }}>
            <label {...stylex.props(styles.field)}><span>Name</span><input autoFocus required value={name} {...stylex.props(styles.input)} onChange={(event) => setName(event.currentTarget.value)} /></label>
            <label {...stylex.props(styles.field)}><span>Description</span><textarea rows={5} value={description} {...stylex.props(styles.input)} onChange={(event) => setDescription(event.currentTarget.value)} /></label>
            {stale ? <div role="alert" {...stylex.props(styles.stale)}><span>This project changed while the dialog was open. Your draft is preserved.</span><Button type="button" size="sm" variant="secondary" label="Review latest project" isLoading={acknowledging} isDisabled={busy || acknowledging || !liveSelected} onClick={() => void acknowledgeLatest()} /></div> : null}
            {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
            <div {...stylex.props(styles.actions)}>{selected ? <Button type="button" size="sm" variant={selected.archivedAt ? "secondary" : "destructive"} label={selected.archivedAt ? "Reopen project" : "Archive project"} isDisabled={busy || stale} onClick={() => void toggleArchived()} /> : null}<Button type="submit" size="sm" variant="primary" label={selected ? "Save project" : "Create project"} isLoading={busy} isDisabled={busy || stale || !name.trim()} /></div>
          </form>
        </div>
      </div>
    </Dialog>
  );
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
}

const styles = stylex.create({
  dialog: { display: "grid" }, layout: { display: "grid", gridTemplateColumns: "190px minmax(0, 1fr)", minHeight: 320, "@media (max-width: 620px)": { gridTemplateColumns: "1fr" } }, list: { display: "flex", flexDirection: "column", gap: 3, borderRightWidth: 1, borderRightStyle: "solid", borderRightColor: "var(--border-subtle)", padding: 12, "@media (max-width: 620px)": { maxHeight: 140, borderRightWidth: 0, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", overflowY: "auto" } }, projectButton: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, minHeight: 34, borderWidth: 0, borderRadius: 7, backgroundColor: "transparent", paddingInline: 9, color: "var(--foreground)", font: "inherit", fontSize: 12, textAlign: "left" }, selectedButton: { backgroundColor: "var(--pine-50)", color: "var(--pine-700)", fontWeight: 650 }, archived: { color: "var(--muted-foreground)", fontSize: 10 }, form: { display: "grid", alignContent: "start", gap: 16, padding: 20 }, field: { display: "grid", gap: 6, fontSize: 13, fontWeight: 600 }, input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", padding: 9, color: "var(--foreground)", font: "inherit" }, stale: { display: "grid", justifyItems: "start", gap: 8, borderRadius: 9, backgroundColor: "var(--clay-50)", padding: 10, color: "var(--clay-700)", fontSize: 12, lineHeight: 1.45 }, error: { margin: 0, color: "var(--destructive)", fontSize: 13 }, actions: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", gap: 8 }
});
