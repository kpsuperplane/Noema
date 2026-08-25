import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { AnimatePresence } from "motion/react";
import { ListCardLink } from "@/components/ListCardLink";
import {
  HumanInterventionList,
  HumanInterventionMotionItem,
  humanInterventionKey
} from "@/components/actions/PendingGovernedActions";
import {
  TaskStatusBadge,
  taskStatusFromProjection
} from "@/components/chatDetail/task/TaskStatusBadge";
import type { TaskStatus } from "@/components/chatDetail/task/taskTypes";
import {
  TasksOverviewDocument,
  type TasksOverviewQuery
} from "@/generated/graphql";
import { recurrenceSummary, relativeTime, taskRunLabel, timestampLabel } from "./tasksModel";
import { normalizeTasksSearch, PERSONAL_WORKSPACE_ID, type TasksTask } from "./tasksTypes";

export function TasksList({
  projectId,
  selectedTaskId,
  terminal
}: {
  projectId?: string;
  selectedTaskId?: string;
  terminal: "all" | "completed" | "cancelled";
}) {
  const historyKind = terminal === "completed" ? "COMPLETED" : terminal === "cancelled" ? "CANCELLED" : "ALL";
  const rootResult = useQuery(TasksOverviewDocument, {
    variables: {
      input: { workspaceId: PERSONAL_WORKSPACE_ID, projectId, scope: "ACTIVE" },
      workspaceId: PERSONAL_WORKSPACE_ID,
      projectId,
      historyKind
    },
    fetchPolicy: "cache-and-network",
    errorPolicy: "all"
  });
  const taskConnection = rootResult.data?.tasks;
  const tasks = taskConnection?.edges.map((edge) => edge.node) ?? [];
  const recurrences = rootResult.data?.taskRecurrences ?? [];
  const interventions = rootResult.data?.pendingHumanInterventions ?? [];
  const visibleInterventions = selectedTaskId
    ? interventions.filter((intervention) => (
        intervention.__typename === "TaskAttention"
        || interventionTaskId(intervention) !== selectedTaskId
      ))
    : interventions;
  const running = tasks.filter((task) => task.stage.behavior === "ACTIVE");
  const upNext = tasks.filter((task) => task.stage.behavior === "DISPATCH");
  const inbox = tasks.filter((task) => task.stage.behavior === "INTAKE" && !task.schedule);
  const oneTimeScheduled = tasks.filter((task) => task.stage.behavior === "INTAKE" && task.schedule && !task.schedule.recurrenceId);
  const initialLoading = !rootResult.data && rootResult.loading;

  return (
    <div {...stylex.props(styles.dashboard)}>
      <VStack aria-label="Tasks" gap={4} className={stylex.props(styles.taskList).className}>
        {initialLoading ? (
          <ListMessage loading error={false} retry={() => rootResult.refetch()} label="tasks" />
        ) : (
          <>
            {!rootResult.data ? (
              <ListMessage loading={rootResult.loading} error={Boolean(rootResult.error)} retry={() => rootResult.refetch()} label="recurring tasks" />
            ) : null}
            <AnimatePresence>
              {visibleInterventions.length > 0 ? (
                <HumanInterventionMotionItem key="tasks-needs-you" exitGap="var(--spacing-4)">
                  <AttentionGroup
                    interventions={visibleInterventions}
                    onResolved={() => void rootResult.refetch()}
                    onRetry={() => rootResult.refetch()}
                    selectedTaskId={selectedTaskId}
                  />
                </HumanInterventionMotionItem>
              ) : null}
            </AnimatePresence>
            {!taskConnection ? (
              <ListMessage loading={rootResult.loading} error={Boolean(rootResult.error)} retry={() => rootResult.refetch()} label="tasks" />
            ) : null}
            {running.length ? <TaskGroup title="Running" tasks={running} /> : null}
            {oneTimeScheduled.length || recurrences.length ? <ScheduledGroup oneTimeTasks={oneTimeScheduled} recurrences={recurrences} /> : null}
            {upNext.length ? <TaskGroup title="Up next" tasks={upNext} /> : null}
            {inbox.length ? <TaskGroup title="Inbox" tasks={inbox} /> : null}
            <ListLoadMore
              visible={Boolean(taskConnection?.pageInfo.hasNextPage)}
              loading={rootResult.loading}
              onLoad={() => rootResult.fetchMore({
                variables: { activeAfter: taskConnection?.pageInfo.endCursor },
                updateQuery: (previous, { fetchMoreResult }) => ({
                  ...fetchMoreResult,
                  tasks: {
                    ...fetchMoreResult.tasks,
                    edges: [...previous.tasks.edges, ...fetchMoreResult.tasks.edges]
                  }
                })
              })}
            />
            <TasksHistory
              connection={rootResult.data?.taskHistory}
              error={Boolean(rootResult.error)}
              loading={rootResult.loading}
              onRefetch={rootResult.refetch}
              onLoadMore={(historyAfter) => rootResult.fetchMore({
                variables: { historyAfter },
                updateQuery: (previous, { fetchMoreResult }) => ({
                  ...fetchMoreResult,
                  taskHistory: {
                    ...fetchMoreResult.taskHistory,
                    edges: [...previous.taskHistory.edges, ...fetchMoreResult.taskHistory.edges]
                  }
                })
              })}
            />
          </>
        )}
      </VStack>
    </div>
  );
}

