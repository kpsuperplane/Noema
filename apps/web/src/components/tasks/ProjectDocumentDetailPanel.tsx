import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Pencil } from "lucide-react";
import { ErrorMarker } from "@/components/ErrorMarker";
import { MarkdownContent } from "@/components/MarkdownContent";
import { MarkdownDocumentInlineEditor } from "@/components/MarkdownDocumentInlineEditor";
import {
  TasksProjectDocumentDocument,
  TasksSaveProjectDocumentDocument,
  type TasksProjectDocumentQuery
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { isStaleCommandError } from "./semanticCommand";
import type { TasksProject } from "./tasksTypes";

type ProjectDocument = TasksProjectDocumentQuery["projectDocument"];

type Authority = {
  projectId: string;
  content: string;
  digest: string;
  revision: number;
};

export function ProjectDocumentDetailPanel({
  project,
  onProjectRefresh
}: {
  project?: TasksProject;
  onProjectRefresh?: () => Promise<readonly TasksProject[]>;
}) {
  const projectId = project?.projectId ?? "";
  const documentResult = useQuery(TasksProjectDocumentDocument, {
    variables: { projectId },
    fetchPolicy: "network-only",
    skip: !projectId
  });
  const document = documentResult.data?.projectDocument;
  const refetch = documentResult.refetch;
  const refreshDocument = React.useCallback(async () => {
    const latest = (await refetch()).data?.projectDocument;
    if (!latest) throw new Error("Latest project context is unavailable.");
    return latest;
  }, [refetch]);

  if (!project || !document || document.projectId !== projectId) {
    if (documentResult.error) {
      return (
        <VStack align="center" justify="center" gap={3} className={stylex.props(styles.state).className}>
          <ErrorMarker message="Project context could not load." recoverable={false} />
          <Button size="sm" variant="secondary" label="Retry" onClick={() => void documentResult.refetch()} />
        </VStack>
      );
    }
    return <p role="status" {...stylex.props(styles.state)}>Loading project context…</p>;
  }

  return (
    <LoadedProjectDocumentDetailPanel
      key={projectId}
      project={project}
      document={document}
      onDocumentRefresh={refreshDocument}
      onProjectRefresh={onProjectRefresh}
    />
  );
}

function LoadedProjectDocumentDetailPanel({
  project,
  document,
  onDocumentRefresh,
  onProjectRefresh
}: {
  project: TasksProject;
  document: ProjectDocument;
  onDocumentRefresh: () => Promise<ProjectDocument>;
  onProjectRefresh?: () => Promise<readonly TasksProject[]>;
}) {
  const projectId = project.projectId;
  const [saveDocument, saveResult] = useMutation(TasksSaveProjectDocumentDocument);
  const [authority, setAuthority] = React.useState<Authority>(() => ({
    projectId: document.projectId,
    content: document.content,
    digest: document.digest,
    revision: project.revision
  }));
  const [editAuthority, setEditAuthority] = React.useState<Authority | null>(null);
  const [acknowledging, setAcknowledging] = React.useState(false);
  const [stale, setStale] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const refreshingRef = React.useRef(false);

  const loadLatest = React.useCallback(async () => {
    const [latestDocument, projects] = await Promise.all([
      onDocumentRefresh(),
      onProjectRefresh?.() ?? Promise.resolve([])
    ]);
    const latestProject = projects.find((candidate) => candidate.projectId === projectId) ?? project;
    if (latestDocument.projectId !== projectId) {
      throw new Error("Latest project context is unavailable.");
    }
    const latest = {
      projectId: latestDocument.projectId,
      content: latestDocument.content,
      digest: latestDocument.digest,
      revision: latestProject.revision
    };
    setAuthority(latest);
    setError(null);
    return latest;
  }, [onDocumentRefresh, onProjectRefresh, project, projectId]);

  React.useEffect(() => {
    if (project.revision <= authority.revision || editAuthority || refreshingRef.current) return;
    refreshingRef.current = true;
    void loadLatest()
      .catch((caught: unknown) => setError(caught instanceof Error ? caught.message : "Latest project context could not load."))
      .finally(() => {
        refreshingRef.current = false;
      });
  }, [authority.revision, editAuthority, loadLatest, project.revision]);

  const cancelEdit = React.useCallback(() => {
    setEditAuthority(null);
    setStale(false);
    setError(null);
  }, []);

  const acknowledge = React.useCallback(async () => {
    setAcknowledging(true);
    try {
      const latest = await loadLatest();
      setEditAuthority(latest);
      setStale(false);
      setError(null);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Latest project context could not load.");
      throw caught;
    } finally {
      setAcknowledging(false);
    }
  }, [loadLatest]);

  const save = React.useCallback(async (content: string) => {
    if (!editAuthority || stale || project.revision !== editAuthority.revision || project.archivedAt) return;
    setError(null);
    try {
      const response = await saveDocument({
        variables: {
          input: {
            projectId,
            expectedRevision: editAuthority.revision,
            expectedDocumentDigest: editAuthority.digest,
            content,
            clientMutationId: createClientId()
          }
        }
      });
      const saved = response.data?.updateProjectDocument;
      if (!saved) throw new Error("Project context could not be saved.");
      setAuthority({
        projectId: saved.document.projectId,
        content: saved.document.content,
        digest: saved.document.digest,
        revision: saved.project.revision
      });
      setEditAuthority(null);
      setStale(false);
    } catch (caught) {
      if (isStaleCommandError(caught)) {
        setStale(true);
        setError("Changed elsewhere. Reload the latest version. Your draft is safe.");
      } else {
        setError(caught instanceof Error ? caught.message : "Project context could not be saved.");
      }
      throw caught;
    }
  }, [editAuthority, project.archivedAt, project.revision, projectId, saveDocument, stale]);

  const archived = Boolean(project.archivedAt);
  const requiresAcknowledgement = Boolean(editAuthority && (
    stale || project.revision !== editAuthority.revision
  ));
  return (
    <VStack as="section" aria-label="Project context document" gap={3} className={stylex.props(styles.panel).className}>
      {archived ? <p {...stylex.props(styles.notice)}>Reopen this project to edit its context.</p> : null}
      {editAuthority ? (
        <MarkdownDocumentInlineEditor
          document={editAuthority.content}
          digest={editAuthority.digest}
          edit={{
            busy: saveResult.loading || acknowledging,
            error,
            requiresAcknowledgement,
            actionUnavailable: archived,
            cancel: cancelEdit,
            acknowledge
          }}
          label="Project document"
          title="PROJECT.md"
          saveLabel="Save PROJECT.md"
          onSave={save}
        />
      ) : (
        <>
          {!archived ? (
            <HStack justify="end" align="center" className={stylex.props(styles.controls).className}>
              <IconButton type="button" size="sm" variant="ghost" label="Edit PROJECT.md" tooltip="Edit project context" icon={<Pencil aria-hidden="true" size={14} />} onClick={() => {
                setEditAuthority(authority);
                setError(null);
                setStale(false);
              }} />
            </HStack>
          ) : null}
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
          <MarkdownContent density="compact" className={stylex.props(styles.markdown).className}>{authority.content}</MarkdownContent>
        </>
      )}
    </VStack>
  );
}

const styles = stylex.create({
  panel: { minWidth: 0, minHeight: 0, padding: "var(--spacing-4)", overflowY: "auto" },
  controls: { minHeight: 32 },
  state: { margin: "var(--spacing-0)", paddingBlock: "var(--spacing-8)", color: "var(--noema-text-muted)", fontSize: 13 },
  notice: { margin: "var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: 12 },
  error: { color: "var(--destructive)", fontSize: 12 },
  markdown: { minWidth: 0 }
});
