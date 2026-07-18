import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { WorkOverviewDocument } from "@/generated/graphql";
import type { WorkProject } from "./workTypes";
import { WorkBoardColumn } from "./WorkBoardColumn";
import { WorkRefreshError } from "./WorkRefreshError";

export function WorkBoard({ projectId, projects, onNewTask }: { projectId?: string; projects: readonly WorkProject[]; onNewTask: () => void }) {
  const result = useQuery(WorkOverviewDocument, {
    variables: { workspaceId: "workspace:personal", projectId },
    fetchPolicy: "cache-and-network"
  });
  const overview = result.data?.workOverview;

  if (result.loading && !overview) {
    return <div role="status" aria-label="Loading board" {...stylex.props(styles.loading)}>{[0, 1, 2, 3, 4].map((value) => <span key={value} {...stylex.props(styles.loadingColumn)} />)}</div>;
  }
  if (result.error && !overview) {
    return <div role="alert" {...stylex.props(styles.empty)}><h2 {...stylex.props(styles.emptyTitle)}>Board unavailable</h2><p {...stylex.props(styles.emptyText)}>Noema could not load Work right now.</p><Button size="sm" variant="secondary" label="Retry" onClick={() => void result.refetch()} /></div>;
  }
  if (!overview || overview.activeColumns.every((column) => column.taskCount === 0)) {
    return <div {...stylex.props(styles.frame)}>{result.error ? <WorkRefreshError message="The board count could not refresh. Showing cached data." onRetry={() => void result.refetch()} /> : null}<div {...stylex.props(styles.empty)}><h2 {...stylex.props(styles.emptyTitle)}>Your board is clear</h2><p {...stylex.props(styles.emptyText)}>Captured and delegated tasks appear here in Inbox.</p><Button size="sm" variant="primary" label="New task" onClick={onNewTask} /></div></div>;
  }

  return (
    <div {...stylex.props(styles.frame)}>
      {result.error ? <WorkRefreshError message="The board count could not refresh. Showing cached data." onRetry={() => void result.refetch()} /> : null}
      <div role="region" aria-label="Work board" {...stylex.props(styles.board)}>
        {overview.activeColumns.map((column) => <WorkBoardColumn key={column.stage.stageId} column={column} projectId={projectId} projects={projects} />)}
      </div>
    </div>
  );
}

const styles = stylex.create({
  frame: { display: "grid", gridTemplateRows: "auto minmax(0, 1fr)", gap: 8, minHeight: 0, height: "100%" },
  board: { display: "grid", gridAutoFlow: "column", gridAutoColumns: "minmax(268px, 1fr)", gap: 12, minHeight: 0, height: "100%", overflowX: "auto", overscrollBehaviorInline: "contain", paddingBottom: 4, scrollSnapType: "x proximity" },
  loading: { display: "grid", gridTemplateColumns: "repeat(5, minmax(268px, 1fr))", gap: 12, minHeight: 0, height: "100%", overflow: "hidden" },
  loadingColumn: { borderRadius: 14, backgroundColor: "var(--paper-100)" },
  empty: { display: "grid", alignContent: "center", justifyItems: "start", gap: 8, minHeight: 280, maxWidth: 480 }, emptyTitle: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 22 }, emptyText: { margin: 0, color: "var(--muted-foreground)", fontSize: 14 }
});
