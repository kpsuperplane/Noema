import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { AnimatePresence } from "motion/react";
import {
  HumanInterventionList,
  HumanInterventionMotionItem,
  usePendingHumanInterventions
} from "@/components/actions/PendingGovernedActions";
import {
  TaskStatusBadge,
  taskStatusFromProjection
} from "@/components/chatDetail/task/TaskStatusBadge";
import type { TaskStatus } from "@/components/chatDetail/task/taskTypes";
import {
  WorkTaskHistoryDocument,
  WorkTasksDocument,
  type PendingHumanInterventionsQuery,
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
  const actionResult = usePendingHumanInterventions({ projectId });
  const taskConnection = taskResult.data?.workTasks;
  const tasks = taskConnection?.edges.map((edge) => edge.node) ?? [];
  const interventions = actionResult.data?.pendingHumanInterventions ?? [];
  const groups = taskGroups
    .map((group) => ({ ...group, tasks: tasks.filter((task) => task.stage.behavior === group.behavior) }))
    .filter((group) => group.tasks.length > 0);
  const initialLoading = !taskConnection && !actionResult.data && taskResult.loading && actionResult.loading;

  return (
    <div {...stylex.props(styles.dashboard)}>
      <VStack aria-label="Tasks" gap={4} className={stylex.props(styles.taskList).className}>
        {initialLoading ? (
          <ListMessage loading error={false} retry={() => Promise.all([taskResult.refetch(), actionResult.refetch()])} label="tasks" />
        ) : (
          <>
            {!actionResult.data && actionResult.error ? (
              <ListMessage loading={false} error retry={() => actionResult.refetch()} label="interventions" />
            ) : null}
            <AnimatePresence>
              {interventions.length > 0 ? (
                <HumanInterventionMotionItem key="work-needs-you" exitGap="var(--spacing-4)">
                  <AttentionGroup
                    interventions={interventions}
                    onResolved={() => void actionResult.refetch()}
                  />
                </HumanInterventionMotionItem>
              ) : null}
            </AnimatePresence>
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
      </VStack>
    </div>
  );
}

function AttentionGroup({
  interventions,
  onResolved
}: {
  interventions: HumanInterventions;
  onResolved: () => void;
}) {
  const taskInterventions = interventions.filter(
    (intervention): intervention is TaskIntervention => intervention.__typename === "TaskAttention"
  );
  const otherInterventions = interventions.filter(
    (intervention) => intervention.__typename !== "TaskAttention"
  );
  return (
    <VStack as="section" aria-labelledby="work-needs-you" gap={1.5} className={stylex.props(styles.taskGroup).className}>
      <SectionHeader id="work-needs-you" title="Needs you" count={interventions.length} attention />
      <AnimatePresence initial={false}>
        {taskInterventions.length ? (
          <HumanInterventionMotionItem key="task-work-interventions" exitGap="var(--spacing-1-5)">
            <VStack as="div" role="list" gap={1.5} className={stylex.props(styles.cards).className}>
              <AnimatePresence initial={false}>
                {taskInterventions.map((intervention) => (
                  <HumanInterventionMotionItem
                    key={`${intervention.task.taskId}:${intervention.gate?.gateId ?? intervention.kind}`}
                    exitGap="var(--spacing-1-5)"
                    role="listitem"
                  >
                    <AttachedTaskIntervention
                      intervention={intervention}
                      onResolved={onResolved}
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
          <HumanInterventionMotionItem key="other-work-interventions" exitGap="var(--spacing-1-5)">
            <HumanInterventionList
              placement="queue"
              interventions={otherInterventions}
              onResolved={onResolved}
              initialAnimation={false}
            />
          </HumanInterventionMotionItem>
        ) : null}
      </AnimatePresence>
    </VStack>
  );
}

type HumanInterventions = PendingHumanInterventionsQuery["pendingHumanInterventions"];
type TaskIntervention = Extract<HumanInterventions[number], { __typename: "TaskAttention" }>;

function AttachedTaskIntervention({
  intervention,
  onResolved
}: {
  intervention: TaskIntervention;
  onResolved: () => void;
}) {
  const task = intervention.task;
  return (
    <VStack
      as="div"
      gap={0}
      className={stylex.props(styles.attachedTask).className}
    >
      <HumanInterventionList
        placement="task"
        interventions={[intervention]}
        onResolved={onResolved}
        animateItems={false}
        initialAnimation={false}
      />
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

function TaskGroup({ title, tasks }: { title: string; tasks: readonly WorkTask[] }) {
  const id = `work-group-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <VStack as="section" aria-labelledby={id} gap={1.5} className={stylex.props(styles.taskGroup).className}>
      <SectionHeader id={id} title={title} count={tasks.length} />
      <VStack as="div" role="list" gap={1.5} className={stylex.props(styles.cards).className}>
        {tasks.map((task) => (
          <TaskCard key={task.taskId} taskId={task.taskId} title={task.title} note={task.descriptionPreview} project={task.project?.name} status={taskStatusFromProjection(task)} statusLabel={taskRunLabel(task) ?? task.stage.name} timestamp={task.completedAt ?? task.updatedAt} />
        ))}
      </VStack>
    </VStack>
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

function TaskCard({ taskId, title, note, project, status, statusLabel, timestamp, listItem = true, attached = false }: {
  taskId: string;
  title: string;
  note?: string | null;
  project?: string | null;
  status: TaskStatus;
  statusLabel: string;
  timestamp: string;
  listItem?: boolean;
  attached?: boolean;
}) {
  const cardStyles = stylex.props(styles.taskCard, attached && styles.attachedTaskCard);
  const selectedCardStyles = stylex.props(styles.taskCard, attached && styles.attachedTaskCard, styles.selectedCard);
  return (
    <Link
      role={listItem ? "listitem" : undefined}
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
      <HStack gap={1} align="center" className={stylex.props(styles.cardMeta).className}>
        <TaskStatusBadge status={status} label={statusLabel} />
        {project ? (
          <>
            <span aria-hidden="true">·</span>
            <span {...stylex.props(styles.cardProject)}>{project}</span>
          </>
        ) : null}
      </HStack>
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
    <VStack as="section" aria-labelledby="work-history" gap={1.5} className={stylex.props(styles.taskGroup).className}>
      <SectionHeader id="work-history" title="History" count={tasks.length} />
      {!connection ? <ListMessage loading={result.loading} error={Boolean(result.error)} retry={() => result.refetch()} label="history" /> : null}
      {connection && !tasks.length ? <ListEmpty title="No matching history" detail="Done and cancelled tasks remain available here." /> : null}
      <VStack as="div" role="list" gap={1.5} className={stylex.props(styles.cards).className}>
        {tasks.map((task) => <TaskCard key={task.taskId} taskId={task.taskId} title={task.title} note={task.descriptionPreview} project={task.project?.name} status={taskStatusFromProjection(task)} statusLabel={task.stage.name} timestamp={task.completedAt ?? task.updatedAt} />)}
      </VStack>
      <ListLoadMore visible={Boolean(connection?.pageInfo.hasNextPage)} loading={result.loading} onLoad={() => result.fetchMore({ variables: { after: connection?.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, taskHistory: { ...fetchMoreResult.taskHistory, edges: [...previous.taskHistory.edges, ...fetchMoreResult.taskHistory.edges] } }) })} />
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
  taskList: {
    minWidth: 0,
    "--work-task-card-radius": "10px",
    "--work-task-card-shadow": "0 1px 2px color-mix(in srgb, black 4%, transparent)"
  },
  taskGroup: { minWidth: 0 },
  cards: { minWidth: 0 },
  sectionHeader: { minHeight: 24, minWidth: 0, paddingInline: "var(--spacing-1)" },
  sectionTitle: { margin: "var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650 },
  attentionTitle: { color: "var(--noema-clay-700)" },
  count: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 9 },
  taskCard: { display: "grid", minWidth: 0, gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: "var(--work-task-card-radius)", backgroundColor: "var(--noema-surface-card)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-3)", color: "var(--noema-text-secondary)", textDecoration: "none", boxShadow: "var(--work-task-card-shadow)", ":hover": { borderColor: "var(--noema-border-default)", backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  selectedCard: { borderColor: "color-mix(in srgb, var(--noema-pine-500) 26%, var(--noema-border-subtle))", backgroundColor: "color-mix(in srgb, var(--noema-pine-50) 70%, var(--noema-surface-card))", boxShadow: "0 2px 8px color-mix(in srgb, var(--noema-pine-700) 9%, transparent)" },
  cardHeading: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", minWidth: 0, alignItems: "baseline", gap: "var(--spacing-2)" },
  cardTitle: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 650, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  cardTime: { flexShrink: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 9 },
  cardPreview: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.35, textOverflow: "ellipsis", whiteSpace: "nowrap" },
  cardMeta: { minWidth: 0, overflow: "hidden", color: "var(--noema-text-muted)", fontSize: 10 },
  cardProject: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  attachedTask: {
    position: "relative",
    minWidth: 0,
    "--human-intervention-card-radius": "var(--work-task-card-radius)",
    "--human-intervention-card-bottom-radius": "0px",
    "--human-intervention-card-overlap": "var(--work-task-card-radius)"
  },
  attachedTaskCard: {
    position: "relative",
    zIndex: 1,
    ":hover": { backgroundColor: "var(--noema-surface-sunken)" }
  },
  state: { display: "flex", minHeight: 64, alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, padding: "var(--spacing-2)", color: "var(--noema-text-muted)", fontSize: 11 },
  retry: { borderWidth: 0, backgroundColor: "transparent", padding: "var(--spacing-0)", color: "var(--noema-pine-700)", font: "inherit", fontWeight: 650, textDecoration: "underline", cursor: "pointer" },
  empty: { display: "grid", minHeight: 72, alignContent: "center", justifyItems: "start", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 10, padding: "var(--spacing-3)", color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.4 },
  loadMore: { paddingBlock: "var(--spacing-1)" }
});
