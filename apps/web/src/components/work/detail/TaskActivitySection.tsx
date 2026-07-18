import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import type { WorkTaskDetail } from "../workTypes";
import { eventLabel, timestampLabel } from "../workModel";
import { WorkTaskActivityPageDocument } from "@/generated/graphql";
import { WorkHistoryLoadMore } from "./WorkHistoryLoadMore";

export function TaskActivitySection({ task }: { task: WorkTaskDetail }) {
  type Event = WorkTaskDetail["activity"]["edges"][number]["node"];
  const [older, setOlder] = React.useState<Event[]>([]);
  const [pageInfoOverride, setPageInfo] = React.useState<typeof task.activity.pageInfo | null>(null);
  const pageInfo = pageInfoOverride ?? task.activity.pageInfo;
  const [load, { loading, error }] = useLazyQuery(WorkTaskActivityPageDocument, { fetchPolicy: "network-only" });
  const initial = task.activity.edges.map((edge) => edge.node);
  const events = [...new Map([...initial, ...older].map((item) => [item.eventId, item])).values()];
  const loadMore = async () => {
    const result = await load({ variables: { taskId: task.taskId, after: pageInfo.endCursor, first: 20 } });
    const next = result.data?.task?.activity;
    if (!next) return;
    setOlder((current) => [...new Map([...current, ...initial, ...next.edges.map((edge) => edge.node)].map((item) => [item.eventId, item])).values()]);
    setPageInfo(next.pageInfo);
  };
  return (
    <section aria-labelledby="task-activity-title" {...stylex.props(styles.section)}>
      <h2 id="task-activity-title" {...stylex.props(styles.title)}>Activity</h2>
      {events.length > 0 ? (
        <ol {...stylex.props(styles.list)}>
          {events.map((event) => <li key={event.eventId} {...stylex.props(styles.event)}><strong {...stylex.props(styles.eventTitle)}>{eventLabel(event)}</strong><span {...stylex.props(styles.eventMeta)}>{event.actor} · {timestampLabel(event.occurredAt)}</span></li>)}
        </ol>
      ) : <p {...stylex.props(styles.empty)}>No activity has been recorded.</p>}
      {pageInfo.hasNextPage ? <WorkHistoryLoadMore label="Load older activity" loading={loading} error={Boolean(error)} onClick={() => { void loadMore().catch(() => undefined); }} /> : null}
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 20 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 },
  list: { display: "grid", gap: 0, margin: 0, padding: 0, listStyle: "none" },
  event: { display: "grid", gap: 2, borderLeftWidth: 2, borderLeftStyle: "solid", borderLeftColor: "var(--pine-100)", paddingBlock: 7, paddingLeft: 12, fontSize: 12 },
  eventTitle: { color: "var(--foreground)" }, eventMeta: { color: "var(--muted-foreground)", fontSize: 11 },
  empty: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }
});
