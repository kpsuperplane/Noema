import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import {
  GovernedActionList,
  usePendingGovernedActions
} from "@/components/actions/PendingGovernedActions";
import {
  WorkTaskHistoryDocument,
  WorkNeedsYouDocument,
  WorkTasksDocument,
  type PendingGovernedActionsQuery,
  type WorkNeedsYouQuery,
  type WorkflowStageBehavior
} from "@/generated/graphql";
import { relativeTime, taskRunLabel, timestampLabel } from "./workModel";
import { normalizeWorkSearch, PERSONAL_WORKSPACE_ID, type WorkTask } from "./workTypes";

const taskGroups: ReadonlyArray<{
  behavior: WorkflowStageBehavior;
  title: string;
  detail: string;
}> = [
  { behavior: "ACTIVE", title: "Running", detail: "Planning, executing, or reviewing now." },
  { behavior: "DISPATCH", title: "Up next", detail: "Authorized and waiting to begin." },
  { behavior: "INTAKE", title: "Inbox", detail: "Captured tasks that have not been queued." }
];

export function WorkTasks({ projectId, onNewTask }: { projectId?: string; onNewTask: () => void }) {
  const taskResult = useQuery(WorkTasksDocument, {
    variables: {
      input: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, scope: "ACTIVE" },
      first: 50
    },
    fetchPolicy: "cache-and-network"
  });
  const attentionResult = useQuery(WorkNeedsYouDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, first: 50 },
    fetchPolicy: "cache-and-network"
  });
  const actionResult = usePendingGovernedActions();
  const taskConnection = taskResult.data?.workTasks;
  const attentionConnection = attentionResult.data?.needsYou;
  const tasks = taskConnection?.edges.map((edge) => edge.node) ?? [];
  const attentionItems = attentionConnection?.edges.map((edge) => edge.node) ?? [];
  const actions = actionResult.data?.pendingGovernedActions ?? [];
  const groups = taskGroups
    .map((group) => ({ ...group, tasks: tasks.filter((task) => task.stage.behavior === group.behavior) }))
    .filter((group) => group.tasks.length > 0);
  const empty = Boolean(taskConnection && attentionConnection && !actionResult.loading && !tasks.length && !attentionItems.length && !actions.length);

  if (!taskConnection && !attentionConnection && taskResult.loading && attentionResult.loading) {
    return <QueryState loading error={false} retry={() => Promise.all([taskResult.refetch(), attentionResult.refetch()])} label="work" />;
  }

  return (
    <div role="region" aria-label="Tasks" {...stylex.props(styles.dashboard)}>
      {!attentionConnection ? (
        <QueryState compact loading={attentionResult.loading} error={Boolean(attentionResult.error)} retry={() => attentionResult.refetch()} label="attention queue" />
      ) : null}
      {!actionResult.data && actionResult.error ? (
        <QueryState compact loading={false} error retry={() => actionResult.refetch()} label="approvals" />
      ) : null}
      {attentionConnection && (attentionItems.length > 0 || actions.length > 0) ? (
        <AttentionSection
          items={attentionItems}
          actions={actions}
          loading={attentionResult.loading}
          hasNextPage={attentionConnection.pageInfo.hasNextPage}
          onResolved={() => void actionResult.refetch()}
          onLoadMore={() => attentionResult.fetchMore({
            variables: { after: attentionConnection.pageInfo.endCursor },
            updateQuery: (previous, { fetchMoreResult }) => ({
              ...fetchMoreResult,
              needsYou: {
                ...fetchMoreResult.needsYou,
                edges: [...previous.needsYou.edges, ...fetchMoreResult.needsYou.edges]
              }
            })
          })}
        />
      ) : null}
      {!taskConnection ? (
        <QueryState compact loading={taskResult.loading} error={Boolean(taskResult.error)} retry={() => taskResult.refetch()} label="tasks" />
      ) : null}
      {groups.map((group) => (
        <TaskGroup key={group.behavior} title={group.title} detail={group.detail} tasks={group.tasks} />
      ))}
      {empty ? (
        <EmptyState
          title="No active work"
          detail="Capture an ad hoc task here, or delegate one from Chat."
          action="New task"
          onAction={onNewTask}
        />
      ) : null}
      <LoadMore
        connection={taskConnection}
        loading={taskResult.loading}
        onLoad={() => taskResult.fetchMore({
          variables: { after: taskConnection?.pageInfo.endCursor },
          updateQuery: (previous, { fetchMoreResult }) => ({
            ...fetchMoreResult,
            workTasks: {
              ...fetchMoreResult.workTasks,
              edges: [...previous.workTasks.edges, ...fetchMoreResult.workTasks.edges]
            }
          })
        })}
      />
    </div>
  );
}

