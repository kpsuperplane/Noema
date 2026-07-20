import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import {
  GovernedActionList,
  usePendingGovernedActions
} from "@/components/actions/PendingGovernedActions";
import {
  WorkActivityDocument,
  WorkArchiveTasksDocument,
  WorkNeedsYouDocument,
  WorkOverviewDocument,
  WorkTasksDocument,
  type WorkflowStageBehavior
} from "@/generated/graphql";
import { compactTaskIdentity, eventLabel, isDisplayedActivityEvent, relativeTime, taskRunLabel, terminalRunIdentity, timestampLabel } from "./workModel";
import { normalizeWorkSearch, PERSONAL_WORKSPACE_ID, type WorkEvent, type WorkOverview, type WorkTask } from "./workTypes";

export function WorkBoard({ projectId, onNewTask }: { projectId?: string; onNewTask: () => void }) {
  const result = useQuery(WorkOverviewDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, projectId },
    fetchPolicy: "cache-and-network"
  });
  const overview = result.data?.workOverview;
  if (!overview) return <QueryState loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="board" />;
  return (
    <div role="region" aria-label="Work board" {...stylex.props(styles.board)}>
      {overview.activeColumns.map((column) => (
        <BoardLane key={column.stage.stageId} column={column} projectId={projectId} />
      ))}
      {overview.activeColumns.length === 0 ? (
        <EmptyState title="Your board is clear" detail="Captured and delegated tasks appear here." action="New task" onAction={onNewTask} />
      ) : null}
    </div>
  );
}

function BoardLane({ column, projectId }: { column: WorkOverview["activeColumns"][number]; projectId?: string }) {
  const result = useQuery(WorkTasksDocument, {
    variables: {
      input: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, stageIds: [column.stage.stageId], scope: "ACTIVE" },
      first: 50
    },
    fetchPolicy: "cache-and-network"
  });
  const tasks = result.data?.workTasks.edges.map((edge) => edge.node) ?? [];
  const titleId = `work-lane-${column.stage.stageId.replaceAll(":", "-")}`;
  return (
    <section aria-labelledby={titleId} {...stylex.props(styles.lane)}>
      <header {...stylex.props(styles.laneHeader)}>
        <h2 id={titleId} {...stylex.props(styles.laneTitle)}>{column.stage.name}</h2>
        <span aria-label={`${column.taskCount} tasks`} {...stylex.props(styles.count)}>{column.taskCount}</span>
      </header>
      <div {...stylex.props(styles.laneBody)}>
        {!result.data ? <QueryState compact loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label={column.stage.name} /> : null}
        {tasks.map((task) => <BoardCard key={task.taskId} task={task} />)}
        {result.data && tasks.length === 0 ? <p {...stylex.props(styles.laneEmpty)}>No tasks</p> : null}
        {result.data?.workTasks.pageInfo.hasNextPage ? <Button size="sm" variant="ghost" label="Load more" isLoading={result.loading} onClick={() => void result.fetchMore({ variables: { after: result.data?.workTasks.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, workTasks: { ...fetchMoreResult.workTasks, edges: [...previous.workTasks.edges, ...fetchMoreResult.workTasks.edges] } }) })} /> : null}
      </div>
    </section>
  );
}

function BoardCard({ task }: { task: WorkTask }) {
  return (
    <Link
      to="/work/tasks/$taskId"
      params={{ taskId: task.taskId }}
      search={(current) => normalizeWorkSearch(current)}
      {...stylex.props(styles.card)}
    >
      <span {...stylex.props(styles.cardTitle)}>{task.title}</span>
      <span {...stylex.props(styles.cardMeta)}>
        <span {...stylex.props(task.attention ? styles.attention : styles.muted)}>
          {task.attention?.title ?? taskRunLabel(task) ?? task.project?.name ?? "No project"}
        </span>
        <time dateTime={task.updatedAt} title={timestampLabel(task.updatedAt)}>{relativeTime(task.updatedAt)}</time>
      </span>
    </Link>
  );
}

