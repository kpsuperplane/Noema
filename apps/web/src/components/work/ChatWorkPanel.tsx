import * as React from "react";
import { useQuery, useSubscription } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { BriefcaseBusiness, ChevronRight, X } from "lucide-react";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { taskDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { WorkEventsDocument, WorkPanelDocument } from "@/generated/graphql";
import { PERSONAL_WORKSPACE_ID } from "./workTypes";
import { useWorkEventCursor } from "./workEventCursor";

export function ChatWorkPanel({
  open,
  onToggle,
  onOpenDetail
}: {
  open: boolean;
  onToggle: () => void;
  onOpenDetail: (target: ChatDetailTarget) => void;
}) {
  const result = useQuery(WorkPanelDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID },
    fetchPolicy: "cache-and-network",
    skip: !open
  });
  const [eventCursor, recordEventCursor] = useWorkEventCursor(PERSONAL_WORKSPACE_ID);
  const refreshTimer = React.useRef<number | null>(null);

  const scheduleRefresh = React.useCallback(() => {
    if (refreshTimer.current !== null) return;
    refreshTimer.current = window.setTimeout(() => {
      refreshTimer.current = null;
      void result.refetch();
    }, 75);
  }, [result]);

  React.useEffect(() => () => {
    if (refreshTimer.current !== null) window.clearTimeout(refreshTimer.current);
  }, []);

  const subscription = useSubscription(WorkEventsDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, after: eventCursor },
    skip: !open,
    onData: ({ data }) => {
      const event = data.data?.workEvents;
      if (!event || event.cursor === eventCursor) return;
      recordEventCursor(event.cursor);
      scheduleRefresh();
    }
  });

  const openTask = (taskId: string) => {
    const target = taskDetailTarget(taskId);
    if (target) onOpenDetail(target);
  };
  const attention = result.data?.needsYou.edges ?? [];
  const active = (result.data?.workTasks.edges ?? [])
    .filter(({ node }) => !node.attention)
    .slice(0, 8);

  return (
    <div data-open={open} {...stylex.props(styles.root)}>
      <button type="button" aria-expanded={open} aria-controls="chat-work-panel" {...stylex.props(styles.toggle)} onClick={onToggle}>
        {open ? <X aria-hidden="true" size={15} /> : <BriefcaseBusiness aria-hidden="true" size={15} />}
        <span>{open ? "Close Tasks" : "Tasks"}</span>
      </button>
      {open ? (
        <aside id="chat-work-panel" aria-label="Task overview" {...stylex.props(styles.panel)}>
          <header {...stylex.props(styles.header)}>
            <strong {...stylex.props(styles.title)}>Tasks</strong>
            <Link to="/work" search={{ terminal: "all" }} {...stylex.props(styles.openWork)}>
              Open Tasks <ChevronRight aria-hidden="true" size={13} />
            </Link>
          </header>
          {result.loading && !result.data ? <p role="status" {...stylex.props(styles.state)}>Loading tasks…</p> : null}
          {result.error || subscription.error ? (
            <div role={result.data ? "status" : "alert"} {...stylex.props(styles.notice)}>
              <span>{result.data ? "Tasks are reconnecting." : "Tasks could not load."}</span>
              <button type="button" {...stylex.props(styles.retry)} onClick={() => void result.refetch()}>Retry</button>
            </div>
          ) : null}
          {result.data ? (
            <div {...stylex.props(styles.sections)}>
              <WorkSection title="Needs you" count={attention.length} empty="Nothing needs your attention.">
                {attention.map(({ node }) => (
                  <WorkRow
                    key={`${node.kind}:${node.task.taskId}`}
                    label={attentionLabel(node.kind)}
                    title={node.task.title}
                    detail={node.summary}
                    attention
                    onClick={() => openTask(node.task.taskId)}
                  />
                ))}
              </WorkSection>
              <WorkSection title="Active" count={active.length} empty="No active tasks.">
                {active.map(({ node }) => (
                  <WorkRow
                    key={node.taskId}
                    label={node.stage.name}
                    title={node.title}
                    detail={node.currentRun?.activityLabel ?? "Ready"}
                    onClick={() => openTask(node.taskId)}
                  />
                ))}
              </WorkSection>
            </div>
          ) : null}
        </aside>
      ) : null}
    </div>
  );
}

