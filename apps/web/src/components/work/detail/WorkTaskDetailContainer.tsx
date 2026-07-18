import * as React from "react";
import { useQuery, useSubscription } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ArrowLeft } from "lucide-react";
import { WorkTaskDetailDocument, WorkTaskEventsDocument } from "@/generated/graphql";
import { useTaskEventCursor } from "@/components/chatDetail/task/taskEventCursor";
import { WorkTaskDetailView } from "./WorkTaskDetailView";
import { WorkTaskDetailSkeleton } from "./WorkTaskDetailSkeleton";
import { WorkTaskUnavailable } from "./WorkTaskUnavailable";
import { useAllWorkProjects } from "../useAllWorkProjects";
import { WorkRefreshError } from "../WorkRefreshError";

export function WorkTaskDetailContainer({ taskId, compact = false, onBack, onTitleChange }: { taskId: string; compact?: boolean; onBack?: () => void; onTitleChange?: (title: string | null) => void }) {
  const result = useQuery(WorkTaskDetailDocument, { variables: { taskId }, fetchPolicy: "cache-and-network", notifyOnNetworkStatusChange: true });
  const projectsResult = useAllWorkProjects();
  const task = result.data?.task ?? null;
  const terminal = task?.stage.behavior === "TERMINAL_SUCCESS" || task?.stage.behavior === "TERMINAL_CANCELLED";
  const [cursor, recordCursor] = useTaskEventCursor(taskId);
  useSubscription(WorkTaskEventsDocument, {
    variables: { taskId, after: cursor },
    skip: terminal,
    onData: ({ data }) => {
      const event = data.data?.taskEvents;
      if (!event) return;
      recordCursor(event.cursor);
      void result.refetch();
    }
  });

  React.useEffect(() => onTitleChange?.(task?.title ?? null), [onTitleChange, task?.title]);

  if (result.loading && !task) return <WorkTaskDetailSkeleton compact={compact} />;
  if (result.error && !task) return <WorkTaskUnavailable compact={compact} onBack={onBack} />;
  if (!task) return <WorkTaskUnavailable compact={compact} onBack={onBack} />;

  return (
    <section data-compact={compact} aria-labelledby="task-detail-title" {...stylex.props(styles.root, compact && styles.compact)}>
      {onBack ? <div {...stylex.props(styles.back)}><Button type="button" size="sm" variant="ghost" label="Back to Work" icon={<ArrowLeft aria-hidden="true" size={15} />} onClick={onBack} /></div> : null}
      {result.error ? <div role="status" {...stylex.props(styles.refreshError)}>Some task history could not refresh. <button type="button" {...stylex.props(styles.retry)} onClick={() => void result.refetch()}>Retry</button></div> : null}
      {projectsResult.error ? <WorkRefreshError message="Some project information could not refresh." onRetry={() => void projectsResult.retry()} /> : null}
      <WorkTaskDetailView key={task.taskId} task={task} compact={compact} projects={projectsResult.projects} onUpdated={async () => { await result.refetch(); }} />
    </section>
  );
}

const styles = stylex.create({
  root: { height: "100%", minHeight: 0, overflowY: "auto", overscrollBehavior: "contain", backgroundColor: "var(--background)", paddingBlock: 18, paddingInline: 22, "@media (max-width: 760px)": { paddingInline: 14, paddingBottom: "max(18px, env(safe-area-inset-bottom))" } }, compact: { paddingBlock: 8, paddingInline: 8 }, back: { marginBottom: 10 }, refreshError: { display: "flex", alignItems: "center", gap: 6, marginBottom: 10, borderRadius: 8, backgroundColor: "var(--clay-50)", padding: 8, color: "var(--clay-600)", fontSize: 12 }, retry: { borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "inherit", font: "inherit", fontWeight: 700, textDecoration: "underline" }
});
