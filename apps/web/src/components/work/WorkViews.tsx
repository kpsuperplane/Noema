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
      <div aria-label="Tasks" {...stylex.props(styles.taskList)}>
        {initialLoading ? (
          <ListMessage loading error={false} retry={() => Promise.all([taskResult.refetch(), attentionResult.refetch()])} label="tasks" />
        ) : (
          <>
            {!attentionConnection ? (
              <ListMessage loading={attentionResult.loading} error={Boolean(attentionResult.error)} retry={() => attentionResult.refetch()} label="attention queue" />
            ) : null}
            {!actionResult.data && actionResult.error ? (
              <ListMessage loading={false} error retry={() => actionResult.refetch()} label="approvals" />
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
              <ListMessage loading={taskResult.loading} error={Boolean(taskResult.error)} retry={() => taskResult.refetch()} label="tasks" />
            ) : null}
            {groups.map((group) => (
              <TaskGroup key={group.behavior} title={group.title} tasks={group.tasks} />
            ))}
            <ListLoadMore
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
    <section aria-labelledby="work-needs-you" {...stylex.props(styles.taskGroup)}>
      <SectionHeader id="work-needs-you" title="Needs you" count={items.length + actions.length} attention />
      {actions.length ? (
        <div {...stylex.props(styles.actionCard)}>
          <GovernedActionList embedded compact actions={actions} onResolved={onResolved} />
        </div>
      ) : null}
      <div role="list" {...stylex.props(styles.cards)}>
        {items.map((item) => <AttentionCard key={`${item.task.taskId}:${item.kind}`} item={item} />)}
      </div>
      <ListLoadMore visible={hasNextPage} loading={loading} onLoad={onLoadMore} />
    </section>
  );
}

type WorkNeedsYouItems = WorkNeedsYouQuery["needsYou"]["edges"][number]["node"][];
type GovernedActions = PendingGovernedActionsQuery["pendingGovernedActions"];

function AttentionCard({ item }: { item: WorkNeedsYouItems[number] }) {
  const status = item.title;
  return (
    <TaskCard
      taskId={item.task.taskId}
      title={item.task.title}
      note={item.summary}
      project={item.task.project?.name}
      status={`${attentionLabel(item.kind)} · ${status}`}
      timestamp={item.task.updatedAt}
      attention
    />
  );
}