function AttentionGroup({
  interventions,
  onResolved,
  onRetry,
  selectedTaskId
}: {
  interventions: HumanInterventions;
  onResolved: () => void;
  onRetry: () => Promise<unknown>;
  selectedTaskId?: string;
}) {
  const attachedInterventions: AttachedIntervention[] = [];
  const otherInterventions: HumanInterventions = [];
  for (const intervention of interventions) {
    const task = intervention.__typename === "TaskAttention"
      ? intervention.task
      : intervention.__typename === "GovernedAction"
        ? intervention.actionTask
        : undefined;
    if (task) attachedInterventions.push({ intervention, task });
    else otherInterventions.push(intervention);
  }
  return (
    <VStack as="section" aria-labelledby="tasks-needs-you" gap={1.5} className={stylex.props(styles.taskGroup).className}>
      <SectionHeader id="tasks-needs-you" title="Needs you" count={interventions.length} attention />
      <AnimatePresence initial={false}>
        {attachedInterventions.length ? (
          <HumanInterventionMotionItem key="task-interventions" exitGap="var(--spacing-1-5)">
            <VStack as="div" role="list" gap={1.5} className={stylex.props(styles.cards).className}>
              <AnimatePresence initial={false}>
                {attachedInterventions.map(({ intervention, task }) => (
                  <HumanInterventionMotionItem
                    key={humanInterventionKey(intervention)}
                    exitGap="var(--spacing-1-5)"
                    role="listitem"
                  >
                    <AttachedTaskIntervention
                      intervention={intervention}
                      onResolved={onResolved}
                      onRetry={onRetry}
                      showAction={task.taskId !== selectedTaskId}
                      task={task}
                    />
                  </HumanInterventionMotionItem>
                ))}
              </AnimatePresence>
            </VStack>
          </HumanInterventionMotionItem>
        ) : null}
      </AnimatePresence>
      <AnimatePresence initial={false}>
        {otherInterventions.length ? (
          <HumanInterventionMotionItem key="other-task-interventions" exitGap="var(--spacing-1-5)">
            <HumanInterventionList
              placement="queue"
              interventions={otherInterventions}
              onResolved={onResolved}
              onRetry={onRetry}
              initialAnimation={false}
            />
          </HumanInterventionMotionItem>
        ) : null}
      </AnimatePresence>
    </VStack>
  );
}

type HumanInterventions = TasksOverviewQuery["pendingHumanInterventions"];
type AttachedTask = Pick<TasksTask, "project" | "taskId" | "title" | "updatedAt">;
type AttachedIntervention = {
  intervention: HumanInterventions[number];
  task: AttachedTask;
};

function interventionTaskId(intervention: HumanInterventions[number]) {
  switch (intervention.__typename) {
    case "TaskAttention": return intervention.task.taskId;
    case "GovernedAction":
    case "McpAuthenticationIntervention":
    case "AdapterAuthenticationIntervention": return intervention.taskId;
    case "McpSetupIntervention":
    case "AdapterOauthClientSetupIntervention":
    case "AdapterDefinition": return undefined;
  }
}

