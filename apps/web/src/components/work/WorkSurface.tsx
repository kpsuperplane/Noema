import * as React from "react";
import { useApolloClient, useSubscription } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { MasterDetailLayout } from "@/components/shell/MasterDetailLayout";
import { ShellPageLayout, ShellPageTrack } from "@/components/shell/ShellPageLayout";
import { WorkEventsDocument } from "@/generated/graphql";
import { ChatDetailRail } from "../chatDetail/ChatDetailRail";
import type { WorkSearch } from "./workTypes";
import { CaptureTaskDialog } from "./CaptureTaskDialog";
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
  const [captureOpen, setCaptureOpen] = React.useState(false);
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

  return (
    <ShellPageLayout width="fluid">
      <section aria-labelledby="work-page-title" {...stylex.props(styles.surface)}>
        <MasterDetailLayout
          detailOpen={Boolean(selectedTaskId)}
          detailLabel="Task details"
          list={
            <>
              <WorkToolbar
                queryDraft={queryDraft}
                terminal={search.terminal ?? "all"}
                onQueryChange={setQueryDraft}
                onTerminalChange={(terminal) => onSearchChange({ ...search, terminal })}
                onNewTask={() => setCaptureOpen(true)}
              />
              {hasNotice ? (
                <ShellPageTrack>
                  <div {...stylex.props(styles.notices)}>
                    <span aria-live="polite" {...stylex.props(styles.live)}>{subscription.error ? "Updating tasks. Reconnecting." : ""}</span>
                    {projectsResult.error ? <button type="button" {...stylex.props(styles.refresh)} onClick={() => void projectsResult.retry()}>Project information could not refresh. Retry</button> : null}
                  </div>
                </ShellPageTrack>
              ) : null}
              <main aria-label="Tasks" {...stylex.props(styles.panel)}>
                <ShellPageTrack>
                  <WorkTasks
                    projectId={search.project}
                    query={search.q}
                    terminal={search.terminal ?? "all"}
                  />
                </ShellPageTrack>
              </main>
            </>
          }
          detail={selectedTaskId ? (
            <ChatDetailRail
              target={{ type: "task", taskId: selectedTaskId }}
              onClose={() => onCloseTask?.()}
              showWorkLink={false}
            />
          ) : null}
        />
        <CaptureTaskDialog key={`${search.project ?? "all"}:${captureOpen ? "open" : "closed"}`} open={captureOpen} projects={projects} initialProjectId={search.project} onOpenChange={setCaptureOpen} onCreated={refresh} />
      </section>
    </ShellPageLayout>
  );
}

const styles = stylex.create({
  surface: {
    height: "100%",
    minHeight: 0,
    position: "relative",
    "--chat-detail-rail-width": "100%"
  },
  panel: { minHeight: 0, outline: "none", overflowY: "auto", overflowX: "hidden", scrollbarWidth: "thin" },
  notices: { display: "grid", gap: 4, ":empty": { display: "none" } },
  live: { justifySelf: "end", paddingBlock: 3, color: "var(--noema-text-muted)", fontSize: 10, ":empty": { display: "none" } },
  refresh: { justifySelf: "start", borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-clay-700)", font: "inherit", fontSize: 11, textDecoration: "underline", cursor: "pointer" }
});