function TaskGroup({ title, tasks }: { title: string; tasks: readonly WorkTask[] }) {
  const id = `work-group-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <section aria-labelledby={id} {...stylex.props(styles.taskGroup)}>
      <SectionHeader id={id} title={title} count={tasks.length} />
      <div role="list" {...stylex.props(styles.cards)}>
        {tasks.map((task) => (
          <TaskCard key={task.taskId} taskId={task.taskId} title={task.title} note={task.descriptionPreview} project={task.project?.name} status={taskRunLabel(task) ?? task.stage.name} timestamp={task.completedAt ?? task.updatedAt} />
        ))}
      </div>
    </section>
  );
}

function SectionHeader({ id, title, count, attention = false }: { id: string; title: string; count: number; attention?: boolean }) {
  return (
    <div {...stylex.props(styles.sectionHeader)}>
      <h2 id={id} {...stylex.props(styles.sectionTitle, attention && styles.attentionTitle)}>{title}</h2>
      <span aria-label={`${count} items`} {...stylex.props(styles.count)}>{count}</span>
    </div>
  );
}

function TaskCard({ taskId, title, note, project, status, timestamp, attention = false }: {
  taskId: string;
  title: string;
  note?: string | null;
  project?: string | null;
  status: string;
  timestamp: string;
  attention?: boolean;
}) {
  const cardStyles = stylex.props(styles.taskCard, attention && styles.attentionCard);
  const selectedCardStyles = stylex.props(styles.taskCard, attention && styles.attentionCard, styles.selectedCard);
  return (
    <Link
      role="listitem"
      to="/work/tasks/$taskId"
      params={{ taskId }}
      search={(current) => normalizeWorkSearch(current)}
      activeOptions={{ exact: true, includeSearch: false }}
      activeProps={{ ...selectedCardStyles, "aria-current": "page" }}
      {...cardStyles}
    >
      <div {...stylex.props(styles.cardHeading)}>
        <strong {...stylex.props(styles.cardTitle)}>{title}</strong>
        <time dateTime={timestamp} title={timestampLabel(timestamp)} {...stylex.props(styles.cardTime)}>{relativeTime(timestamp)}</time>
      </div>
      {note ? <span {...stylex.props(styles.cardPreview)}>{note}</span> : null}
      <div {...stylex.props(styles.cardMeta)}>
        <span {...stylex.props(attention ? styles.attentionStatus : styles.cardStatus)}>{status}</span>
        <span aria-hidden="true">·</span>
        <span {...stylex.props(styles.cardProject)}>{project ?? "No project"}</span>
      </div>
    </Link>
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
    <section aria-labelledby="work-history" {...stylex.props(styles.taskGroup)}>
      <SectionHeader id="work-history" title="History" count={tasks.length} />
      {!connection ? <ListMessage loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="history" /> : null}
      {connection && !tasks.length ? <ListEmpty title="No matching history" detail="Done and cancelled tasks remain available here." /> : null}
      <div role="list" {...stylex.props(styles.cards)}>
        {tasks.map((task) => <TaskCard key={task.taskId} taskId={task.taskId} title={task.title} note={task.descriptionPreview} project={task.project?.name} status={task.stage.name} timestamp={task.completedAt ?? task.updatedAt} />)}
      </div>
      <ListLoadMore visible={Boolean(connection?.pageInfo.hasNextPage)} loading={result.loading} onLoad={() => result.fetchMore({ variables: { after: connection?.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, taskHistory: { ...fetchMoreResult.taskHistory, edges: [...previous.taskHistory.edges, ...fetchMoreResult.taskHistory.edges] } }) })} />
    </section>
  );
}

function ListMessage({ loading, error, retry, label }: { loading: boolean; error: boolean; retry: () => unknown; label: string }) {
  const content = loading ? <>Loading {label}…</> : error ? <>Could not load {label}. <button type="button" {...stylex.props(styles.retry)} onClick={() => retry()}>Retry</button></> : null;
  if (!content) return null;
  return <div {...stylex.props(styles.state)}>{content}</div>;
}

function ListEmpty({ title, detail }: { title: string; detail: string }) {
  return (
    <div {...stylex.props(styles.empty)}>
      <strong>{title}</strong><span>{detail}</span>
    </div>
  );
}

function ListLoadMore({ visible, loading, onLoad }: { visible: boolean; loading: boolean; onLoad: () => unknown }) {
  if (!visible) return null;
  return (
    <div {...stylex.props(styles.loadMore)}>
      <Button size="sm" variant="ghost" label="Load more" isLoading={loading} onClick={() => onLoad()} />
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
  taskList: { display: "grid", minWidth: 0, gap: "var(--spacing-4)" },
  taskGroup: { display: "grid", minWidth: 0, gap: "var(--spacing-1-5)" },
  cards: { display: "grid", minWidth: 0, gap: "var(--spacing-1-5)" },
  sectionHeader: { display: "flex", minHeight: 24, minWidth: 0, alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-2)", paddingInline: "var(--spacing-1)" },
  sectionTitle: { margin: 0, color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650 },
  attentionTitle: { color: "var(--noema-clay-700)" },
  count: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 9 },
  taskCard: { display: "grid", minWidth: 0, gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, backgroundColor: "var(--noema-surface-card)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)", color: "var(--noema-text-secondary)", textDecoration: "none", boxShadow: "0 1px 2px color-mix(in srgb, black 4%, transparent)", ":hover": { borderColor: "var(--noema-border-default)", backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  selectedCard: { borderColor: "color-mix(in srgb, var(--noema-pine-500) 26%, var(--noema-border-subtle))", backgroundColor: "color-mix(in srgb, var(--noema-pine-50) 70%, var(--noema-surface-card))", boxShadow: "0 2px 8px color-mix(in srgb, var(--noema-pine-700) 9%, transparent)" },
  attentionCard: { borderColor: "color-mix(in srgb, var(--noema-clay-600) 28%, var(--noema-border-subtle))" },
  cardHeading: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", minWidth: 0, alignItems: "baseline", gap: "var(--spacing-2)" },
  cardTitle: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 650, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  cardTime: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 9 },
  cardPreview: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  cardMeta: { display: "flex", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)", overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10 },
  cardStatus: { flexShrink: 0, color: "var(--noema-text-secondary)", fontWeight: 650 },
  cardProject: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  attentionStatus: { color: "var(--noema-pine-700)", fontWeight: 650 },
  actionCard: { minWidth: 0, overflow: "hidden", borderWidth: 1, borderStyle: "solid", borderColor: "color-mix(in srgb, var(--noema-clay-600) 28%, var(--noema-border-subtle))", borderRadius: 10, backgroundColor: "var(--noema-surface-card)" },
  state: { display: "flex", minHeight: 64, alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, padding: "var(--spacing-2)", color: "var(--noema-text-muted)", fontSize: 11 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--noema-pine-700)", font: "inherit", fontWeight: 650, textDecoration: "underline", cursor: "pointer" },
  empty: { display: "grid", minHeight: 72, alignContent: "center", justifyItems: "start", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, padding: "var(--spacing-3)", color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.4 },
  loadMore: { display: "flex", justifyContent: "center", paddingBlock: "var(--spacing-1)" }
});