export function WorkList({ projectId, query, onClearFilters }: { projectId?: string; query?: string; onClearFilters: () => void }) {
  const [behavior, setBehavior] = React.useState<WorkflowStageBehavior | "">("");
  const [attentionOnly, setAttentionOnly] = React.useState(false);
  const result = useQuery(WorkTasksDocument, {
    variables: {
      input: {
        workspaceId: PERSONAL_WORKSPACE_ID,
        projectId,
        text: query,
        stageBehaviors: behavior ? [behavior] : undefined,
        attentionOnly,
        scope: "ACTIVE"
      },
      first: 50
    },
    fetchPolicy: "cache-and-network"
  });
  const connection = result.data?.workTasks;
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  return (
    <section aria-label="Active tasks" {...stylex.props(styles.stack)}>
      <div aria-label="List filters" {...stylex.props(styles.inlineFilters)}>
        <label {...stylex.props(styles.filterLabel)}>Stage
          <select value={behavior} {...stylex.props(styles.select)} onChange={(event) => setBehavior(event.currentTarget.value as typeof behavior)}>
            <option value="">All active stages</option>
            <option value="INTAKE">Inbox</option>
            <option value="DISPATCH">Queue</option>
            <option value="ACTIVE">Doing</option>
            <option value="HUMAN_GATE">Waiting</option>
            <option value="ACCEPTANCE">Done</option>
          </select>
        </label>
        <label {...stylex.props(styles.checkbox)}><input type="checkbox" checked={attentionOnly} onChange={(event) => setAttentionOnly(event.currentTarget.checked)} /> Needs attention</label>
      </div>
      {!connection ? <QueryState loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="tasks" /> : null}
      {connection && tasks.length === 0 ? (
        <EmptyState
          title="No matching tasks"
          detail="Adjust the active filters to see more work."
          action="Clear filters"
          onAction={() => { setBehavior(""); setAttentionOnly(false); onClearFilters(); }}
        />
      ) : null}
      {tasks.length ? <TaskRows tasks={tasks} /> : null}
      <LoadMore connection={connection} loading={result.loading} onLoad={() => result.fetchMore({
        variables: { after: connection?.pageInfo.endCursor },
        updateQuery: (previous, { fetchMoreResult }) => ({
          ...fetchMoreResult,
          workTasks: { ...fetchMoreResult.workTasks, edges: [...previous.workTasks.edges, ...fetchMoreResult.workTasks.edges] }
        })
      })} />
    </section>
  );
}

function TaskRows({ tasks, terminal = false }: { tasks: readonly WorkTask[]; terminal?: boolean }) {
  return (
    <div role="table" aria-label={terminal ? "Archive tasks" : "Active tasks"} {...stylex.props(styles.table)}>
      <div role="row" {...stylex.props(styles.tableHeader)}>
        <span role="columnheader">Task</span><span role="columnheader" {...stylex.props(styles.hiddenMobile)}>Project</span><span role="columnheader" {...stylex.props(styles.hiddenMobile)}>Stage</span><span role="columnheader">Updated</span>
      </div>
      {tasks.map((task) => (
        <Link
          key={task.taskId}
          role="row"
          to="/work/tasks/$taskId"
          params={{ taskId: task.taskId }}
          search={(current) => normalizeWorkSearch(current)}
          {...stylex.props(styles.tableRow)}
        >
          <span role="cell" {...stylex.props(styles.rowTask)}>
            <strong {...stylex.props(styles.rowTitle)}>{task.title}</strong>
            {task.attention ? <span {...stylex.props(styles.rowNote)}>{task.attention.title}</span> : null}
            <span {...stylex.props(styles.mobileContext)}>{task.project?.name ?? "No project"} · {task.stage.name}</span>
          </span>
          <span role="cell" {...stylex.props(styles.cell, styles.projectCell)}>{task.project?.name ?? "—"}</span>
          <span role="cell" {...stylex.props(styles.cell, styles.stageCell)}>{task.stage.name}</span>
          <time role="cell" dateTime={task.completedAt ?? task.updatedAt} title={timestampLabel(task.completedAt ?? task.updatedAt)} {...stylex.props(styles.cell)}>
            {relativeTime(task.completedAt ?? task.updatedAt)}
          </time>
        </Link>
      ))}
    </div>
  );
}