function AttentionSection({
  items,
  actions,
  loading,
  hasNextPage,
  onResolved,
  onLoadMore
}: {
  items: WorkNeedsYouItems;
  actions: GovernedActions;
  loading: boolean;
  hasNextPage: boolean;
  onResolved: () => void;
  onLoadMore: () => unknown;
}) {
  const count = items.length + actions.length;
  return (
    <section aria-labelledby="work-needs-you" {...stylex.props(styles.group)}>
      <SectionHeader id="work-needs-you" title="Needs you" detail="Decisions that are holding up work." count={count} attention />
      {actions.length ? <GovernedActionList compact actions={actions} onResolved={onResolved} /> : null}
      {items.length ? (
        <div {...stylex.props(styles.rows)}>
          {items.map((item) => (
            <Link
              key={`${item.task.taskId}:${item.kind}`}
              to="/work/tasks/$taskId"
              params={{ taskId: item.task.taskId }}
              search={(current) => normalizeWorkSearch(current)}
              {...stylex.props(styles.attentionRow)}
            >
              <span {...stylex.props(styles.attentionKind)}>{attentionLabel(item.kind)}</span>
              <span {...stylex.props(styles.rowCopy)}>
                <strong {...stylex.props(styles.rowTitle)}>{item.task.title}</strong>
                <span {...stylex.props(styles.rowNote)}>{item.summary}</span>
                <span {...stylex.props(styles.mobileContext)}>{item.task.project?.name ?? "No project"} · {relativeTime(item.task.updatedAt)}</span>
              </span>
              <span {...stylex.props(styles.attentionAction)}>{item.title}</span>
            </Link>
          ))}
        </div>
      ) : null}
      {hasNextPage ? <Button size="sm" variant="ghost" label="Load more" isLoading={loading} onClick={() => onLoadMore()} /> : null}
    </section>
  );
}

type WorkNeedsYouItems = WorkNeedsYouQuery["needsYou"]["edges"][number]["node"][];
type GovernedActions = PendingGovernedActionsQuery["pendingGovernedActions"];

