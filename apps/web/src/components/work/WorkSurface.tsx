import * as React from "react";
import { useApolloClient, useSubscription } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { WorkEventsDocument } from "@/generated/graphql";
import type { WorkSearch, WorkView } from "./workTypes";
import { CaptureTaskDialog } from "./CaptureTaskDialog";
import { ProjectManagerDialog } from "./ProjectManagerDialog";
import { WorkToolbar } from "./WorkToolbar";
import { WorkActivity, WorkArchive, WorkBoard, WorkList, WorkNeedsYou } from "./WorkViews";
import { useWorkEventCursor } from "./workEventCursor";
import { PERSONAL_WORKSPACE_ID } from "./workTypes";
import { useAllWorkProjects } from "./useAllWorkProjects";
import { useWorkEventInvalidation } from "./useWorkEventInvalidation";

export function WorkSurface({ search, onSearchChange }: { search: WorkSearch; onSearchChange: (next: WorkSearch, replace?: boolean) => void }) {
  const client = useApolloClient();
  const projectsResult = useAllWorkProjects();
  const projects = projectsResult.projects;
  const [captureOpen, setCaptureOpen] = React.useState(false);
  const [projectsOpen, setProjectsOpen] = React.useState(false);
  const [queryDraftState, setQueryDraftState] = React.useState(() => ({ source: search.q, value: search.q ?? "" }));
  const queryDraft = queryDraftState.source === search.q ? queryDraftState.value : search.q ?? "";
  const setQueryDraft = React.useCallback((value: string) => setQueryDraftState({ source: search.q, value }), [search.q]);
  const [eventCursor, recordEventCursor] = useWorkEventCursor(PERSONAL_WORKSPACE_ID);
  const seenEventIdsRef = React.useRef(new Set<string>());
  const scheduleInvalidation = useWorkEventInvalidation({
    client,
    view: search.view,
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

  const setView = (view: WorkView) => onSearchChange({ ...search, view });
  const hasNotice = Boolean(subscription.error || projectsResult.error);

  return (
    <section aria-labelledby="work-page-title" {...stylex.props(styles.surface, hasNotice && styles.surfaceWithNotice)}>
      <WorkToolbar
        view={search.view}
        projectId={search.project}
        query={queryDraft}
        projects={projects}
        onViewChange={setView}
        onProjectChange={(project) => onSearchChange({ ...search, project })}
        onQueryChange={setQueryDraft}
        onNewTask={() => setCaptureOpen(true)}
        onManageProjects={() => setProjectsOpen(true)}
      />
      {hasNotice ? <div {...stylex.props(styles.notices)}>
        <span aria-live="polite" {...stylex.props(styles.live)}>{subscription.error ? "Updating Work. Reconnecting." : ""}</span>
        {projectsResult.error ? <button type="button" {...stylex.props(styles.refresh)} onClick={() => void projectsResult.retry()}>Project information could not refresh. Retry</button> : null}
      </div> : null}
      <main id={`work-panel-${search.view}`} role="tabpanel" tabIndex={-1} aria-label={`${search.view} view`} {...stylex.props(styles.panel)}>
        {search.view === "board" ? <WorkBoard projectId={search.project} onNewTask={() => setCaptureOpen(true)} /> : null}
        {search.view === "list" ? <WorkList projectId={search.project} query={search.q} onClearFilters={() => onSearchChange({ view: "list" })} /> : null}
        {search.view === "needs-you" ? <WorkNeedsYou projectId={search.project} /> : null}
        {search.view === "activity" ? <WorkActivity projectId={search.project} /> : null}
        {search.view === "archive" ? <WorkArchive projectId={search.project} query={search.q} terminal={search.terminal ?? "all"} onTerminalChange={(terminal) => onSearchChange({ ...search, terminal })} /> : null}
      </main>
      <CaptureTaskDialog key={`${search.project ?? "all"}:${captureOpen ? "open" : "closed"}`} open={captureOpen} projects={projects} initialProjectId={search.project} onOpenChange={setCaptureOpen} onCreated={refresh} />
      <ProjectManagerDialog open={projectsOpen} projects={projects} onOpenChange={setProjectsOpen} onUpdated={refresh} />
    </section>
  );
}

const styles = stylex.create({
  surface: { display: "grid", gridTemplateRows: "auto minmax(0, 1fr)", height: "100%", minHeight: 0, overflow: "hidden", backgroundColor: "var(--noema-surface-card)" },
  surfaceWithNotice: { gridTemplateRows: "auto auto minmax(0, 1fr)" },
  panel: { minHeight: 0, outline: "none", overflowY: "auto", overflowX: "hidden", scrollbarWidth: "thin" },
  notices: { display: "grid", gap: 4, paddingInline: 12, ":empty": { display: "none" } },
  live: { justifySelf: "end", paddingBlock: 3, color: "var(--noema-text-muted)", fontSize: 10, ":empty": { display: "none" } },
  refresh: { justifySelf: "start", borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-clay-700)", font: "inherit", fontSize: 11, textDecoration: "underline", cursor: "pointer" }
});