export function WorkNeedsYou({ projectId }: { projectId?: string }) {
  const result = useQuery(WorkNeedsYouDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, first: 50 },
    fetchPolicy: "cache-and-network"
  });
  const actionResult = usePendingGovernedActions();
  const connection = result.data?.needsYou;
  const items = connection?.edges.map((edge) => edge.node) ?? [];
  const actions = actionResult.data?.pendingGovernedActions ?? [];
  if (!connection) return <QueryState loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="attention queue" />;
  if (!items.length && !actions.length) return <EmptyState title="Nothing needs you" detail="Questions, approvals, recovery choices, and reviewed results will appear here." />;
  return (
    <div {...stylex.props(styles.attentionStack)}>
      {actions.length ? (
        <GovernedActionList actions={actions} onResolved={() => void actionResult.refetch()} />
      ) : null}
      {items.length ? (
        <section aria-label="Task decisions" {...stylex.props(styles.queue)}>
          {items.map((item) => (
            <Link
              key={`${item.task.taskId}:${item.kind}`}
              to="/work/tasks/$taskId"
              params={{ taskId: item.task.taskId }}
              search={(current) => normalizeWorkSearch(current)}
              {...stylex.props(styles.queueRow)}
            >
              <span {...stylex.props(styles.queueKind)}>{attentionLabel(item.kind)}</span>
              <span {...stylex.props(styles.queueCopy)}>
                <strong {...stylex.props(styles.rowTitle)}>{item.task.title}</strong>
                <span {...stylex.props(styles.queueSummary)}>{item.summary}</span>
                <span {...stylex.props(styles.queueMeta)}>{item.task.project?.name ?? "No project"} · {relativeTime(item.task.updatedAt)}</span>
              </span>
              <span {...stylex.props(styles.queueAction)}>{item.title}</span>
            </Link>
          ))}
          <LoadMore connection={connection} loading={result.loading} onLoad={() => result.fetchMore({
            variables: { after: connection.pageInfo.endCursor },
            updateQuery: (previous, { fetchMoreResult }) => ({
              ...fetchMoreResult,
              needsYou: { ...fetchMoreResult.needsYou, edges: [...previous.needsYou.edges, ...fetchMoreResult.needsYou.edges] }
            })
          })} />
        </section>
      ) : null}
    </div>
  );
}

export function WorkActivity({ projectId }: { projectId?: string }) {
  const result = useQuery(WorkActivityDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, first: 50 },
    fetchPolicy: "cache-and-network"
  });
  const taskIndexResult = useQuery(WorkTasksDocument, {
    variables: {
      input: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, scope: "ALL" },
      first: 100
    },
    fetchPolicy: "cache-and-network"
  });
  const connection = result.data?.workActivity;
  const events = connection?.edges.map((edge) => edge.node).filter(isDisplayedActivityEvent) ?? [];
  const taskTitles = new Map(
    taskIndexResult.data?.workTasks.edges.map(({ node }) => [node.taskId, node.title]) ?? []
  );
  if (!connection) return <QueryState loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="activity" />;
  return (
    <section aria-label="Work activity" {...stylex.props(styles.activityFrame)}>
      {events.length ? (
        <ol {...stylex.props(styles.activity)}>
          {groupEvents(events).map((group) => (
            <React.Fragment key={group.key}>
              <li {...stylex.props(styles.day)}>{group.label}</li>
              {group.events.map((event) => (
                <li key={event.eventId} {...stylex.props(styles.event)}>
                  <span aria-hidden="true" {...stylex.props(styles.eventDot)} />
                  <span {...stylex.props(styles.eventCopy)}>
                    {event.taskId ? (
                      <Link to="/work/tasks/$taskId" params={{ taskId: event.taskId }} search={(current) => normalizeWorkSearch(current)} {...stylex.props(styles.eventLink)}>
                        <span {...stylex.props(styles.eventLabel)}>{eventLabel(event)}</span>
                        <span {...stylex.props(styles.eventContext)}>
                          {taskTitles.get(event.taskId) ?? compactTaskIdentity(event.taskId)}
                          {terminalRunIdentity(event) ? ` · ${terminalRunIdentity(event)}` : null}
                        </span>
                      </Link>
                    ) : <strong {...stylex.props(styles.eventLabel)}>{eventLabel(event)}</strong>}
                    <time dateTime={event.occurredAt} title={timestampLabel(event.occurredAt)} {...stylex.props(styles.eventTime)}>{relativeTime(event.occurredAt)}</time>
                  </span>
                </li>
              ))}
            </React.Fragment>
          ))}
        </ol>
      ) : (
        <EmptyState
          title={connection.pageInfo.hasNextPage ? "No milestones on this page" : "No activity yet"}
          detail={connection.pageInfo.hasNextPage ? "Load more to find earlier task and project milestones." : "Task and project milestones will appear here."}
        />
      )}
      <LoadMore connection={connection} loading={result.loading} onLoad={() => result.fetchMore({ variables: { after: connection.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, workActivity: { ...fetchMoreResult.workActivity, edges: [...previous.workActivity.edges, ...fetchMoreResult.workActivity.edges] } }) })} />
    </section>
  );
}

