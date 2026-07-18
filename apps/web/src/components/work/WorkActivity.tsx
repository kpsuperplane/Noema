import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { WorkActivityDocument } from "@/generated/graphql";
import { eventLabel, timestampLabel } from "./workModel";
import { WorkRefreshError } from "./WorkRefreshError";
import { normalizeWorkSearch } from "./workTypes";

export function WorkActivity({ projectId }: { projectId?: string }) {
  const result = useQuery(WorkActivityDocument, { variables: { workspaceId: "workspace:personal", projectId, first: 50 }, fetchPolicy: "cache-and-network" });
  const connection = result.data?.workActivity;
  const events = connection?.edges.map((edge) => edge.node) ?? [];
  if (result.loading && !connection) return <div role="status" {...stylex.props(styles.state)}>Loading Work activity…</div>;
  if (result.error && !connection) return <div role="alert" {...stylex.props(styles.state)}><p {...stylex.props(styles.stateText)}>Activity could not be loaded.</p><Button size="sm" variant="secondary" label="Retry" onClick={() => void result.refetch()} /></div>;
  if (events.length === 0) return <div {...stylex.props(styles.emptyFrame)}>{result.error ? <WorkRefreshError message="Activity could not refresh. Showing cached data." onRetry={() => void result.refetch()} /> : null}<div {...stylex.props(styles.state)}><h2 {...stylex.props(styles.stateTitle)}>No activity yet</h2><p {...stylex.props(styles.stateText)}>Durable task and run events will appear here.</p></div></div>;
  return (
    <section aria-labelledby="work-activity-title" {...stylex.props(styles.root)}>
      <h2 id="work-activity-title" tabIndex={-1} {...stylex.props(styles.title)}>Activity</h2>
      {result.error ? <WorkRefreshError message="Activity could not refresh. Showing the last loaded events." onRetry={() => void result.refetch()} /> : null}
      <ol {...stylex.props(styles.timeline)}>{events.map((event) => <li key={event.eventId} {...stylex.props(styles.event)}><span aria-hidden="true" {...stylex.props(styles.dot)} /><div {...stylex.props(styles.eventCopy)}><strong {...stylex.props(styles.eventTitle)}>{eventLabel(event)}</strong><span {...stylex.props(styles.eventTime)}>{timestampLabel(event.occurredAt)}{event.runId ? ` · ${event.runId.split(":")[1] ?? "run"}` : ""}</span>{event.taskId ? <Link to="/work/tasks/$taskId" params={{ taskId: event.taskId }} search={(current) => normalizeWorkSearch(current)} {...stylex.props(styles.eventLink)}>Open task</Link> : null}</div></li>)}</ol>
      {connection?.pageInfo.hasNextPage ? <Button size="sm" variant="ghost" label="Load more activity" onClick={() => void result.fetchMore({ variables: { after: connection.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, workActivity: { ...fetchMoreResult.workActivity, edges: [...previous.workActivity.edges, ...fetchMoreResult.workActivity.edges] } }) })} /> : null}
    </section>
  );
}

const styles = stylex.create({
  root: { display: "grid", alignContent: "start", gap: 14, minHeight: 0, overflowY: "auto", maxWidth: 860 },
  emptyFrame: { display: "grid", alignContent: "start", gap: 10, minHeight: 0, maxWidth: 860, overflowY: "auto" },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 },
  timeline: { display: "grid", gap: 0, margin: 0, padding: 0, listStyle: "none" },
  event: { position: "relative", display: "grid", gridTemplateColumns: "18px minmax(0, 1fr)", gap: 10, minHeight: 64, paddingBottom: 14, ":not(:last-child)::before": { content: "''", position: "absolute", top: 14, bottom: 0, left: 6, width: 1, backgroundColor: "var(--border)" } },
  eventCopy: { display: "grid", justifyItems: "start", gap: 3 }, eventTitle: { fontSize: 13, color: "var(--foreground)" }, eventTime: { fontSize: 11, color: "var(--muted-foreground)" }, eventLink: { fontSize: 12, color: "var(--pine-700)" },
  dot: { position: "relative", zIndex: 1, width: 13, height: 13, marginTop: 3, borderWidth: 3, borderStyle: "solid", borderColor: "var(--pine-100)", borderRadius: 999, backgroundColor: "var(--pine-600)" },
  state: { display: "grid", alignContent: "center", justifyItems: "start", gap: 8, minHeight: 260, color: "var(--muted-foreground)" }, stateTitle: { margin: 0, color: "var(--foreground)", fontFamily: "var(--font-heading)" }, stateText: { margin: 0 }
});
