import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { WorkCompletedTasksDocument } from "@/generated/graphql";
import type { WorkProject } from "./workTypes";
import { WorkTaskCard } from "./WorkTaskCard";
import { WorkRefreshError } from "./WorkRefreshError";

export function WorkCompleted({ projectId, query, terminal, projects, onTerminalChange }: { projectId?: string; query?: string; terminal: "all" | "accepted" | "cancelled"; projects: readonly WorkProject[]; onTerminalChange: (value: "all" | "accepted" | "cancelled") => void }) {
  const kind: "ALL" | "ACCEPTED" | "CANCELLED" = terminal === "accepted" ? "ACCEPTED" : terminal === "cancelled" ? "CANCELLED" : "ALL";
  const result = useQuery(WorkCompletedTasksDocument, { variables: { workspaceId: "workspace:personal", projectId, text: query, kind, first: 50 }, fetchPolicy: "cache-and-network" });
  const connection = result.data?.completedTasks;
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  return (
    <section aria-labelledby="work-completed-title" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}><h2 id="work-completed-title" tabIndex={-1} {...stylex.props(styles.title)}>Completed</h2><label {...stylex.props(styles.filter)}>Show <select value={terminal} {...stylex.props(styles.select)} onChange={(event) => onTerminalChange(event.currentTarget.value as "all" | "accepted" | "cancelled")}><option value="all">Completed and cancelled</option><option value="accepted">Completed</option><option value="cancelled">Cancelled</option></select></label></div>
      {result.loading && !connection ? <div role="status" {...stylex.props(styles.state)}>Loading completed work…</div> : null}
      {result.error && !connection ? <div role="alert" {...stylex.props(styles.state)}><p {...stylex.props(styles.stateText)}>Completed work could not be loaded.</p><Button size="sm" variant="secondary" label="Retry" onClick={() => void result.refetch()} /></div> : null}
      {result.error && connection ? <WorkRefreshError message="Completed work could not refresh. Showing the last loaded history." onRetry={() => void result.refetch()} /> : null}
      {connection && tasks.length === 0 ? <div {...stylex.props(styles.state)}><h3 {...stylex.props(styles.stateTitle)}>No matching history</h3><p {...stylex.props(styles.stateText)}>Accepted and cancelled tasks stay available here.</p></div> : null}
      <div {...stylex.props(styles.grid)}>{tasks.map((task) => <WorkTaskCard key={task.taskId} task={task} projects={projects} onUpdated={async () => { await result.refetch(); }} />)}</div>
      {connection?.pageInfo.hasNextPage ? <Button size="sm" variant="ghost" label="Load more history" onClick={() => void result.fetchMore({ variables: { after: connection.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, completedTasks: { ...fetchMoreResult.completedTasks, edges: [...previous.completedTasks.edges, ...fetchMoreResult.completedTasks.edges] } }) })} /> : null}
    </section>
  );
}

const styles = stylex.create({
  root: { display: "grid", alignContent: "start", gap: 14, minHeight: 0, overflowY: "auto" },
  header: { display: "flex", flexWrap: "wrap", alignItems: "end", justifyContent: "space-between", gap: 12 }, title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 }, filter: { display: "grid", gap: 4, fontSize: 11, fontWeight: 650, color: "var(--muted-foreground)" }, select: { minHeight: 34, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingInline: 9, color: "var(--foreground)", font: "inherit", fontSize: 12 },
  grid: { display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(min(300px, 100%), 1fr))", gap: 10 },
  state: { display: "grid", alignContent: "center", justifyItems: "start", gap: 8, minHeight: 220, color: "var(--muted-foreground)" }, stateTitle: { margin: 0, color: "var(--foreground)", fontFamily: "var(--font-heading)" }, stateText: { margin: 0 }
});
