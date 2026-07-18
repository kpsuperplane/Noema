import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { WorkTasksDocument } from "@/generated/graphql";
import { normalizeWorkSearch, type WorkProject } from "./workTypes";
import { AttentionBadge } from "./AttentionBadge";
import { StageBadge } from "./StageBadge";
import { TaskActions } from "./TaskActions";
import { relativeTime, taskRunLabel } from "./workModel";
import { WorkTaskCard } from "./WorkTaskCard";
import { WorkRefreshError } from "./WorkRefreshError";

export function WorkList({
  projectId,
  query,
  projects,
  onClearFilters
}: {
  projectId?: string;
  query?: string;
  projects: readonly WorkProject[];
  onClearFilters: () => void;
}) {
  const [behavior, setBehavior] = React.useState<"" | "INTAKE" | "DISPATCH" | "ACTIVE" | "HUMAN_GATE" | "ACCEPTANCE">("");
  const [attentionOnly, setAttentionOnly] = React.useState(false);
  const result = useQuery(WorkTasksDocument, {
    variables: {
      input: {
        workspaceId: "workspace:personal",
        projectId,
        text: query,
        stageBehaviors: behavior ? [behavior] : undefined,
        attentionOnly,
        scope: "ACTIVE"
      },
      first: 50
    },
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true
  });
  const connection = result.data?.workTasks;
  const tasks = connection?.edges.map((edge) => edge.node) ?? [];
  const filtered = Boolean(projectId || query || behavior || attentionOnly);

  return (
    <section aria-labelledby="work-list-title" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}>
        <h2 id="work-list-title" tabIndex={-1} {...stylex.props(styles.title)}>Active tasks</h2>
        <div aria-label="List filters" {...stylex.props(styles.filters)}>
          <label {...stylex.props(styles.filterLabel)}>Stage
            <select value={behavior} {...stylex.props(styles.select)} onChange={(event) => setBehavior(event.currentTarget.value as typeof behavior)}>
              <option value="">All active stages</option>
              <option value="INTAKE">Inbox</option>
              <option value="DISPATCH">Queue</option>
              <option value="ACTIVE">Doing</option>
              <option value="HUMAN_GATE">Waiting</option>
              <option value="ACCEPTANCE">Review</option>
            </select>
          </label>
          <label {...stylex.props(styles.check)}><input type="checkbox" checked={attentionOnly} onChange={(event) => setAttentionOnly(event.currentTarget.checked)} /> Needs attention</label>
        </div>
      </div>
      {result.loading && !connection ? <div role="status" aria-label="Loading tasks" {...stylex.props(styles.skeleton)}>{[0, 1, 2, 3, 4].map((row) => <span key={row} {...stylex.props(styles.skeletonRow)} />)}</div> : null}
      {result.error && !connection ? <div role="alert" {...stylex.props(styles.state)}><p {...stylex.props(styles.stateText)}>Tasks could not be loaded.</p><Button size="sm" variant="secondary" label="Retry" onClick={() => void result.refetch()} /></div> : null}
      {result.error && connection ? <WorkRefreshError message="Tasks could not refresh. Showing the last loaded results." onRetry={() => void result.refetch()} /> : null}
      {connection && tasks.length === 0 ? <div {...stylex.props(styles.state)}><p {...stylex.props(styles.stateText)}>{filtered ? "No tasks match these filters." : "No active tasks yet."}</p>{filtered ? <Button size="sm" variant="secondary" label="Clear filters" onClick={() => { setBehavior(""); setAttentionOnly(false); onClearFilters(); }} /> : null}</div> : null}
      {tasks.length > 0 ? (
        <>
          <div {...stylex.props(styles.tableWrap)}>
            <table {...stylex.props(styles.table)}>
              <thead><tr><th {...stylex.props(styles.th)}>Task</th><th {...stylex.props(styles.th)}>Project</th><th {...stylex.props(styles.th)}>Stage</th><th {...stylex.props(styles.th)}>Current work</th><th {...stylex.props(styles.th)}>Updated</th><th {...stylex.props(styles.th)}><span {...stylex.props(styles.srOnly)}>Actions</span></th></tr></thead>
              <tbody>{tasks.map((task, index) => (
                <tr key={task.taskId}>
                  <td {...stylex.props(styles.td, styles.taskCell, index === tasks.length - 1 && styles.lastCell)}><Link to="/work/tasks/$taskId" params={{ taskId: task.taskId }} search={(current) => normalizeWorkSearch(current)} {...stylex.props(styles.taskLink)}>{task.title}</Link>{task.attention ? <AttentionBadge kind={task.attention.kind} label={task.attention.title} /> : null}</td>
                  <td {...stylex.props(styles.td, index === tasks.length - 1 && styles.lastCell)}>{task.project?.name ?? <span {...stylex.props(styles.muted)}>None</span>}</td>
                  <td {...stylex.props(styles.td, index === tasks.length - 1 && styles.lastCell)}><StageBadge name={task.stage.name} behavior={task.stage.behavior} /></td>
                  <td {...stylex.props(styles.td, index === tasks.length - 1 && styles.lastCell)}>{taskRunLabel(task) ?? <span {...stylex.props(styles.muted)}>Idle</span>}</td>
                  <td {...stylex.props(styles.td, index === tasks.length - 1 && styles.lastCell)}>{relativeTime(task.updatedAt)}</td>
                  <td {...stylex.props(styles.td, index === tasks.length - 1 && styles.lastCell)}><TaskActions compact task={task} validActions={task.validActions} projects={projects} onUpdated={async () => { await result.refetch(); }} /></td>
                </tr>
              ))}</tbody>
            </table>
          </div>
          <div {...stylex.props(styles.mobileList)}>{tasks.map((task) => <WorkTaskCard key={task.taskId} task={task} projects={projects} onUpdated={async () => { await result.refetch(); }} />)}</div>
        </>
      ) : null}
      {connection?.pageInfo.hasNextPage ? <Button size="sm" variant="ghost" label="Load more tasks" isLoading={result.loading} onClick={() => void result.fetchMore({ variables: { after: connection.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, workTasks: { ...fetchMoreResult.workTasks, edges: [...previous.workTasks.edges, ...fetchMoreResult.workTasks.edges] } }) })} /> : null}
    </section>
  );
}