export function WorkArchive({ projectId, query, terminal, onTerminalChange }: { projectId?: string; query?: string; terminal: "all" | "accepted" | "cancelled"; onTerminalChange: (value: "all" | "accepted" | "cancelled") => void }) {
  const kind = terminal === "accepted" ? "ACCEPTED" : terminal === "cancelled" ? "CANCELLED" : "ALL";
  const result = useQuery(WorkArchiveTasksDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, text: query, kind, first: 50 },
    fetchPolicy: "cache-and-network"
  });
  const connection = result.data?.archiveTasks;
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  return (
    <section aria-label="Archive work" {...stylex.props(styles.stack)}>
      <div {...stylex.props(styles.inlineFilters)}>
        <label {...stylex.props(styles.filterLabel)}>Show
          <select value={terminal} {...stylex.props(styles.select)} onChange={(event) => onTerminalChange(event.currentTarget.value as typeof terminal)}>
            <option value="all">Archived and cancelled</option><option value="accepted">Archived</option><option value="cancelled">Cancelled</option>
          </select>
        </label>
      </div>
      {!connection ? <QueryState loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="history" /> : null}
      {connection && !tasks.length ? <EmptyState title="No matching archive items" detail="Archived and cancelled tasks remain available here." /> : null}
      {tasks.length ? <TaskRows tasks={tasks} terminal /> : null}
      <LoadMore connection={connection} loading={result.loading} onLoad={() => result.fetchMore({ variables: { after: connection?.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, archiveTasks: { ...fetchMoreResult.archiveTasks, edges: [...previous.archiveTasks.edges, ...fetchMoreResult.archiveTasks.edges] } }) })} />
    </section>
  );
}

function QueryState({ loading, error, retry, label, compact = false }: { loading: boolean; error: boolean; retry: () => unknown; label: string; compact?: boolean }) {
  if (loading) return <div role="status" {...stylex.props(styles.state, compact && styles.stateCompact)}>Loading {label}…</div>;
  if (error) return <div role="alert" {...stylex.props(styles.state, compact && styles.stateCompact)}>Could not load {label}. <button type="button" {...stylex.props(styles.retry)} onClick={() => retry()}>Retry</button></div>;
  return null;
}

function EmptyState({ title, detail, action, onAction }: { title: string; detail: string; action?: string; onAction?: () => void }) {
  return <div {...stylex.props(styles.empty)}><strong>{title}</strong><span>{detail}</span>{action && onAction ? <Button size="sm" variant="secondary" label={action} onClick={onAction} /> : null}</div>;
}

