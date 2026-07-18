import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { WorkNeedsYouDocument } from "@/generated/graphql";
import type { WorkProject } from "./workTypes";
import { AttentionGroup } from "./AttentionGroup";
import { WorkRefreshError } from "./WorkRefreshError";

const groups = [
  { kind: "CLARIFICATION_REQUIRED", title: "Clarification" },
  { kind: "APPROVAL_REQUIRED", title: "Approval" },
  { kind: "RECOVERY_REQUIRED", title: "Recovery" },
  { kind: "REVIEW_READY", title: "Ready for acceptance" }
] as const;

export function WorkNeedsYou({ projectId, projects }: { projectId?: string; projects: readonly WorkProject[] }) {
  const result = useQuery(WorkNeedsYouDocument, {
    variables: { workspaceId: "workspace:personal", projectId, first: 50 },
    fetchPolicy: "cache-and-network"
  });
  const connection = result.data?.needsYou;
  const attention = connection?.edges.map((edge) => edge.node) ?? [];

  if (result.loading && !connection) return <div role="status" {...stylex.props(styles.state)}>Loading items that need you…</div>;
  if (result.error && !connection) return <div role="alert" {...stylex.props(styles.state)}><p {...stylex.props(styles.stateText)}>Needs You could not be loaded.</p><Button size="sm" variant="secondary" label="Retry" onClick={() => void result.refetch()} /></div>;
  if (attention.length === 0) return <div {...stylex.props(styles.emptyFrame)}>{result.error ? <WorkRefreshError message="Needs You could not refresh. Showing cached data." onRetry={() => void result.refetch()} /> : null}<div {...stylex.props(styles.state)}><h2 id="needs-you-empty-title" tabIndex={-1} {...stylex.props(styles.stateTitle)}>Nothing needs you</h2><p {...stylex.props(styles.stateText)}>Noema will collect questions, approvals, recovery choices, and reviewed results here.</p></div></div>;

  return (
    <section aria-labelledby="needs-you-title" {...stylex.props(styles.root)}>
      <h2 id="needs-you-title" tabIndex={-1} {...stylex.props(styles.pageTitle)}>Needs You</h2>
      {result.error ? <WorkRefreshError message="Needs You could not refresh. Showing the last loaded items." onRetry={() => void result.refetch()} /> : null}
      {groups.map((group) => {
        const items = attention.filter((item) => item.kind === group.kind);
        if (items.length === 0) return null;
        return <AttentionGroup key={group.kind} title={group.title} items={items} projects={projects} onUpdated={async () => { await result.refetch(); }} />;
      })}
      {connection?.pageInfo.hasNextPage ? <Button size="sm" variant="ghost" label="Load more attention items" onClick={() => void result.fetchMore({ variables: { after: connection.pageInfo.endCursor }, updateQuery: (previous, { fetchMoreResult }) => ({ ...fetchMoreResult, needsYou: { ...fetchMoreResult.needsYou, edges: [...previous.needsYou.edges, ...fetchMoreResult.needsYou.edges] } }) })} /> : null}
    </section>
  );
}

const styles = stylex.create({
  root: { display: "grid", alignContent: "start", gap: 18, minHeight: 0, overflowY: "auto" },
  emptyFrame: { display: "grid", alignContent: "start", gap: 10, minHeight: 0, overflowY: "auto" },
  pageTitle: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 },
  state: { display: "grid", alignContent: "center", justifyItems: "start", gap: 8, minHeight: 260, maxWidth: 560, color: "var(--muted-foreground)" }, stateTitle: { margin: 0, color: "var(--foreground)", fontFamily: "var(--font-heading)", fontSize: 22 }, stateText: { margin: 0 }
});