function WorkSection({ title, count, empty, children }: { title: string; count: number; empty: string; children: React.ReactNode }) {
  const id = `chat-work-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <section aria-labelledby={id} {...stylex.props(styles.section)}>
      <h3 id={id} {...stylex.props(styles.sectionTitle)}>{title}<span {...stylex.props(styles.count)}>{count}</span></h3>
      {count ? <div {...stylex.props(styles.rows)}>{children}</div> : <p {...stylex.props(styles.empty)}>{empty}</p>}
    </section>
  );
}

function WorkRow({ label, title, detail, attention = false, onClick }: { label: string; title: string; detail: string; attention?: boolean; onClick: () => void }) {
  return (
    <button type="button" {...stylex.props(styles.row)} onClick={onClick}>
      <span {...stylex.props(styles.rowLabel, attention && styles.attention)}>{label}</span>
      <span {...stylex.props(styles.rowCopy)}>
        <strong {...stylex.props(styles.rowTitle)}>{title}</strong>
        <span {...stylex.props(styles.rowDetail)}>{detail}</span>
      </span>
      <ChevronRight aria-hidden="true" size={13} {...stylex.props(styles.chevron)} />
    </button>
  );
}

function attentionLabel(kind: string): string {
  if (kind === "CLARIFICATION_REQUIRED") return "Question";
  if (kind === "APPROVAL_REQUIRED") return "Approval";
  if (kind === "RECOVERY_REQUIRED") return "Recovery";
  return "Done";
}

const styles = stylex.create({
  root: { position: "absolute", top: 14, right: 16, zIndex: 3, display: "grid", maxWidth: "calc(100% - 32px)", justifyItems: "end", gap: "var(--spacing-1-5)", pointerEvents: "none", "@media (max-width: 560px)": { top: 8, right: 8, maxWidth: "calc(100% - 16px)" } },
  toggle: { display: "inline-flex", minHeight: 30, alignItems: "center", gap: "calc(var(--spacing-1) + 1px)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 999, cornerShape: "var(--corner-shape-full)", backgroundColor: "var(--noema-surface-card)", paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))", color: "var(--noema-text-primary)", boxShadow: "0 2px 8px color-mix(in srgb, black 7%, transparent)", font: "inherit", fontSize: 11, fontWeight: 650, pointerEvents: "auto", cursor: "pointer", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  panel: { width: "min(320px, calc(100vw - 32px))", maxHeight: "min(560px, calc(100vh - 82px))", overflowY: "auto", overscrollBehavior: "contain", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, backgroundColor: "var(--noema-surface-card)", boxShadow: "0 12px 32px color-mix(in srgb, black 14%, transparent)", pointerEvents: "auto", "@media (max-width: 560px)": { width: "calc(100vw - 16px)", maxHeight: "calc(100vh - 58px)", borderRadius: 8 } },
  header: { display: "flex", minHeight: 40, alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-2)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))" },
  title: { color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 700 },
  openWork: { display: "inline-flex", minHeight: 28, alignItems: "center", gap: "var(--spacing-0-5)", color: "var(--noema-pine-700)", fontSize: 10, fontWeight: 700, textDecoration: "none", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  sections: { display: "grid" },
  section: { display: "grid", minWidth: 0, paddingBlock: "calc(var(--spacing-1-5) + 1px)", paddingInline: "var(--spacing-2)", ":not(:last-child)": { borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)" } },
  sectionTitle: { display: "flex", minHeight: 25, alignItems: "center", gap: "calc(var(--spacing-1) + 1px)", margin: "var(--spacing-0)", paddingInline: "calc(var(--spacing-1) - 1px)", color: "var(--noema-text-primary)", fontSize: 10, fontWeight: 700, textTransform: "uppercase", letterSpacing: "0.04em" },
  count: { color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 9 },
  rows: { display: "grid" },
  row: { display: "grid", gridTemplateColumns: "64px minmax(0, 1fr) 14px", minHeight: 44, alignItems: "center", gap: "calc(var(--spacing-1-5) + 1px)", width: "100%", borderWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", backgroundColor: "transparent", paddingInline: "var(--spacing-1)", color: "inherit", font: "inherit", textAlign: "left", cursor: "pointer", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: -2 } },
  rowLabel: { overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 9, fontWeight: 650, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  attention: { color: "var(--noema-clay-700)" },
  rowCopy: { display: "grid", minWidth: 0, gap: "calc(var(--spacing-0-5) - 1px)" },
  rowTitle: { overflow: "hidden", color: "var(--noema-text-primary)", fontSize: 11, fontWeight: 650, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  rowDetail: { overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 9, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  chevron: { color: "var(--noema-text-muted)" },
  state: { margin: "var(--spacing-0)", padding: "calc(var(--spacing-3) + var(--spacing-0-5))", color: "var(--noema-text-muted)", fontSize: 11 },
  notice: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-2)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingBlock: "var(--spacing-1-5)", paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))", color: "var(--noema-clay-700)", fontSize: 10 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: "var(--spacing-0)", color: "inherit", font: "inherit", fontWeight: 700, textDecoration: "underline", cursor: "pointer" },
  empty: { margin: "var(--spacing-0)", paddingBlock: "calc(var(--spacing-2) + 1px)", paddingInline: "calc(var(--spacing-1) - 1px)", color: "var(--noema-text-muted)", fontSize: 10 }
});
