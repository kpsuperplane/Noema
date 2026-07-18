import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";
import { WorkCaptureTaskDocument } from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { WorkProject } from "./workTypes";

export function CaptureTaskDialog({ open, projects, initialProjectId, onOpenChange, onCreated }: { open: boolean; projects: readonly WorkProject[]; initialProjectId?: string; onOpenChange: (open: boolean) => void; onCreated: () => void | Promise<void> }) {
  const [title, setTitle] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [projectId, setProjectId] = React.useState(initialProjectId ?? "");
  const [capture, state] = useMutation(WorkCaptureTaskDocument);

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={580} aria-label="New task">
      <div {...stylex.props(styles.dialog)}>
        <DialogHeader title="New task" subtitle="Capture an Inbox item. Nothing runs until you choose Queue." onOpenChange={onOpenChange} />
        <form {...stylex.props(styles.form)} onSubmit={(event) => { event.preventDefault(); void capture({ variables: { input: { workspaceId: "workspace:personal", projectId: projectId || null, title: title.trim(), description: description.trim(), clientMutationId: createClientId() } } }).then(async () => { setTitle(""); setDescription(""); await onCreated(); onOpenChange(false); }).catch(() => undefined); }}>
          <label {...stylex.props(styles.field)}><span>Title</span><input autoFocus required value={title} {...stylex.props(styles.input)} onChange={(event) => setTitle(event.currentTarget.value)} /></label>
          <label {...stylex.props(styles.field)}><span>Description</span><textarea rows={6} value={description} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => setDescription(event.currentTarget.value)} /></label>
          <label {...stylex.props(styles.field)}><span>Project</span><select value={projectId} {...stylex.props(styles.input)} onChange={(event) => setProjectId(event.currentTarget.value)}><option value="">No project</option>{projects.filter((project) => !project.archivedAt).map((project) => <option key={project.projectId} value={project.projectId}>{project.name}</option>)}</select></label>
          {state.error ? <p role="alert" {...stylex.props(styles.error)}>{state.error.message}</p> : null}
          <div {...stylex.props(styles.actions)}><Button type="button" size="sm" variant="ghost" label="Cancel" onClick={() => onOpenChange(false)} /><Button type="submit" size="sm" variant="primary" label="Capture task" isLoading={state.loading} isDisabled={state.loading || !title.trim()} /></div>
        </form>
      </div>
    </Dialog>
  );
}

const styles = stylex.create({
  dialog: { display: "grid" }, form: { display: "grid", gap: 16, padding: 20 }, field: { display: "grid", gap: 6, fontSize: 13, fontWeight: 600 }, input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingBlock: 8, paddingInline: 10, color: "var(--foreground)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } }, textarea: { resize: "vertical", lineHeight: 1.5 }, error: { margin: 0, color: "var(--destructive)", fontSize: 13 }, actions: { display: "flex", justifyContent: "flex-end", gap: 8 }
});