function AttachedTaskIntervention({
  intervention,
  onResolved,
  onRetry,
  showAction,
  task
}: {
  intervention: HumanInterventions[number];
  onResolved: () => void;
  onRetry: () => Promise<unknown>;
  showAction: boolean;
  task: AttachedTask;
}) {
  return (
    <VStack
      as="div"
      gap={0}
      className={stylex.props(styles.attachedTask).className}
    >
      <AnimatePresence initial={false}>
        {showAction ? (
          <HumanInterventionMotionItem key="attached-task-action">
            <HumanInterventionList
              placement="task"
              interventions={[intervention]}
              onResolved={onResolved}
              onRetry={onRetry}
              animateItems={false}
              initialAnimation={false}
            />
          </HumanInterventionMotionItem>
        ) : null}
      </AnimatePresence>
      <TaskCard
        taskId={task.taskId}
        title={task.title}
        project={task.project?.name}
        status="waiting_for_human"
        statusLabel="Needs you"
        timestamp={task.updatedAt}
        listItem={false}
        attached
      />
    </VStack>
  );
}

function TaskGroup({ title, tasks }: { title: string; tasks: readonly TasksTask[] }) {
  const id = `tasks-group-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <VStack as="section" aria-labelledby={id} gap={1.5} className={stylex.props(styles.taskGroup).className}>
      <SectionHeader id={id} title={title} count={tasks.length} />
      <VStack as="ul" gap={1.5} className={stylex.props(styles.cards).className}>
        {tasks.map((task) => (
          <TaskCard key={task.taskId} taskId={task.taskId} title={task.title} note={task.taskDocumentPreview} project={task.project?.name} status={taskStatusFromProjection(task)} statusLabel={taskRunLabel(task) ?? task.stage.name} timestamp={task.completedAt ?? task.updatedAt} />
        ))}
      </VStack>
    </VStack>
  );
}

type TasksRecurrence = TasksOverviewQuery["taskRecurrences"][number];

function ScheduledGroup({ oneTimeTasks, recurrences }: { oneTimeTasks: readonly TasksTask[]; recurrences: readonly TasksRecurrence[] }) {
  return (
    <VStack as="section" aria-labelledby="tasks-group-scheduled" gap={1.5} className={stylex.props(styles.taskGroup).className}>
      <SectionHeader id="tasks-group-scheduled" title="Scheduled" count={oneTimeTasks.length + recurrences.length} />
      <VStack as="ul" gap={1.5} className={stylex.props(styles.cards).className}>
        {oneTimeTasks.map((task) => (
          <TaskCard key={task.taskId} taskId={task.taskId} title={task.title} note={task.taskDocumentPreview} project={task.project?.name} status="queued" statusLabel="Scheduled" timestamp={task.schedule!.scheduledFor} />
        ))}
        {recurrences.map((recurrence) => <RecurrenceTaskCard key={recurrence.recurrenceId} recurrence={recurrence} />)}
      </VStack>
    </VStack>
  );
}

function RecurrenceTaskCard({ recurrence }: { recurrence: TasksRecurrence }) {
  const timestamp = recurrence.nextRunAt ?? recurrence.updatedAt;
  const repeat = recurrenceSummary(recurrence.cronExpression);
  return (
    <TaskCard
      recurrenceId={recurrence.recurrenceId}
      title={recurrence.title}
      note={recurrence.nextRunAt ? `${repeat} · Next ${timestampLabel(recurrence.nextRunAt)}` : repeat}
      status="queued"
      statusLabel={recurrence.lifecycle === "PAUSED" ? "Paused" : "Recurring"}
      timestamp={timestamp}
    />
  );
}

function SectionHeader({ id, title, count, attention = false }: { id: string; title: string; count: number; attention?: boolean }) {
  return (
    <HStack justify="between" align="center" gap={2} className={stylex.props(styles.sectionHeader).className}>
      <h2 id={id} {...stylex.props(styles.sectionTitle, attention && styles.attentionTitle)}>{title}</h2>
      <span aria-label={`${count} items`} {...stylex.props(styles.count)}>{count}</span>
    </HStack>
  );
}

function TaskCard({ taskId = "", recurrenceId, title, note, project, status, statusLabel, timestamp, listItem = true, attached = false }: {
  taskId?: string;
  recurrenceId?: string;
  title: string;
  note?: string | null;
  project?: string | null;
  status: TaskStatus;
  statusLabel: string;
  timestamp: string;
  listItem?: boolean;
  attached?: boolean;
}) {
  const link = (
    <ListCardLink
      to={recurrenceId ? "/tasks/recurrences/$recurrenceId" : "/tasks/$taskId"}
      params={recurrenceId ? { recurrenceId } : { taskId }}
      search={(current) => normalizeTasksSearch(current)}
      activeOptions={{ exact: true, includeSearch: false }}
      activeProps={{ selected: true, "aria-current": "page" }}
      xstyle={[styles.taskCardLayout, attached && styles.attachedTaskCard]}
    >
      <div {...stylex.props(styles.cardHeading)}>
        <strong {...stylex.props(styles.cardTitle)}>{title}</strong>
        <time dateTime={timestamp} title={timestampLabel(timestamp)} {...stylex.props(styles.cardTime)}>{relativeTime(timestamp)}</time>
      </div>
      {note ? <span {...stylex.props(styles.cardPreview)}>{note}</span> : null}
      <HStack gap={1} align="center" className={stylex.props(styles.cardMeta).className}>
        <TaskStatusBadge status={status} label={statusLabel} />
        {project ? (
          <>
            <span aria-hidden="true">·</span>
            <span {...stylex.props(styles.cardProject)}>{project}</span>
          </>
        ) : null}
      </HStack>
    </ListCardLink>
  );
  return listItem ? <li {...stylex.props(styles.cardListItem)}>{link}</li> : link;
}

function TasksHistory({ connection, error, loading, onRefetch, onLoadMore }: {
  connection?: TasksOverviewQuery["taskHistory"];
  error: boolean;
  loading: boolean;
  onRefetch: () => unknown;
  onLoadMore: (after: string) => unknown;
}) {
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  return (
    <VStack as="section" aria-labelledby="tasks-history" gap={1.5} className={stylex.props(styles.taskGroup).className}>
      <SectionHeader id="tasks-history" title="History" count={tasks.length} />
      {!connection ? <ListMessage loading={loading} error={error} retry={onRefetch} label="history" /> : null}
      {connection && !tasks.length ? <ListEmpty title="No matching history" detail="Done and cancelled tasks remain available here." /> : null}
      <VStack as="ul" gap={1.5} className={stylex.props(styles.cards).className}>
        {tasks.map((task) => <TaskCard key={task.taskId} taskId={task.taskId} title={task.title} note={task.taskDocumentPreview} project={task.project?.name} status={taskStatusFromProjection(task)} statusLabel={task.stage.name} timestamp={task.completedAt ?? task.updatedAt} />)}
      </VStack>
      <ListLoadMore visible={Boolean(connection?.pageInfo.hasNextPage)} loading={loading} onLoad={() => {
        const cursor = connection?.pageInfo.endCursor;
        if (cursor) onLoadMore(cursor);
      }} />
    </VStack>
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
    <HStack justify="center" className={stylex.props(styles.loadMore).className}>
      <Button size="sm" variant="ghost" label="Load more" isLoading={loading} onClick={() => onLoad()} />
    </HStack>
  );
}

const styles = stylex.create({
  dashboard: { minHeight: 0, paddingBlock: "var(--spacing-3)", "@media (max-width: 760px)": { paddingBlock: "var(--spacing-2)" } },
  taskList: { minWidth: 0 },
  taskGroup: { minWidth: 0 },
  cards: { minWidth: 0, margin: "var(--spacing-0)", padding: "var(--spacing-0)", listStyle: "none" },
  cardListItem: { minWidth: 0, listStyle: "none" },
  sectionHeader: { minHeight: 24, minWidth: 0, paddingInline: "var(--spacing-1)" },
  sectionTitle: { margin: "var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: 12, fontWeight: 650 },
  attentionTitle: { color: "var(--noema-clay-700)" },
  count: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 12, fontVariantNumeric: "tabular-nums" },
  taskCardLayout: { gap: "var(--spacing-1)" },
  cardHeading: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", minWidth: 0, alignItems: "baseline", gap: "var(--spacing-2)" },
  cardTitle: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 650, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  cardTime: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 12, fontVariantNumeric: "tabular-nums" },
  cardPreview: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  cardMeta: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 12 },
  cardProject: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  attachedTask: {
    position: "relative",
    minWidth: 0,
    "--human-intervention-card-radius": "10px",
    "--human-intervention-card-bottom-radius": "0px",
    "--human-intervention-card-overlap": "10px"
  },
  attachedTaskCard: {
    position: "relative",
    zIndex: 1,
    ":hover": { backgroundColor: "var(--noema-surface-sunken)" }
  },
  state: { display: "flex", minHeight: 64, alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, padding: "var(--spacing-2)", color: "var(--noema-text-muted)", fontSize: 12 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: "var(--spacing-0)", color: "var(--noema-pine-700)", font: "inherit", fontWeight: 650, textDecoration: "underline", cursor: "pointer" },
  empty: { display: "grid", minHeight: 72, alignContent: "center", justifyItems: "start", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, padding: "var(--spacing-3)", color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.4 },
  loadMore: { paddingBlock: "var(--spacing-1)" }
});
