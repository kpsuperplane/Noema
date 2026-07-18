import * as React from "react";
import { useQuery, useSubscription } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { BriefcaseBusiness, ChevronRight, X } from "lucide-react";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { taskDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { WorkEventsDocument, WorkPanelDocument } from "@/generated/graphql";
import { AttentionBadge } from "./AttentionBadge";
import { StageBadge } from "./StageBadge";
import { TaskActions } from "./TaskActions";
import { PERSONAL_WORKSPACE_ID } from "./workTypes";
import { useWorkEventCursor } from "./workEventCursor";
import { useAllWorkProjects } from "./useAllWorkProjects";
import { WorkRefreshError } from "./WorkRefreshError";

export function ChatWorkPanel({ open, onToggle, onOpenDetail }: { open: boolean; onToggle: () => void; onOpenDetail: (target: ChatDetailTarget) => void }) {
  const { data, loading, error, refetch } = useQuery(WorkPanelDocument, { variables: { workspaceId: PERSONAL_WORKSPACE_ID }, fetchPolicy: "cache-and-network", skip: !open });
  const projectsResult = useAllWorkProjects({ skip: !open });
  const [eventCursor, recordEventCursor] = useWorkEventCursor(PERSONAL_WORKSPACE_ID);
  const hadSubscriptionError = React.useRef(false);
  const refreshTimerRef = React.useRef<number | null>(null);
  const refreshProjectsRef = React.useRef(false);
  const scheduleRefresh = React.useCallback((includeProjects: boolean) => {
    refreshProjectsRef.current ||= includeProjects;
    if (refreshTimerRef.current !== null) return;
    refreshTimerRef.current = window.setTimeout(() => {
      refreshTimerRef.current = null;
      const refreshProjects = refreshProjectsRef.current;
      refreshProjectsRef.current = false;
      void Promise.allSettled([
        refetch(),
        ...(refreshProjects ? [projectsResult.refetch()] : [])
      ]);
    }, 75);
  }, [projectsResult, refetch]);
  React.useEffect(() => () => {
    if (refreshTimerRef.current !== null) window.clearTimeout(refreshTimerRef.current);
  }, []);
  const subscription = useSubscription(WorkEventsDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, after: eventCursor },
    skip: !open,
    onData: ({ data: result }) => {
      const event = result.data?.workEvents;
      if (!event || event.cursor === eventCursor) return;
      recordEventCursor(event.cursor);
      scheduleRefresh(!event.taskId);
    }
  });
  React.useEffect(() => {
    if (!open) return;
    if (subscription.error) {
      hadSubscriptionError.current = true;
    } else if (hadSubscriptionError.current) {
      hadSubscriptionError.current = false;
      scheduleRefresh(false);
    }
  }, [open, scheduleRefresh, subscription.error]);
  const attention = data?.needsYou.edges ?? [];
  const active = data?.workTasks.edges ?? [];
  const projects = projectsResult.projects.filter((project) => !project.archivedAt);
  const openTask = (taskId: string) => {
    const target = taskDetailTarget(taskId);
    if (target) onOpenDetail(target);
  };

  return (
    <div data-open={open} {...stylex.props(styles.root)}>
      <button type="button" aria-expanded={open} aria-controls="chat-work-panel" {...stylex.props(styles.toggle)} onClick={onToggle}>
        {open ? <X aria-hidden="true" size={16} /> : <BriefcaseBusiness aria-hidden="true" size={16} />}
        <span>{open ? "Close Work" : "Work"}</span>
      </button>
      {open ? (
        <aside id="chat-work-panel" aria-label="Work overview" {...stylex.props(styles.panel)}>
          <div {...stylex.props(styles.heading)}><div {...stylex.props(styles.headingCopy)}><strong {...stylex.props(styles.headingTitle)}>Work</strong><span {...stylex.props(styles.headingSubtitle)}>Personal workspace</span></div><Link to="/work" search={{ view: "board", terminal: "all" }} {...stylex.props(styles.workLink)}>Open Work <ChevronRight aria-hidden="true" size={13} /></Link></div>
          {loading && !data ? <p role="status" {...stylex.props(styles.state)}>Loading work…</p> : null}
          {error && !data ? <div role="alert" {...stylex.props(styles.state)}><span>Work could not load.</span><button type="button" {...stylex.props(styles.retry)} onClick={() => void refetch()}>Retry</button></div> : null}
          {error && data ? <WorkRefreshError message="Work could not refresh. Showing cached items." onRetry={() => void refetch()} /> : null}
          {projectsResult.error ? <WorkRefreshError message="Projects could not refresh. Showing cached projects." onRetry={() => void projectsResult.retry()} /> : null}
          {data ? (
            <div {...stylex.props(styles.sections)}>
              <section aria-labelledby="chat-work-needs" {...stylex.props(styles.section)}><h3 id="chat-work-needs" {...stylex.props(styles.sectionTitle)}>Needs you <span {...stylex.props(styles.count)}>{attention.length}</span></h3>{attention.length ? <ul {...stylex.props(styles.list)}>{attention.map(({ node }) => <li key={`${node.kind}:${node.task.taskId}`} {...stylex.props(styles.item)}><button type="button" {...stylex.props(styles.summaryButton)} onClick={() => openTask(node.task.taskId)}><AttentionBadge kind={node.kind} label={node.title} /><strong {...stylex.props(styles.itemTitle)}>{node.task.title}</strong><small {...stylex.props(styles.itemSummary)}>{node.summary}</small></button><TaskActions compact task={node.task} validActions={node.validActions} projects={projects} onUpdated={async () => { await refetch(); }} /></li>)}</ul> : <p {...stylex.props(styles.empty)}>Nothing needs your attention.</p>}</section>
              <section aria-labelledby="chat-work-active" {...stylex.props(styles.section)}><h3 id="chat-work-active" {...stylex.props(styles.sectionTitle)}>Active <span {...stylex.props(styles.count)}>{active.length}</span></h3>{active.length ? <ul {...stylex.props(styles.list)}>{active.map(({ node }) => <li key={node.taskId} {...stylex.props(styles.item)}><button type="button" {...stylex.props(styles.summaryButton)} onClick={() => openTask(node.taskId)}><StageBadge name={node.stage.name} behavior={node.stage.behavior} /><strong {...stylex.props(styles.itemTitle)}>{node.title}</strong><small {...stylex.props(styles.itemSummary)}>{node.currentRun?.activityLabel ?? "Ready"}</small></button><TaskActions compact task={node} validActions={node.validActions} projects={projects} onUpdated={async () => { await refetch(); }} /></li>)}</ul> : <p {...stylex.props(styles.empty)}>No active tasks.</p>}</section>
            </div>
          ) : null}
        </aside>
      ) : null}
    </div>
  );
}

