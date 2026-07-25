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
}> = [
  { behavior: "ACTIVE", title: "Running" },
  { behavior: "DISPATCH", title: "Up next" },
  { behavior: "INTAKE", title: "Inbox" }
];

export function WorkTasks({
  projectId,
  query,
  terminal
}: {
  projectId?: string;
  query?: string;
  terminal: "all" | "completed" | "cancelled";
}) {
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
  const initialLoading = !taskConnection && !attentionConnection && taskResult.loading && attentionResult.loading;

  return (
    <div {...stylex.props(styles.dashboard)}>
      <div role="table" aria-label="Tasks" aria-colcount={4} {...stylex.props(styles.workTable)}>
        <TableHeader />
        {initialLoading ? (
          <TableMessage loading error={false} retry={() => Promise.all([taskResult.refetch(), attentionResult.refetch()])} label="tasks" />
        ) : (
          <>
            {!attentionConnection ? (
              <TableMessage loading={attentionResult.loading} error={Boolean(attentionResult.error)} retry={() => attentionResult.refetch()} label="attention queue" />
            ) : null}
            {!actionResult.data && actionResult.error ? (
              <TableMessage loading={false} error retry={() => actionResult.refetch()} label="approvals" />
            ) : null}
            {attentionConnection && (attentionItems.length > 0 || actions.length > 0) ? (
              <AttentionGroup
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
              <TableMessage loading={taskResult.loading} error={Boolean(taskResult.error)} retry={() => taskResult.refetch()} label="tasks" />
            ) : null}
            {groups.map((group) => (
              <TaskGroup key={group.behavior} title={group.title} tasks={group.tasks} />
            ))}
            <TableLoadMore
              visible={Boolean(taskConnection?.pageInfo.hasNextPage)}
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
            <WorkHistory
              projectId={projectId}
              query={query}
              terminal={terminal}
            />
          </>
        )}
      </div>
    </div>
  );
}

function TableHeader() {
  return (
    <div role="row" {...stylex.props(styles.rowGrid, styles.tableHeader)}>
      <span role="columnheader">Task</span>
      <span role="columnheader" {...stylex.props(styles.hiddenMobile)}>Project</span>
      <span role="columnheader" {...stylex.props(styles.hiddenMobile)}>Status</span>
      <span role="columnheader">Updated</span>
    </div>
  );
}

