import * as React from "react";
import { useMutation } from "@apollo/client/react";
import {
  WorkArchiveProjectDocument,
  WorkCreateProjectDocument,
  WorkReopenProjectDocument,
  WorkUpdateProjectDocument
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { isStaleCommandError } from "./semanticCommand";
import type { WorkProject } from "./workTypes";

type ProjectEditor =
  | { kind: "create" }
  | { kind: "edit"; projectId: string };

export type ProjectManagerController = {
  editor: ProjectEditor | null;
  name: string;
  folder: string;
  busy: boolean;
  error: string | null;
  openCreate: () => void;
  openRename: (projectId: string) => void;
  closeEditor: () => void;
  setName: (name: string) => void;
  setFolder: (folder: string) => void;
  save: () => Promise<void>;
  toggleArchived: (projectId: string) => Promise<void>;
};

export function useProjectManager({
  projects,
  onUpdated
}: {
  projects: readonly WorkProject[];
  onUpdated: () => void | Promise<void>;
}): ProjectManagerController {
  const [editor, setEditor] = React.useState<ProjectEditor | null>(null);
  const [name, setName] = React.useState("");
  const [folder, setFolder] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [create, createState] = useMutation(WorkCreateProjectDocument);
  const [update, updateState] = useMutation(WorkUpdateProjectDocument);
  const [archive, archiveState] = useMutation(WorkArchiveProjectDocument);
  const [reopen, reopenState] = useMutation(WorkReopenProjectDocument);
  const busy = createState.loading || updateState.loading || archiveState.loading || reopenState.loading;

  const closeEditor = React.useCallback(() => {
    setEditor(null);
    setName("");
    setFolder("");
    setError(null);
  }, []);

  const openCreate = React.useCallback(() => {
    setEditor({ kind: "create" });
    setName("");
    setFolder("");
    setError(null);
  }, []);

  const openRename = React.useCallback((projectId: string) => {
    const project = projects.find((candidate) => candidate.projectId === projectId);
    if (!project) return;
    setEditor({ kind: "edit", projectId });
    setName(project.name);
    setFolder(project.folder ?? "");
    setError(null);
  }, [projects]);

  const save = React.useCallback(async () => {
    const nextName = name.trim();
    if (!editor || !nextName) return;
    setError(null);
    try {
      if (editor.kind === "create") {
        await create({
          variables: {
            input: {
              workspaceId: "workspace:personal",
              name: nextName,
              description: "",
              folder: folder.trim() || null,
              clientMutationId: createClientId()
            }
          }
        });
      } else {
        const project = projects.find((candidate) => candidate.projectId === editor.projectId);
        if (!project) {
          setError("This project is no longer available.");
          return;
        }
        await update({
          variables: {
            input: {
              projectId: project.projectId,
              expectedRevision: project.revision,
              name: nextName,
              description: project.description,
              folder: folder.trim() || null,
              clearFolder: !folder.trim(),
              clientMutationId: createClientId()
            }
          }
        });
      }
      try {
        await onUpdated();
      } catch {
        // The committed mutation remains authoritative.
      }
      closeEditor();
    } catch (caught) {
      if (isStaleCommandError(caught)) {
        try {
          await onUpdated();
        } catch {
          // The inline editor can retry when project updates reconnect.
        }
        setError("This project changed elsewhere. Review the name and save again.");
      } else {
        setError(caught instanceof Error ? caught.message : "Project could not be saved.");
      }
    }
  }, [closeEditor, create, editor, folder, name, onUpdated, projects, update]);

  const toggleArchived = React.useCallback(async (projectId: string) => {
    const project = projects.find((candidate) => candidate.projectId === projectId);
    if (!project) return;
    setError(null);
    try {
      const input = {
        projectId: project.projectId,
        expectedRevision: project.revision,
        clientMutationId: createClientId()
      };
      if (project.archivedAt) {
        await reopen({ variables: { input } });
      } else {
        await archive({ variables: { input } });
      }
      try {
        await onUpdated();
      } catch {
        // The committed mutation remains authoritative.
      }
    } catch (caught) {
      openRename(projectId);
      if (isStaleCommandError(caught)) {
        try {
          await onUpdated();
        } catch {
          // The inline editor can retry when project updates reconnect.
        }
        setError("This project changed elsewhere. Retry the action from its menu.");
      } else {
        setError(caught instanceof Error ? caught.message : "Project could not be updated.");
      }
    }
  }, [archive, onUpdated, openRename, projects, reopen]);

  return {
    editor,
    name,
    folder,
    busy,
    error,
    openCreate,
    openRename,
    closeEditor,
    setName,
    setFolder,
    save,
    toggleArchived
  };
}