function TaskGroup({ title, detail, tasks }: { title: string; detail: string; tasks: readonly WorkTask[] }) {
  const id = `work-group-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <section aria-labelledby={id} {...stylex.props(styles.group)}>
      <SectionHeader id={id} title={title} detail={detail} count={tasks.length} />
      <TaskRows tasks={tasks} label={`${title} tasks`} />
    </section>
  );
}

function SectionHeader({ id, title, detail, count, attention = false }: { id: string; title: string; detail: string; count: number; attention?: boolean }) {
  return (
    <header {...stylex.props(styles.sectionHeader)}>
      <span {...stylex.props(styles.sectionCopy)}>
        <h2 id={id} {...stylex.props(styles.sectionTitle, attention && styles.attentionTitle)}>{title}</h2>
        <span {...stylex.props(styles.sectionDetail)}>{detail}</span>
      </span>
      <span aria-label={`${count} items`} {...stylex.props(styles.count)}>{count}</span>
    </header>
  );
}

function TaskRows({ tasks, label, terminal = false }: { tasks: readonly WorkTask[]; label: string; terminal?: boolean }) {
  return (
    <div role={terminal ? "table" : "list"} aria-label={label} {...stylex.props(styles.table)}>
      {terminal ? (
        <div role="row" {...stylex.props(styles.tableHeader)}>
          <span role="columnheader">Task</span><span role="columnheader" {...stylex.props(styles.hiddenMobile)}>Project</span><span role="columnheader" {...stylex.props(styles.hiddenMobile)}>Stage</span><span role="columnheader">Updated</span>
        </div>
      ) : null}
      {tasks.map((task) => (
        <Link
          key={task.taskId}
          role={terminal ? "row" : "listitem"}
          to="/work/tasks/$taskId"
          params={{ taskId: task.taskId }}
          search={(current) => normalizeWorkSearch(current)}
          {...stylex.props(styles.tableRow, terminal && styles.historyRow)}
        >
          <span role={terminal ? "cell" : undefined} {...stylex.props(styles.rowCopy)}>
            <strong {...stylex.props(styles.rowTitle)}>{task.title}</strong>
            {!terminal ? <span {...stylex.props(styles.rowNote)}>{taskRunLabel(task) ?? task.descriptionPreview}</span> : null}
            <span {...stylex.props(styles.mobileContext)}>{task.project?.name ?? "No project"}{terminal ? ` · ${task.stage.name}` : ""}</span>
          </span>
          <span role={terminal ? "cell" : undefined} {...stylex.props(styles.cell, styles.projectCell)}>{task.project?.name ?? "—"}</span>
          {terminal ? <span role="cell" {...stylex.props(styles.cell, styles.stageCell)}>{task.stage.name}</span> : null}
          <time role={terminal ? "cell" : undefined} dateTime={task.completedAt ?? task.updatedAt} title={timestampLabel(task.completedAt ?? task.updatedAt)} {...stylex.props(styles.cell)}>
            {relativeTime(task.completedAt ?? task.updatedAt)}
          </time>
        </Link>
      ))}
    </div>
  );
}

export function WorkHistory({ projectId, query, terminal, onTerminalChange }: { projectId?: string; query?: string; terminal: "all" | "completed" | "cancelled"; onTerminalChange: (value: "all" | "completed" | "cancelled") => void }) {
  const kind = terminal === "completed" ? "COMPLETED" : terminal === "cancelled" ? "CANCELLED" : "ALL";
  const result = useQuery(WorkTaskHistoryDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, text: query, kind, first: 50 },
    fetchPolicy: "cache-and-network"
  });
  const connection = result.data?.taskHistory;
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  return (
    <section aria-label="Task history" {...stylex.props(styles.history)}>
      <div {...stylex.props(styles.inlineFilters)}>
        <label {...stylex.props(styles.filterLabel)}>Show
          <select value={terminal} {...stylex.props(styles.select)} onChange={(event) => onTerminalChange(event.currentTarget.value as typeof terminal)}>
            <option value="all">Done and cancelled</option><option value="completed">Done</option><option value="cancelled">Cancelled</option>
          </select>
        </label>
      </div>
      {!connection ? <QueryState loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="history" /> : null}
      {connection && !tasks.length ? <EmptyState title="No matching history" detail="Done and cancelled tasks remain available here." /> : null}
      {tasks.length ? <TaskRows tasks={tasks} label="Task history" terminal /> : null}
      <LoadMore connection={connection} loading={result.loading} onLoad={() => result.fetchMore({ variables: { after: connection?.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, taskHistory: { ...fetchMoreResult.taskHistory, edges: [...previous.taskHistory.edges, ...fetchMoreResult.taskHistory.edges] } }) })} />
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
  return "Recovery";
}

const styles = stylex.create({
  dashboard: { display: "grid", alignContent: "start", gap: "var(--spacing-4)", minHeight: 0, padding: "var(--spacing-3)", "@media (max-width: 760px)": { gap: "var(--spacing-3)", padding: "var(--spacing-2)" } },
  group: { display: "grid", alignContent: "start", minWidth: 0 },
  sectionHeader: { display: "flex", minHeight: 40, alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-3)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-2)" },
  sectionCopy: { display: "grid", minWidth: 0, gap: "var(--spacing-0-5)" },
  sectionTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 700 },
  attentionTitle: { color: "var(--noema-clay-700)" },
  sectionDetail: { overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  count: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 10 },
  rows: { display: "grid", minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  attentionRow: { display: "grid", gridTemplateColumns: "88px minmax(0, 1fr) minmax(110px, auto)", minHeight: 48, alignItems: "center", gap: "var(--spacing-3)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingInline: "var(--spacing-2)", color: "inherit", textDecoration: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: -2 }, "@media (max-width: 640px)": { gridTemplateColumns: "72px minmax(0, 1fr)", paddingBlock: "var(--spacing-1)" } },
  attentionKind: { color: "var(--noema-clay-700)", fontSize: 10, fontWeight: 700 },
  attentionAction: { color: "var(--noema-pine-700)", fontSize: 11, fontWeight: 650, "@media (max-width: 640px)": { display: "none" } },
  table: { display: "grid", minWidth: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)" },
  tableHeader: { position: "sticky", top: 0, zIndex: 1, display: "grid", gridTemplateColumns: "minmax(240px, 2fr) minmax(100px, .8fr) minmax(100px, .7fr) 76px", minHeight: 30, alignItems: "center", gap: "var(--spacing-3)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", backgroundColor: "var(--noema-surface-card)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650, "@media (max-width: 700px)": { gridTemplateColumns: "minmax(0, 1fr) 72px" } },
  tableRow: { display: "grid", gridTemplateColumns: "minmax(240px, 2fr) minmax(100px, .8fr) 76px", minHeight: 48, alignItems: "center", gap: "var(--spacing-3)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 11, textDecoration: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: -2 }, "@media (max-width: 700px)": { gridTemplateColumns: "minmax(0, 1fr) 72px" } },
  historyRow: { gridTemplateColumns: "minmax(240px, 2fr) minmax(100px, .8fr) minmax(100px, .7fr) 76px", "@media (max-width: 700px)": { gridTemplateColumns: "minmax(0, 1fr) 72px" } },
  rowCopy: { display: "grid", minWidth: 0, gap: "var(--spacing-0-5)" },
  rowTitle: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 650, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  rowNote: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  mobileContext: { display: "none", overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 9, textOverflow: "ellipsis", whiteSpace: "nowrap", "@media (max-width: 700px)": { display: "block" } },
  cell: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  projectCell: { "@media (max-width: 700px)": { display: "none" } },
  stageCell: { "@media (max-width: 700px)": { display: "none" } },
  hiddenMobile: { "@media (max-width: 700px)": { display: "none" } },
  history: { display: "grid", alignContent: "start", gap: "var(--spacing-2)", minHeight: 0, padding: "var(--spacing-3)", "@media (max-width: 760px)": { padding: "var(--spacing-2)" } },
  inlineFilters: { display: "flex", minHeight: 32, flexWrap: "wrap", alignItems: "center", gap: "var(--spacing-3)" },
  filterLabel: { display: "flex", alignItems: "center", gap: "var(--spacing-1)", color: "var(--noema-text-muted)", fontSize: 11, fontWeight: 650 },
  select: { minHeight: 28, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 6, backgroundColor: "var(--noema-surface-card)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-primary)", font: "inherit", fontSize: 11 },
  state: { display: "flex", minHeight: 120, alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", color: "var(--noema-text-muted)", fontSize: 12 },
  stateCompact: { minHeight: 64, fontSize: 11 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-pine-700)", font: "inherit", fontWeight: 650, textDecoration: "underline", cursor: "pointer" },
  empty: { display: "grid", maxWidth: 420, minHeight: 160, alignContent: "center", justifyItems: "start", gap: "var(--spacing-1)", padding: "var(--spacing-3)", color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.4 }
});