function LoadMore({ connection, loading, onLoad }: { connection?: { pageInfo: { hasNextPage: boolean } }; loading: boolean; onLoad: () => unknown }) {
  return connection?.pageInfo.hasNextPage ? <Button size="sm" variant="ghost" label="Load more" isLoading={loading} onClick={() => onLoad()} /> : null;
}

function attentionLabel(kind: string): string {
  if (kind === "CLARIFICATION_REQUIRED") return "Question";
  if (kind === "APPROVAL_REQUIRED") return "Approval";
  if (kind === "RECOVERY_REQUIRED") return "Recovery";
  return "Done";
}

function groupEvents(events: readonly WorkEvent[]) {
  const groups: Array<{ key: string; label: string; events: WorkEvent[] }> = [];
  const format = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });
  for (const event of events) {
    const date = new Date(event.occurredAt);
    const key = Number.isFinite(date.getTime()) ? date.toISOString().slice(0, 10) : event.occurredAt;
    let group = groups.at(-1);
    if (!group || group.key !== key) {
      group = { key, label: Number.isFinite(date.getTime()) ? format.format(date) : "Recent", events: [] };
      groups.push(group);
    }
    group.events.push(event);
  }
  return groups;
}

const styles = stylex.create({
  board: { display: "grid", gridAutoFlow: "column", gridAutoColumns: "276px", alignItems: "stretch", gap: 8, minHeight: 0, height: "100%", overflowX: "auto", overscrollBehaviorInline: "contain", padding: 12, "@media (max-width: 760px)": { gridAutoColumns: "264px", padding: 8 } },
  lane: { display: "grid", gridTemplateRows: "36px minmax(0, 1fr)", minWidth: 0, minHeight: 0, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 8, backgroundColor: "var(--noema-surface-sunken)", overflow: "hidden" },
  laneHeader: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingInline: 10 },
  laneTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 700 },
  count: { color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 10 },
  laneBody: { display: "flex", minHeight: 0, flexDirection: "column", gap: 6, overflowY: "auto", padding: 6, scrollbarWidth: "thin" },
  laneEmpty: { margin: 4, color: "var(--noema-text-muted)", fontSize: 11 },
  card: { display: "grid", alignContent: "center", gap: 7, minHeight: 72, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 7, backgroundColor: "var(--noema-surface-card)", paddingBlock: 9, paddingInline: 10, color: "inherit", textDecoration: "none", transitionDuration: "120ms", transitionProperty: "background-color, border-color", ":hover": { borderColor: "var(--noema-border-strong)", backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  cardTitle: { display: "-webkit-box", overflow: "hidden", WebkitBoxOrient: "vertical", WebkitLineClamp: 2, color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 650, lineHeight: 1.35 },
  cardMeta: { display: "flex", minWidth: 0, alignItems: "center", justifyContent: "space-between", gap: 8, color: "var(--noema-text-muted)", fontSize: 10 },
  muted: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  attention: { minWidth: 0, overflow: "hidden", color: "var(--noema-clay-700)", fontWeight: 650, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  stack: { display: "grid", alignContent: "start", gap: 8, minHeight: 0, padding: 12, "@media (max-width: 760px)": { padding: 8 } },
  inlineFilters: { display: "flex", minHeight: 32, flexWrap: "wrap", alignItems: "center", gap: 10 },
  filterLabel: { display: "flex", alignItems: "center", gap: 5, color: "var(--noema-text-muted)", fontSize: 11, fontWeight: 650 },
  select: { minHeight: 28, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 6, backgroundColor: "var(--noema-surface-card)", paddingInline: 7, color: "var(--noema-text-primary)", font: "inherit", fontSize: 11 },
  checkbox: { display: "flex", alignItems: "center", gap: 5, color: "var(--noema-text-secondary)", fontSize: 11 },
  table: { display: "grid", minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  tableHeader: { position: "sticky", top: 0, zIndex: 1, display: "grid", gridTemplateColumns: "minmax(240px, 2fr) minmax(100px, .8fr) minmax(100px, .7fr) 76px", minHeight: 30, alignItems: "center", gap: 10, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", backgroundColor: "var(--noema-surface-card)", paddingInline: 10, color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650, "@media (max-width: 700px)": { gridTemplateColumns: "minmax(0, 1fr) 72px" } },
  tableRow: { display: "grid", gridTemplateColumns: "minmax(240px, 2fr) minmax(100px, .8fr) minmax(100px, .7fr) 76px", minHeight: 44, alignItems: "center", gap: 10, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingInline: 10, color: "var(--noema-text-secondary)", fontSize: 11, textDecoration: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: -2 }, "@media (max-width: 700px)": { gridTemplateColumns: "minmax(0, 1fr) 72px" } },
  rowTask: { display: "grid", minWidth: 0, gap: 1 },
  rowTitle: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 650, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  rowNote: { overflow: "hidden", color: "var(--noema-clay-700)", fontSize: 10, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  mobileContext: { display: "none", overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 9, textOverflow: "ellipsis", whiteSpace: "nowrap", "@media (max-width: 700px)": { display: "block" } },
  cell: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  projectCell: { "@media (max-width: 700px)": { display: "none" } },
  stageCell: { "@media (max-width: 700px)": { display: "none" } },
  hiddenMobile: { "@media (max-width: 700px)": { display: "none" } },
  attentionStack: { display: "grid", alignContent: "start", gap: 8, padding: 12, "@media (max-width: 760px)": { padding: 8 } },
  queue: { display: "grid", alignContent: "start", padding: 12, "@media (max-width: 760px)": { padding: 8 } },
  queueRow: { display: "grid", gridTemplateColumns: "88px minmax(0, 1fr) minmax(110px, auto)", minHeight: 48, alignItems: "center", gap: 10, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingInline: 8, color: "inherit", textDecoration: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: -2 }, "@media (max-width: 640px)": { gridTemplateColumns: "72px minmax(0, 1fr)", paddingBlock: 5 } },
  queueKind: { color: "var(--noema-clay-700)", fontSize: 10, fontWeight: 700 },
  queueCopy: { display: "grid", minWidth: 0, gap: 1 },
  queueSummary: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  queueMeta: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 9, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  queueAction: { color: "var(--noema-pine-700)", fontSize: 11, fontWeight: 650, "@media (max-width: 640px)": { display: "none" } },
  activityFrame: { display: "grid", alignContent: "start", gap: 6 },
  activity: { display: "grid", alignContent: "start", margin: 0, paddingBlock: 8, paddingInline: 12, listStyle: "none", "@media (max-width: 760px)": { paddingInline: 8 } },
  day: { position: "sticky", top: 0, zIndex: 1, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", backgroundColor: "var(--noema-surface-card)", paddingBlock: 6, color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 700 },
  event: { display: "grid", gridTemplateColumns: "12px minmax(0, 1fr)", minHeight: 40, alignItems: "center", gap: 7, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)" },
  eventDot: { width: 6, height: 6, borderRadius: 999, backgroundColor: "var(--noema-pine-600)" },
  eventCopy: { display: "flex", minWidth: 0, alignItems: "center", justifyContent: "space-between", gap: 12, fontSize: 11 },
  eventLink: { display: "grid", minWidth: 0, overflow: "hidden", gap: 1, color: "var(--noema-text-primary)", textDecoration: "none", ":hover": { textDecoration: "underline" } },
  eventLabel: { minWidth: 0, overflow: "hidden", fontWeight: 600, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  eventContext: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  eventTime: { flexShrink: 0, color: "var(--noema-text-muted)", fontSize: 10 },
  state: { display: "flex", minHeight: 120, alignItems: "center", justifyContent: "center", gap: 4, color: "var(--noema-text-muted)", fontSize: 12 },
  stateCompact: { minHeight: 64, fontSize: 11 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-pine-700)", font: "inherit", fontWeight: 650, textDecoration: "underline", cursor: "pointer" },
  empty: { display: "grid", maxWidth: 420, minHeight: 160, alignContent: "center", justifyItems: "start", gap: 5, padding: 12, color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.4 }
});