const styles = stylex.create({
  root: { position: "absolute", top: 14, right: 16, zIndex: 3, display: "grid", justifyItems: "end", gap: 8, maxWidth: "calc(100% - 32px)", pointerEvents: "none", "@media (max-width: 560px)": { top: 8, right: 8, maxWidth: "calc(100% - 16px)" } },
  toggle: { display: "inline-flex", alignItems: "center", gap: 6, minHeight: 32, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 999, backgroundColor: "var(--noema-surface-card)", paddingInline: 11, color: "var(--noema-text-primary)", boxShadow: "0 3px 12px color-mix(in srgb, black 8%, transparent)", font: "inherit", fontSize: 12, fontWeight: 650, pointerEvents: "auto", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  panel: { width: "min(340px, calc(100vw - 32px))", maxHeight: "min(620px, calc(100vh - 90px))", overflowY: "auto", overscrollBehavior: "contain", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 14, backgroundColor: "var(--noema-surface-card)", boxShadow: "0 18px 44px color-mix(in srgb, black 16%, transparent)", padding: 13, pointerEvents: "auto", "@media (max-width: 560px)": { width: "calc(100vw - 16px)", maxHeight: "calc(100vh - 64px)", borderRadius: 12 } },
  heading: { display: "flex", alignItems: "start", justifyContent: "space-between", gap: 10, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingBottom: 10 },
  headingCopy: { display: "grid", gap: 2 },
  headingTitle: { color: "var(--noema-text-primary)", fontFamily: "var(--font-heading)", fontSize: 16 },
  headingSubtitle: { color: "var(--noema-text-muted)", fontSize: 10 },
  workLink: { display: "inline-flex", alignItems: "center", gap: 2, minHeight: 28, color: "var(--noema-pine-700)", fontSize: 11, fontWeight: 700, textDecoration: "none", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  sections: { display: "grid", gap: 16, paddingTop: 12 },
  section: { display: "grid", gap: 7 },
  sectionTitle: { display: "flex", alignItems: "center", gap: 6, margin: 0, color: "var(--noema-text-primary)", fontFamily: "var(--font-heading)", fontSize: 12 },
  count: { minWidth: 19, borderRadius: 999, backgroundColor: "var(--noema-surface-sunken)", paddingBlock: 2, paddingInline: 5, color: "var(--noema-text-muted)", fontFamily: "var(--font-body)", fontSize: 9, textAlign: "center" },
  list: { display: "grid", gap: 7, margin: 0, padding: 0, listStyle: "none" },
  item: { display: "grid", gap: 7, minWidth: 0, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, backgroundColor: "var(--noema-surface-card)", padding: 9 },
  summaryButton: { display: "grid", justifyItems: "start", gap: 4, minWidth: 0, width: "100%", borderWidth: 0, borderRadius: 7, backgroundColor: "transparent", padding: 0, color: "var(--noema-text-primary)", font: "inherit", textAlign: "left", ":hover": { color: "var(--noema-pine-700)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 3 } },
  itemTitle: { maxWidth: "100%", overflow: "hidden", fontSize: 12, lineHeight: 1.3, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  itemSummary: { maxWidth: "100%", overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  empty: { margin: 0, borderRadius: 9, backgroundColor: "var(--noema-surface-sunken)", padding: 10, color: "var(--noema-text-muted)", fontSize: 11 },
  state: { display: "flex", alignItems: "center", gap: 8, margin: 0, paddingBlock: 18, color: "var(--noema-text-muted)", fontSize: 12 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-pine-700)", font: "inherit", fontWeight: 700, textDecoration: "underline", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } }
});