function AttentionGroup({
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
  return (
    <section role="rowgroup" aria-labelledby="work-needs-you">
      <SectionRow id="work-needs-you" title="Needs you" count={items.length + actions.length} attention />
      {actions.length ? (
        <div role="row" {...stylex.props(styles.rowGrid)}>
          <div role="cell" aria-colspan={4} {...stylex.props(styles.actionCell)}>
            <GovernedActionList embedded compact actions={actions} onResolved={onResolved} />
          </div>
        </div>
      ) : null}
      {items.map((item) => <AttentionRow key={`${item.task.taskId}:${item.kind}`} item={item} />)}
      <TableLoadMore visible={hasNextPage} loading={loading} onLoad={onLoadMore} />
    </section>
  );
}

type WorkNeedsYouItems = WorkNeedsYouQuery["needsYou"]["edges"][number]["node"][];
type GovernedActions = PendingGovernedActionsQuery["pendingGovernedActions"];

function AttentionRow({ item }: { item: WorkNeedsYouItems[number] }) {
  const status = item.title;
  return (
    <div role="row" {...stylex.props(styles.rowGrid, styles.taskRow)}>
      <div role="cell" {...stylex.props(styles.rowCopy)}>
        <Link
          to="/work/tasks/$taskId"
          params={{ taskId: item.task.taskId }}
          search={(current) => normalizeWorkSearch(current)}
          {...stylex.props(styles.rowLink)}
        >
          <strong {...stylex.props(styles.rowTitle)}>{item.task.title}</strong>
          <span {...stylex.props(styles.rowNote)}>{attentionLabel(item.kind)} · {item.summary}</span>
          <span {...stylex.props(styles.mobileContext)}>{item.task.project?.name ?? "No project"} · {status}</span>
        </Link>
      </div>
      <span role="cell" {...stylex.props(styles.cell, styles.hiddenMobile)}>{item.task.project?.name ?? "—"}</span>
      <span role="cell" {...stylex.props(styles.cell, styles.attentionStatus, styles.hiddenMobile)}>{status}</span>
      <time role="cell" dateTime={item.task.updatedAt} title={timestampLabel(item.task.updatedAt)} {...stylex.props(styles.cell)}>{relativeTime(item.task.updatedAt)}</time>
    </div>
  );
}

function TaskGroup({ title, tasks }: { title: string; tasks: readonly WorkTask[] }) {
  const id = `work-group-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <section role="rowgroup" aria-labelledby={id}>
      <SectionRow id={id} title={title} count={tasks.length} />
      {tasks.map((task) => (
        <TaskRow key={task.taskId} task={task} status={taskRunLabel(task) ?? task.stage.name} />
      ))}
    </section>
  );
}

function SectionRow({ id, title, count, attention = false }: { id: string; title: string; count: number; attention?: boolean }) {
  return (
    <div role="row" {...stylex.props(styles.rowGrid, styles.sectionRow)}>
      <div role="columnheader" aria-colspan={4} {...stylex.props(styles.sectionCell)}>
        <h2 id={id} {...stylex.props(styles.sectionTitle, attention && styles.attentionTitle)}>{title}</h2>
        <span aria-label={`${count} items`} {...stylex.props(styles.count)}>{count}</span>
      </div>
    </div>
  );
}

function TaskRow({ task, status }: { task: WorkTask; status: string }) {
  const timestamp = task.completedAt ?? task.updatedAt;
  return (
    <div role="row" {...stylex.props(styles.rowGrid, styles.taskRow)}>
      <div role="cell" {...stylex.props(styles.rowCopy)}>
        <Link
          to="/work/tasks/$taskId"
          params={{ taskId: task.taskId }}
          search={(current) => normalizeWorkSearch(current)}
          {...stylex.props(styles.rowLink)}
        >
          <strong {...stylex.props(styles.rowTitle)}>{task.title}</strong>
          <span {...stylex.props(styles.rowNote)}>{task.descriptionPreview}</span>
          <span {...stylex.props(styles.mobileContext)}>{task.project?.name ?? "No project"} · {status}</span>
        </Link>
      </div>
      <span role="cell" {...stylex.props(styles.cell, styles.hiddenMobile)}>{task.project?.name ?? "—"}</span>
      <span role="cell" {...stylex.props(styles.cell, styles.hiddenMobile)}>{status}</span>
      <time role="cell" dateTime={timestamp} title={timestampLabel(timestamp)} {...stylex.props(styles.cell)}>{relativeTime(timestamp)}</time>
    </div>
  );
}

function WorkHistory({ projectId, query, terminal }: { projectId?: string; query?: string; terminal: "all" | "completed" | "cancelled" }) {
  const kind = terminal === "completed" ? "COMPLETED" : terminal === "cancelled" ? "CANCELLED" : "ALL";
  const result = useQuery(WorkTaskHistoryDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, text: query, kind, first: 10 },
    fetchPolicy: "cache-and-network"
  });
  const connection = result.data?.taskHistory;
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  return (
    <section role="rowgroup" aria-labelledby="work-history">
      <SectionRow id="work-history" title="History" count={tasks.length} />
      {!connection ? <TableMessage loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="history" /> : null}
      {connection && !tasks.length ? <TableEmpty title="No matching history" detail="Done and cancelled tasks remain available here." /> : null}
      {tasks.map((task) => <TaskRow key={task.taskId} task={task} status={task.stage.name} />)}
      <TableLoadMore visible={Boolean(connection?.pageInfo.hasNextPage)} loading={result.loading} onLoad={() => result.fetchMore({ variables: { after: connection?.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, taskHistory: { ...fetchMoreResult.taskHistory, edges: [...previous.taskHistory.edges, ...fetchMoreResult.taskHistory.edges] } }) })} />
    </section>
  );
}

function TableMessage({ loading, error, retry, label }: { loading: boolean; error: boolean; retry: () => unknown; label: string }) {
  const content = loading ? <>Loading {label}…</> : error ? <>Could not load {label}. <button type="button" {...stylex.props(styles.retry)} onClick={() => retry()}>Retry</button></> : null;
  if (!content) return null;
  return (
    <div role="row" {...stylex.props(styles.rowGrid)}>
      <div role="cell" aria-colspan={4} {...stylex.props(styles.fullWidthCell, styles.state)}>{content}</div>
    </div>
  );
}

function TableEmpty({ title, detail }: { title: string; detail: string }) {
  return (
    <div role="row" {...stylex.props(styles.rowGrid)}>
      <div role="cell" aria-colspan={4} {...stylex.props(styles.fullWidthCell, styles.empty)}>
        <strong>{title}</strong><span>{detail}</span>
      </div>
    </div>
  );
}

function TableLoadMore({ visible, loading, onLoad }: { visible: boolean; loading: boolean; onLoad: () => unknown }) {
  if (!visible) return null;
  return (
    <div role="row" {...stylex.props(styles.rowGrid)}>
      <div role="cell" aria-colspan={4} {...stylex.props(styles.fullWidthCell, styles.loadMoreCell)}>
        <Button size="sm" variant="ghost" label="Load more" isLoading={loading} onClick={() => onLoad()} />
      </div>
    </div>
  );
}

function attentionLabel(kind: string): string {
  if (kind === "CLARIFICATION_REQUIRED") return "Question";
  if (kind === "APPROVAL_REQUIRED") return "Approval";
  return "Recovery";
}

const styles = stylex.create({
  dashboard: { minHeight: 0, padding: "var(--spacing-3)", "@media (max-width: 760px)": { padding: "var(--spacing-2)" } },
  workTable: { display: "grid", minWidth: 0, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 8, backgroundColor: "var(--noema-surface-card)" },
  rowGrid: { display: "grid", gridTemplateColumns: "minmax(240px, 2fr) minmax(100px, .8fr) minmax(120px, .8fr) 76px", minWidth: 0, alignItems: "center", gap: "var(--spacing-3)", paddingInline: "var(--spacing-2)", "@media (max-width: 700px)": { gridTemplateColumns: "minmax(0, 1fr) 72px" } },
  tableHeader: { position: "sticky", top: 0, zIndex: 2, minHeight: 30, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", borderTopLeftRadius: 8, borderTopRightRadius: 8, backgroundColor: "var(--noema-surface-card)", color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650 },
  sectionRow: { paddingInline: 0, backgroundColor: "var(--noema-surface-card)" },
  sectionCell: { display: "flex", gridColumn: "1 / -1", minHeight: 24, minWidth: 0, alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-2)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", paddingInline: "var(--spacing-2)" },
  sectionTitle: { margin: 0, color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650 },
  attentionTitle: { color: "var(--noema-clay-700)" },
  count: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 9 },
  taskRow: { position: "relative", minHeight: 48, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", color: "var(--noema-text-secondary)", fontSize: 11, ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-within": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: -2 } },
  rowCopy: { display: "grid", minWidth: 0, gap: "var(--spacing-0-5)" },
  rowLink: { display: "grid", minWidth: 0, gap: "var(--spacing-0-5)", color: "inherit", textDecoration: "none", "::after": { content: "''", position: "absolute", zIndex: 1, inset: 0 } },
  rowTitle: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-primary)", fontSize: 12, fontWeight: 650, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  rowNote: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  mobileContext: { display: "none", overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 9, textOverflow: "ellipsis", whiteSpace: "nowrap", "@media (max-width: 700px)": { display: "block" } },
  cell: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  attentionStatus: { color: "var(--noema-pine-700)", fontWeight: 650 },
  hiddenMobile: { "@media (max-width: 700px)": { display: "none" } },
  fullWidthCell: { gridColumn: "1 / -1", marginInline: "calc(-1 * var(--spacing-2))", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)" },
  actionCell: { gridColumn: "1 / -1", marginInline: "calc(-1 * var(--spacing-2))" },
  state: { display: "flex", minHeight: 64, alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", padding: "var(--spacing-2)", color: "var(--noema-text-muted)", fontSize: 11 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-pine-700)", font: "inherit", fontWeight: 650, textDecoration: "underline", cursor: "pointer" },
  empty: { display: "grid", minHeight: 72, alignContent: "center", justifyItems: "start", gap: "var(--spacing-1)", padding: "var(--spacing-3)", color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.4 },
  loadMoreCell: { paddingBlock: "var(--spacing-1)", paddingInline: "var(--spacing-2)" },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" }
});