const styles = stylex.create({
  root: { display: "grid", alignContent: "start", gap: 14, minHeight: 0, overflowY: "auto" },
  header: { display: "flex", flexWrap: "wrap", alignItems: "end", justifyContent: "space-between", gap: 12 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18, color: "var(--foreground)" },
  filters: { display: "flex", flexWrap: "wrap", alignItems: "end", gap: 10 },
  filterLabel: { display: "grid", gap: 4, fontSize: 11, fontWeight: 650, color: "var(--muted-foreground)" },
  select: { minHeight: 34, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingInline: 9, color: "var(--foreground)", font: "inherit", fontSize: 12 },
  check: { display: "flex", minHeight: 34, alignItems: "center", gap: 7, fontSize: 12, color: "var(--foreground)" },
  tableWrap: { overflowX: "auto", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 12, "@media (max-width: 760px)": { display: "none" } },
  table: { width: "100%", borderCollapse: "collapse", fontSize: 12 }, th: { padding: 10, textAlign: "left", fontWeight: 650, color: "var(--muted-foreground)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)" }, td: { minWidth: 100, padding: 10, verticalAlign: "middle", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", color: "var(--text-secondary)" }, taskCell: { display: "grid", minWidth: 220, gap: 6 }, lastCell: { borderBottomWidth: 0 },
  taskLink: { color: "var(--foreground)", fontFamily: "var(--font-heading)", fontSize: 13, fontWeight: 700, textDecoration: "none", ":hover": { textDecoration: "underline" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  mobileList: { display: "none", gap: 10, "@media (max-width: 760px)": { display: "grid" } },
  muted: { color: "var(--muted-foreground)" },
  state: { display: "grid", justifyItems: "start", gap: 8, minHeight: 180, alignContent: "center", color: "var(--muted-foreground)" }, stateText: { margin: 0 },
  skeleton: { display: "grid", gap: 1, borderRadius: 12, overflow: "hidden" }, skeletonRow: { height: 58, backgroundColor: "var(--paper-100)" },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)" }
});
