import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { useBlocker } from "@tanstack/react-router";
import { Code2, Pencil } from "lucide-react";
import { ErrorMarker } from "@/components/ErrorMarker";
import { MarkdownContent } from "@/components/MarkdownContent";
import {
  TasksProjectDocumentDocument,
  TasksSaveProjectDocumentDocument
} from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import { TaskMarkdownEditor } from "./TaskMarkdownEditor";
import { isStaleCommandError } from "./semanticCommand";
import type { TasksProject } from "./tasksTypes";

type Authority = {
  content: string;
  digest: string;
  revision: number;
};

type SaveState = "saving" | "saved" | "error" | "conflict";

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
    fetchPolicy: "cache-and-network",
    skip: !projectId
  });
  const [saveDocument] = useMutation(TasksSaveProjectDocumentDocument);
  const [authority, setAuthority] = React.useState<Authority | null>(null);
  const [draft, setDraft] = React.useState("");
  const [editing, setEditing] = React.useState(false);
  const [sourceMode, setSourceMode] = React.useState(false);
  const [saveState, setSaveState] = React.useState<SaveState>("saved");
  const [error, setError] = React.useState<string | null>(null);
  const [conflict, setConflict] = React.useState<Authority | null>(null);
  const authorityRef = React.useRef<Authority | null>(null);
  const draftRef = React.useRef("");
  const conflictRef = React.useRef<Authority | null>(null);
  const queuedRef = React.useRef(false);
  const runnerRef = React.useRef<Promise<boolean> | null>(null);
  const refreshingRef = React.useRef(false);
  const initializedRef = React.useRef(false);

  React.useEffect(() => {
    const document = documentResult.data?.projectDocument;
    if (!document || !project || initializedRef.current) return;
    const initial = {
      content: document.content,
      digest: document.digest,
      revision: project.revision
    };
    initializedRef.current = true;
    authorityRef.current = initial;
    draftRef.current = initial.content;
    setAuthority(initial);
    setDraft(initial.content);
  }, [documentResult.data?.projectDocument, project]);

  const loadLatest = React.useCallback(async (forConflict: boolean) => {
    const [documentResponse, projects] = await Promise.all([
      documentResult.refetch(),
      onProjectRefresh?.() ?? Promise.resolve([])
    ]);
    const document = documentResponse.data?.projectDocument;
    const latestProject = projects.find((candidate) => candidate.projectId === projectId);
    if (!document || !latestProject) throw new Error("Latest project context is unavailable.");
    const latest = {
      content: document.content,
      digest: document.digest,
      revision: latestProject.revision
    };
    authorityRef.current = latest;
    setAuthority(latest);
    if (forConflict) {
      conflictRef.current = latest;
      setConflict(latest);
      setSaveState("conflict");
    } else {
      draftRef.current = latest.content;
      setDraft(latest.content);
      setSaveState("saved");
    }
    return latest;
  }, [documentResult, onProjectRefresh, projectId]);

  React.useEffect(() => {
    const current = authorityRef.current;
    if (!current || !project || project.revision <= current.revision) return;
    if (draftRef.current !== current.content || runnerRef.current || refreshingRef.current) return;
    refreshingRef.current = true;
    void loadLatest(false)
      .catch((caught: unknown) => {
        setSaveState("error");
        setError(caught instanceof Error ? caught.message : "Latest project context could not load.");
      })
      .finally(() => {
        refreshingRef.current = false;
      });
  }, [loadLatest, project]);

  const requestSave = React.useCallback((): Promise<boolean> => {
    queuedRef.current = true;
    if (runnerRef.current) return runnerRef.current;
    const runner = (async () => {
      while (queuedRef.current) {
        queuedRef.current = false;
        const current = authorityRef.current;
        if (!current || conflictRef.current) return false;
        const content = draftRef.current;
        if (content === current.content) continue;
        setSaveState("saving");
        setError(null);
        try {
          const response = await saveDocument({
            variables: {
              input: {
                projectId,
                expectedRevision: current.revision,
                expectedDocumentDigest: current.digest,
                content,
                clientMutationId: createClientId()
              }
            }
          });
          const saved = response.data?.updateProjectDocument;
          if (!saved) throw new Error("Project context could not be saved.");
          const next = {
            content: saved.document.content,
            digest: saved.document.digest,
            revision: saved.project.revision
          };
          authorityRef.current = next;
          setAuthority(next);
          setSaveState("saved");
          if (draftRef.current !== content) queuedRef.current = true;
        } catch (caught) {
          if (isStaleCommandError(caught)) {
            try {
              await loadLatest(true);
            } catch (reloadError) {
              setSaveState("error");
              setError(reloadError instanceof Error ? reloadError.message : "Latest project context could not load.");
            }
          } else {
            setSaveState("error");
            setError(caught instanceof Error ? caught.message : "Project context could not be saved.");
          }
          return false;
        }
      }
      return true;
    })();
    runnerRef.current = runner.finally(() => {
      runnerRef.current = null;
    });
    return runnerRef.current;
  }, [loadLatest, projectId, saveDocument]);

  const dirty = Boolean(authority && draft !== authority.content);
  React.useEffect(() => {
    if (!editing || !dirty || conflict) return;
    const timer = window.setTimeout(() => void requestSave(), 600);
    return () => window.clearTimeout(timer);
  }, [conflict, dirty, draft, editing, requestSave]);

  useBlocker({
    shouldBlockFn: async () => !(await requestSave()),
    enableBeforeUnload: () => dirty || saveState === "saving",
    disabled: !dirty && saveState !== "saving" && saveState !== "conflict"
  });

  const changeDraft = React.useCallback((content: string) => {
    draftRef.current = content;
    setDraft(content);
    if (runnerRef.current) queuedRef.current = true;
  }, []);

  if (!project || (!authority && documentResult.loading)) {
    return <p role="status" {...stylex.props(styles.state)}>Loading project context…</p>;
  }
  if (!authority || documentResult.error) {
    return (
      <VStack align="center" justify="center" gap={3} className={stylex.props(styles.state).className}>
        <ErrorMarker message="Project context could not load." recoverable={false} />
        <Button size="sm" variant="secondary" label="Retry" onClick={() => void documentResult.refetch()} />
      </VStack>
    );
  }

  const archived = Boolean(project.archivedAt);
  return (
    <VStack as="section" aria-label="Project context document" gap={3} className={stylex.props(styles.panel).className}>
      <HStack justify="between" align="center" gap={2} className={stylex.props(styles.controls).className}>
        <span aria-live="polite" {...stylex.props(styles.status, saveState === "error" && styles.errorText)}>
          {saveState === "saving" ? "Saving" : saveState === "saved" ? "Saved" : saveState === "conflict" ? "Conflict" : "Could not save"}
        </span>
        <HStack align="center" gap={1}>
          {editing ? (
            <IconButton type="button" size="sm" variant="ghost" label={sourceMode ? "Use rich editor" : "Edit source"} tooltip={sourceMode ? "Use rich editor" : "Edit source"} icon={<Code2 aria-hidden="true" size={14} />} onClick={() => setSourceMode((current) => !current)} />
          ) : !archived ? (
            <IconButton type="button" size="sm" variant="ghost" label="Edit PROJECT.md" tooltip="Edit project context" icon={<Pencil aria-hidden="true" size={14} />} onClick={() => setEditing(true)} />
          ) : null}
        </HStack>
      </HStack>
      {archived ? <p {...stylex.props(styles.notice)}>Reopen this project to edit its context.</p> : null}
      {conflict ? (
        <VStack gap={2} className={stylex.props(styles.conflict).className}>
          <strong>PROJECT.md changed elsewhere.</strong>
          <span>The latest stored document is ready. Your draft remains unchanged.</span>
          <HStack gap={2} wrap="wrap">
            <Button size="sm" variant="secondary" label="Use latest" onClick={() => {
              conflictRef.current = null;
              setConflict(null);
              changeDraft(conflict.content);
              setSaveState("saved");
            }} />
            <Button size="sm" variant="primary" label="Keep my draft" onClick={() => {
              conflictRef.current = null;
              setConflict(null);
              void requestSave();
            }} />
          </HStack>
        </VStack>
      ) : null}
      {error ? (
        <HStack align="center" gap={2}>
          <span role="alert" {...stylex.props(styles.errorText)}>{error}</span>
          <Button size="sm" variant="ghost" label="Retry save" onClick={() => void requestSave()} />
        </HStack>
      ) : null}
      {editing && !archived ? (
        <TaskMarkdownEditor
          value={draft}
          onChange={changeDraft}
          label="Project document"
          sourceMode={sourceMode}
          onSourceModeChange={setSourceMode}
        />
      ) : (
        <MarkdownContent density="compact" className={stylex.props(styles.markdown).className}>{draft}</MarkdownContent>
      )}
    </VStack>
  );
}

const styles = stylex.create({
  panel: { minWidth: 0, minHeight: 0, padding: "var(--spacing-4)", overflowY: "auto" },
  controls: { minHeight: 32 },
  status: { color: "var(--noema-text-muted)", fontSize: 12 },
  state: { margin: "var(--spacing-0)", paddingBlock: "var(--spacing-8)", color: "var(--noema-text-muted)", fontSize: 13 },
  notice: { margin: "var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: 12 },
  conflict: { borderWidth: "var(--border-width)", borderStyle: "solid", borderColor: "var(--noema-border-default)", borderRadius: "var(--radius-element)", padding: "var(--spacing-3)", color: "var(--noema-text-secondary)", fontSize: 13 },
  errorText: { color: "var(--destructive)", fontSize: 12 },
  markdown: { minWidth: 0 }
});
