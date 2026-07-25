import * as React from "react";
import { useApolloClient, useSubscription } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { WorkEventsDocument } from "@/generated/graphql";
import { ChatDetailRail } from "../chatDetail/ChatDetailRail";
import type { WorkSearch } from "./workTypes";
import { CaptureTaskDialog } from "./CaptureTaskDialog";
import { ProjectManagerDialog } from "./ProjectManagerDialog";
import { WorkFolderRail } from "./WorkFolderRail";
import { WorkToolbar } from "./WorkToolbar";
import { WorkTasks } from "./WorkViews";
import { useWorkEventCursor } from "./workEventCursor";
import { PERSONAL_WORKSPACE_ID } from "./workTypes";
import { useAllWorkProjects } from "./useAllWorkProjects";
import { useWorkEventInvalidation } from "./useWorkEventInvalidation";

export function WorkSurface({ search, onSearchChange, selectedTaskId, onCloseTask }: {
  search: WorkSearch;
  onSearchChange: (next: WorkSearch, replace?: boolean) => void;
  selectedTaskId?: string;
  onCloseTask?: () => void;
}) {
  const client = useApolloClient();
  const projectsResult = useAllWorkProjects();
  const projects = projectsResult.projects;
  const hasFolderNavigation = projects.length > 0;
  const [captureOpen, setCaptureOpen] = React.useState(false);
  const [projectsOpen, setProjectsOpen] = React.useState(false);
  const [foldersOpen, setFoldersOpen] = React.useState(false);
  const [queryDraftState, setQueryDraftState] = React.useState(() => ({ source: search.q, value: search.q ?? "" }));
  const queryDraft = queryDraftState.source === search.q ? queryDraftState.value : search.q ?? "";
  const setQueryDraft = React.useCallback((value: string) => setQueryDraftState({ source: search.q, value }), [search.q]);
  const [eventCursor, recordEventCursor] = useWorkEventCursor(PERSONAL_WORKSPACE_ID);
  const seenEventIdsRef = React.useRef(new Set<string>());
  const scheduleInvalidation = useWorkEventInvalidation({
    client,
    refetchProjects: projectsResult.refetch
  });

  React.useEffect(() => {
    const timeout = window.setTimeout(() => {
      const q = queryDraft.trim() || undefined;
      if (q !== search.q) onSearchChange({ ...search, q }, true);
    }, 250);
    return () => window.clearTimeout(timeout);
  }, [onSearchChange, queryDraft, search]);
  const subscription = useSubscription(WorkEventsDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, after: eventCursor },
    onData: ({ data }) => {
      const event = data.data?.workEvents;
      if (!event || event.cursor === eventCursor || seenEventIdsRef.current.has(event.eventId)) return;
      seenEventIdsRef.current.add(event.eventId);
      if (seenEventIdsRef.current.size > 512) {
        const oldest = seenEventIdsRef.current.values().next().value;
        if (oldest) seenEventIdsRef.current.delete(oldest);
      }
      recordEventCursor(event.cursor);
      scheduleInvalidation(event);
    }
  });

  const refresh = React.useCallback(async () => {
    await Promise.allSettled([
      projectsResult.refetch(),
      client.refetchQueries({ include: "active" })
    ]);
  }, [client, projectsResult]);

  const hasNotice = Boolean(subscription.error || projectsResult.error);
  const selectedProject = projects.find((project) => project.projectId === search.project);
  const folderNavOpen = foldersOpen && hasFolderNavigation;
  const selectProject = React.useCallback((project?: string) => {
    setFoldersOpen(false);
    onSearchChange({ ...search, project });
  }, [onSearchChange, search]);

  React.useEffect(() => {
    if (!folderNavOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setFoldersOpen(false);
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [folderNavOpen]);

  return (
    <section aria-labelledby="work-page-title" {...stylex.props(styles.surface, !hasFolderNavigation && styles.surfaceNoFolders)}>
      {hasFolderNavigation ? (
        <WorkFolderRail
          projectId={search.project}
          projects={projects}
          open={folderNavOpen}
          onProjectChange={selectProject}
          onManageProjects={() => {
            setFoldersOpen(false);
            setProjectsOpen(true);
          }}
        />
      ) : null}
      {folderNavOpen ? (
        <button
          type="button"
          aria-label="Close task folders"
          {...stylex.props(styles.folderBackdrop)}
          onClick={() => setFoldersOpen(false)}
        />
      ) : null}
      <div {...stylex.props(styles.listPane, !hasFolderNavigation && styles.listPaneNoFolders, Boolean(selectedTaskId) && styles.listPaneDetailOpen)}>
        <WorkToolbar
          folderLabel={selectedProject?.name ?? "All tasks"}
          foldersAvailable={hasFolderNavigation}
          foldersOpen={folderNavOpen}
          queryDraft={queryDraft}
          terminal={search.terminal ?? "all"}
          onOpenFolders={() => setFoldersOpen(true)}
          onQueryChange={setQueryDraft}
          onTerminalChange={(terminal) => onSearchChange({ ...search, terminal })}
          onNewTask={() => setCaptureOpen(true)}
        />
        {hasNotice ? <div {...stylex.props(styles.notices)}>
          <span aria-live="polite" {...stylex.props(styles.live)}>{subscription.error ? "Updating tasks. Reconnecting." : ""}</span>
          {projectsResult.error ? <button type="button" {...stylex.props(styles.refresh)} onClick={() => void projectsResult.retry()}>Project information could not refresh. Retry</button> : null}
        </div> : null}
        <main aria-label="Tasks" {...stylex.props(styles.panel)}>
          <WorkTasks
            projectId={search.project}
            query={search.q}
            terminal={search.terminal ?? "all"}
          />
        </main>
      </div>
      <div role="region" aria-label="Task details" {...stylex.props(styles.detailPane, !hasFolderNavigation && styles.detailPaneNoFolders, Boolean(selectedTaskId) && styles.detailPaneOpen)}>
        {selectedTaskId ? (
          <ChatDetailRail
            target={{ type: "task", taskId: selectedTaskId }}
            onClose={() => onCloseTask?.()}
            showWorkLink={false}
          />
        ) : null}
      </div>
      <CaptureTaskDialog key={`${search.project ?? "all"}:${captureOpen ? "open" : "closed"}`} open={captureOpen} projects={projects} initialProjectId={search.project} onOpenChange={setCaptureOpen} onCreated={refresh} />
      <ProjectManagerDialog open={projectsOpen} projects={projects} onOpenChange={setProjectsOpen} onUpdated={refresh} />
    </section>
  );
}

const styles = stylex.create({
  surface: {
    display: "grid",
    position: "relative",
    gridTemplateColumns: "minmax(0, 1fr)",
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    backgroundColor: "var(--noema-surface-card)",
    "@media (min-width: 980px)": { gridTemplateColumns: "minmax(0, 1fr) 440px" },
    "@media (min-width: 1180px)": { gridTemplateColumns: "220px minmax(0, 1fr) 440px" }
  },
  surfaceNoFolders: {
    "@media (min-width: 1180px)": { gridTemplateColumns: "minmax(0, 1fr) 440px" }
  },
  listPane: {
    display: "flex",
    gridColumn: 1,
    minWidth: 0,
    minHeight: 0,
    flexDirection: "column",
    overflow: "hidden",
    "@media (min-width: 1180px)": { gridColumn: 2 }
  },
  listPaneNoFolders: {
    "@media (min-width: 1180px)": { gridColumn: 1 }
  },
  listPaneDetailOpen: {
    "@media (max-width: 979px)": { visibility: "hidden", pointerEvents: "none" }
  },
  folderBackdrop: {
    position: "absolute",
    inset: 0,
    zIndex: 5,
    borderWidth: 0,
    backgroundColor: "color-mix(in srgb, black 14%, transparent)",
    cursor: "default",
    "@media (min-width: 1180px)": { display: "none" }
  },
  detailPane: {
    display: "none",
    position: "relative",
    minWidth: 0,
    minHeight: 0,
    borderLeftWidth: 1,
    borderLeftStyle: "solid",
    borderLeftColor: "var(--noema-border-subtle)",
    "--chat-detail-rail-width": "100%",
    "@media (min-width: 980px)": { display: "block", gridColumn: 2 },
    "@media (min-width: 1180px)": { gridColumn: 3 }
  },
  detailPaneNoFolders: {
    "@media (min-width: 1180px)": { gridColumn: 2 }
  },
  detailPaneOpen: {
    "@media (max-width: 979px)": {
      display: "block",
      position: "absolute",
      inset: 0,
      zIndex: 4,
      backgroundColor: "var(--noema-surface-card)"
    }
  },
  panel: { minHeight: 0, outline: "none", overflowY: "auto", overflowX: "hidden", scrollbarWidth: "thin" },
  notices: { display: "grid", gap: 4, paddingInline: 12, ":empty": { display: "none" } },
  live: { justifySelf: "end", paddingBlock: 3, color: "var(--noema-text-muted)", fontSize: 10, ":empty": { display: "none" } },
  refresh: { justifySelf: "start", borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-clay-700)", font: "inherit", fontSize: 11, textDecoration: "underline", cursor: "pointer" }
});
