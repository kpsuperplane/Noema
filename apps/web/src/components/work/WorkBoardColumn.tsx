import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { WorkTasksDocument } from "@/generated/graphql";
import type { WorkProject, WorkOverview } from "./workTypes";
import { WorkTaskCard } from "./WorkTaskCard";
import { WorkRefreshError } from "./WorkRefreshError";

type WorkColumn = WorkOverview["activeColumns"][number];

export function WorkBoardColumn({
  column,
  projectId,
  projects
}: {
  column: WorkColumn;
  projectId?: string;
  projects: readonly WorkProject[];
}) {
  const result = useQuery(WorkTasksDocument, {
    variables: {
      input: {
        workspaceId: "workspace:personal",
        projectId,
        stageIds: [column.stage.stageId],
        scope: "ACTIVE"
      },
      first: 20
    },
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true
  });
  const connection = result.data?.workTasks;
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  const titleId = `work-column-${column.stage.stageId.replaceAll(":", "-")}`;

  return (
    <section aria-labelledby={titleId} {...stylex.props(styles.column)}>
      <header {...stylex.props(styles.header)}>
        <h2 id={titleId} {...stylex.props(styles.title)}>{column.stage.name}</h2>
        <span aria-label={`${column.taskCount} tasks`} {...stylex.props(styles.count)}>{column.taskCount}</span>
      </header>
      <div {...stylex.props(styles.cards)}>
        {result.loading && !connection ? <div role="status" aria-label="Loading tasks" {...stylex.props(styles.skeleton)}><span {...stylex.props(styles.skeletonCard)} /><span {...stylex.props(styles.skeletonCard)} /><span {...stylex.props(styles.skeletonCard)} /></div> : null}
        {result.error && !connection ? (
          <div role="alert" {...stylex.props(styles.state)}>
            <span>This column could not be loaded.</span>
            <Button size="sm" variant="secondary" label="Retry" onClick={() => void result.refetch()} />
          </div>
        ) : null}
        {result.error && connection ? <WorkRefreshError message={`${column.stage.name} could not refresh. Showing cached tasks.`} onRetry={() => void result.refetch()} /> : null}
        {!result.loading && connection && tasks.length === 0 ? (
          <p {...stylex.props(styles.state)}>No tasks in {column.stage.name}.</p>
        ) : null}
        {tasks.map((task) => (
          <WorkTaskCard key={task.taskId} task={task} projects={projects} showStage={false} onUpdated={async () => { await result.refetch(); }} />
        ))}
      </div>
      {connection?.pageInfo.hasNextPage ? (
        <Button
          size="sm"
          variant="ghost"
          label={`Load more ${column.stage.name}`}
          isLoading={result.loading}
          onClick={() => {
            void result.fetchMore({
              variables: { after: connection.pageInfo.endCursor },
              updateQuery: (previous, { fetchMoreResult }) => ({
                ...fetchMoreResult,
                workTasks: {
                  ...fetchMoreResult.workTasks,
                  edges: [...previous.workTasks.edges, ...fetchMoreResult.workTasks.edges]
                }
              })
            });
          }}
        />
      ) : null}
    </section>
  );
}

const styles = stylex.create({
  column: { display: "grid", gridTemplateRows: "auto minmax(0, 1fr) auto", gap: 10, width: "100%", minWidth: 268, minHeight: 0, borderRadius: 14, backgroundColor: "color-mix(in srgb, var(--paper-100) 68%, white)", padding: 10, scrollSnapAlign: "start" },
  header: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, paddingInline: 4 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 14, fontWeight: 750, color: "var(--foreground)" },
  count: { display: "grid", minWidth: 24, height: 24, placeItems: "center", borderRadius: 999, backgroundColor: "var(--surface-card)", fontSize: 11, fontWeight: 700, color: "var(--muted-foreground)" },
  cards: { display: "flex", minHeight: 0, flexDirection: "column", gap: 9, overflowY: "auto", overscrollBehavior: "contain", scrollbarWidth: "thin" },
  state: { display: "grid", gap: 10, margin: 0, borderRadius: 10, padding: 14, color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.45 },
  skeleton: { display: "grid", gap: 9 }, skeletonCard: { height: 118, borderRadius: 12, backgroundColor: "var(--paper-200)", opacity: 0.7 }
});
