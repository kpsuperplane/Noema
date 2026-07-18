import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import * as stylex from "@stylexjs/stylex";
import { TaskRunTranscript } from "@/components/chatDetail/task/TaskRunTranscript";
import type { WorkTaskDetail } from "../workTypes";
import { sentenceCase, timestampLabel } from "../workModel";
import { WorkTaskRunsPageDocument } from "@/generated/graphql";
import { WorkHistoryLoadMore } from "./WorkHistoryLoadMore";

export function TaskRunsSection({ task }: { task: WorkTaskDetail }) {
  type Run = WorkTaskDetail["runs"]["edges"][number]["node"];
  const [older, setOlder] = React.useState<Run[]>([]);
  const [pageInfoOverride, setPageInfo] = React.useState<typeof task.runs.pageInfo | null>(null);
  const pageInfo = pageInfoOverride ?? task.runs.pageInfo;
  const [load, { loading, error }] = useLazyQuery(WorkTaskRunsPageDocument, { fetchPolicy: "network-only" });
  const initial = task.runs.edges.map((edge) => edge.node);
  const runs = [...new Map([...initial, ...older].map((item) => [item.runId, item])).values()];
  const [selectedRunId, setSelectedRunId] = React.useState<string | null>(runs[0]?.runId ?? null);
  const selected = runs.find((run) => run.runId === selectedRunId) ?? runs[0] ?? null;
  const loadMore = async () => {
    const result = await load({ variables: { taskId: task.taskId, after: pageInfo.endCursor, first: 20 } });
    const next = result.data?.task?.runs;
    if (!next) return;
    setOlder((current) => [...new Map([...current, ...initial, ...next.edges.map((edge) => edge.node)].map((item) => [item.runId, item])).values()]);
    setPageInfo(next.pageInfo);
  };
  if (runs.length === 0) return <section aria-labelledby="task-runs-title" {...stylex.props(styles.section)}><h2 id="task-runs-title" {...stylex.props(styles.title)}>Runs and transcripts</h2><p {...stylex.props(styles.empty)}>No runs have been queued.</p></section>;
  return (
    <section aria-labelledby="task-runs-title" {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.heading)}><h2 id="task-runs-title" {...stylex.props(styles.title)}>Runs and transcripts</h2><span {...stylex.props(styles.headingMeta)}>{runs.length} recorded</span></div>
      <div {...stylex.props(styles.layout)}>
        <div role="list" aria-label="Task runs" {...stylex.props(styles.list)}>{runs.map((run) => <button key={run.runId} type="button" role="listitem" data-selected={run.runId === selected?.runId} {...stylex.props(styles.runButton, run.runId === selected?.runId && styles.selectedRun)} onClick={() => setSelectedRunId(run.runId)}><strong {...stylex.props(styles.runTitle)}>{sentenceCase(run.kind)}</strong><span {...stylex.props(styles.runMeta)}>{sentenceCase(run.status)} · attempt {run.attemptIndex + 1}</span><small {...stylex.props(styles.runMeta)}>{timestampLabel(run.createdAt)}</small></button>)}</div>
        {selected ? <div {...stylex.props(styles.detail)}><dl {...stylex.props(styles.facts)}><div {...stylex.props(styles.fact)}><dt {...stylex.props(styles.term)}>Run</dt><dd {...stylex.props(styles.definition)}>{sentenceCase(selected.kind)} · {sentenceCase(selected.status)}</dd></div><div {...stylex.props(styles.fact)}><dt {...stylex.props(styles.term)}>Lineage</dt><dd {...stylex.props(styles.definition)}>{selected.parentRunId ? `Child of ${selected.parentRunId}` : "Root run"}</dd></div><div {...stylex.props(styles.fact)}><dt {...stylex.props(styles.term)}>Model</dt><dd {...stylex.props(styles.definition)}>{selected.actualProviderKind ?? selected.model.providerKind} · {selected.actualModelProfile ?? selected.model.modelProfile ?? "provider default"}</dd></div><div {...stylex.props(styles.fact)}><dt {...stylex.props(styles.term)}>Usage</dt><dd {...stylex.props(styles.definition)}>{selected.providerCallCount} model · {selected.toolCallCount} tools · {selected.inputTokens + selected.outputTokens} tokens</dd></div></dl>{selected.errorMessage ? <p role="status" {...stylex.props(styles.error)}>{selected.errorCode ? `${selected.errorCode}: ` : ""}{selected.errorMessage}</p> : null}<div {...stylex.props(styles.transcript)}><TaskRunTranscript key={selected.runId} run={selected} /></div></div> : null}
      </div>
      {pageInfo.hasNextPage ? <WorkHistoryLoadMore label="Load older runs" loading={loading} error={Boolean(error)} onClick={() => { void loadMore().catch(() => undefined); }} /> : null}
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 20 }, title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 }, heading: { display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 8 }, headingMeta: { color: "var(--muted-foreground)", fontSize: 11 }, layout: { display: "grid", gridTemplateColumns: "190px minmax(0, 1fr)", minHeight: 380, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 12, overflow: "hidden", "@media (max-width: 700px)": { gridTemplateColumns: "1fr" } }, list: { display: "flex", flexDirection: "column", gap: 3, borderRightWidth: 1, borderRightStyle: "solid", borderRightColor: "var(--border-subtle)", backgroundColor: "var(--paper-100)", padding: 8, overflowY: "auto", "@media (max-width: 700px)": { maxHeight: 180, borderRightWidth: 0, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)" } }, runButton: { display: "grid", justifyItems: "start", gap: 2, width: "100%", borderWidth: 0, borderRadius: 8, backgroundColor: "transparent", padding: 9, color: "var(--foreground)", font: "inherit", textAlign: "left" }, selectedRun: { backgroundColor: "var(--surface-card)", boxShadow: "0 1px 2px color-mix(in srgb, black 8%, transparent)" }, runTitle: { fontSize: 12 }, runMeta: { color: "var(--muted-foreground)", fontSize: 10 }, detail: { display: "grid", gridTemplateRows: "auto auto minmax(260px, 1fr)", gap: 10, minWidth: 0, minHeight: 0, padding: 10 }, facts: { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7, margin: 0 }, fact: { display: "grid", gap: 2 }, term: { color: "var(--muted-foreground)", fontSize: 10 }, definition: { margin: 0, overflowWrap: "anywhere", fontSize: 11 }, transcript: { minHeight: 280, overflow: "hidden", borderRadius: 9, backgroundColor: "var(--surface-card)" }, error: { margin: 0, borderRadius: 8, backgroundColor: "var(--red-100)", padding: 8, color: "var(--red-700)", fontSize: 11 }, empty: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }
});
