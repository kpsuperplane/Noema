import * as React from "react";
import { useApolloClient, useSubscription } from "@apollo/client/react";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { MasterDetailLayout } from "@/components/shell/MasterDetailLayout";
import { ShellPageLayout, ShellPageTrack } from "@/components/shell/ShellPageLayout";
import { TasksEventsDocument } from "@/generated/graphql";
import { ChatDetailRail } from "../chatDetail/ChatDetailRail";
import type { TasksSearch } from "./tasksTypes";
import { CaptureTaskDialog } from "./CaptureTaskDialog";
import { TasksToolbar } from "./TasksToolbar";
import { TasksList } from "./TasksViews";
import { useTasksEventCursor } from "./tasksEventCursor";
import { PERSONAL_WORKSPACE_ID } from "./tasksTypes";
import { useTaskProjects } from "./useTaskProjects";
import { useTasksEventInvalidation } from "./useTasksEventInvalidation";

export function TasksSurface({ search, selectedTaskId, onCloseTask }: {
  search: TasksSearch;
  selectedTaskId?: string;
  onCloseTask?: () => void;
}) {
  const client = useApolloClient();
  const projectsResult = useTaskProjects();
  const projects = projectsResult.projects;
  const [captureOpen, setCaptureOpen] = React.useState(false);
  const [eventCursor, recordEventCursor] = useTasksEventCursor(PERSONAL_WORKSPACE_ID);
  const seenEventIdsRef = React.useRef(new Set<string>());
  const scheduleInvalidation = useTasksEventInvalidation({
    client,
    refetchProjects: projectsResult.refetch
  });

  const subscription = useSubscription(TasksEventsDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, after: eventCursor },
    onData: ({ data }) => {
      const event = data.data?.tasksEvents;
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
      <section aria-labelledby="tasks-page-title" {...stylex.props(styles.surface)}>
        <MasterDetailLayout
          detailOpen={Boolean(selectedTaskId)}
          detailLabel="Task and artifact details"
          onDetailOpenChange={(open) => {
            if (!open) onCloseTask?.();
          }}
          list={
            <VStack {...stylex.props(styles.listScroller)}>
              <TasksToolbar onNewTask={() => setCaptureOpen(true)} />
              {hasNotice ? (
                <ShellPageTrack>
                  <div {...stylex.props(styles.notices)}>
                    <span aria-live="polite" {...stylex.props(styles.live)}>{subscription.error ? "Updating tasks. Reconnecting." : ""}</span>
                    {projectsResult.error ? <button type="button" {...stylex.props(styles.refresh)} onClick={() => void projectsResult.retry()}>Project information could not refresh. Retry</button> : null}
                  </div>
                </ShellPageTrack>
              ) : null}
              <section aria-label="Tasks" {...stylex.props(styles.panel)}>
                <ShellPageTrack>
                  <TasksList
                    projectId={search.project}
                    query={search.q}
                    selectedTaskId={selectedTaskId}
                    terminal={search.terminal ?? "all"}
                  />
                </ShellPageTrack>
              </section>
            </VStack>
          }
          detail={selectedTaskId ? (
            <ChatDetailRail
              animateEntrance={false}
              target={{ type: "task", taskId: selectedTaskId }}
              onClose={() => onCloseTask?.()}
              showTasksLink={false}
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
    boxSizing: "border-box",
    height: "100%",
    minHeight: 0,
    position: "relative",
    "--chat-detail-rail-width": "100%",
    "@media (max-width: 760px)": {
      paddingTop: "var(--shell-deck-header-height)"
    }
  },
  listScroller: { width: "100%", height: "100%", minHeight: 0, overflowY: "auto", overflowX: "hidden", overscrollBehavior: "contain", scrollbarWidth: "thin" },
  panel: { minHeight: 0, outline: "none" },
  notices: { display: "grid", gap: "var(--spacing-1)", ":empty": { display: "none" } },
  live: { justifySelf: "end", paddingBlock: "calc(var(--spacing-1) - 1px)", color: "var(--noema-text-muted)", fontSize: 12, ":empty": { display: "none" } },
  refresh: { justifySelf: "start", borderWidth: 0, backgroundColor: "transparent", padding: "var(--spacing-0)", color: "var(--noema-clay-700)", font: "inherit", fontSize: 12, textDecoration: "underline", cursor: "pointer" }
});
