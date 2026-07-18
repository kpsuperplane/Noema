import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import { Markdown } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { WorkTaskDetail } from "../workTypes";
import { sentenceCase, timestampLabel } from "../workModel";
import { WorkTaskContractsPageDocument } from "@/generated/graphql";
import { WorkHistoryLoadMore } from "./WorkHistoryLoadMore";

export function TaskContractHistory({ task }: { task: WorkTaskDetail }) {
  type Contract = WorkTaskDetail["contracts"]["edges"][number]["node"];
  const [older, setOlder] = React.useState<Contract[]>([]);
  const [pageInfoOverride, setPageInfo] = React.useState<typeof task.contracts.pageInfo | null>(null);
  const pageInfo = pageInfoOverride ?? task.contracts.pageInfo;
  const [load, { loading, error }] = useLazyQuery(WorkTaskContractsPageDocument, { fetchPolicy: "network-only" });
  const initial = task.contracts.edges.map((edge) => edge.node);
  const contracts = [...new Map([...initial, ...older].map((item) => [item.contractId, item])).values()];
  if (contracts.length === 0) return null;
  const loadMore = async () => {
    const result = await load({ variables: { taskId: task.taskId, after: pageInfo.endCursor, first: 20 } });
    const next = result.data?.task?.contracts;
    if (!next) return;
    setOlder((current) => [...new Map([...current, ...initial, ...next.edges.map((edge) => edge.node)].map((item) => [item.contractId, item])).values()]);
    setPageInfo(next.pageInfo);
  };
  return (
    <section aria-labelledby="task-contract-history-title" {...stylex.props(styles.section)}>
      <h2 id="task-contract-history-title" {...stylex.props(styles.title)}>Contract revisions</h2>
      <ol {...stylex.props(styles.list)}>{contracts.map((contract) => <li key={contract.contractId} {...stylex.props(styles.item)}><details open={contract.contractId === task.currentContract?.contractId}><summary {...stylex.props(styles.summary)}><strong {...stylex.props(styles.version)}>Version {contract.version}</strong><span {...stylex.props(styles.meta)}>{sentenceCase(contract.origin)} · {timestampLabel(contract.createdAt)}</span></summary><div {...stylex.props(styles.body)}><Markdown density="compact" headingLevelStart={3}>{contract.requestMarkdown}</Markdown>{contract.supersedesContractId ? <small {...stylex.props(styles.meta)}>Supersedes {contract.supersedesContractId}</small> : <small {...stylex.props(styles.meta)}>Initial contract</small>}</div></details></li>)}</ol>
      {pageInfo.hasNextPage ? <WorkHistoryLoadMore label="Load older contracts" loading={loading} error={Boolean(error)} onClick={() => { void loadMore().catch(() => undefined); }} /> : null}
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 20 }, title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 }, list: { display: "grid", gap: 7, margin: 0, padding: 0, listStyle: "none" }, item: { borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 10, padding: 10 }, summary: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", gap: 8 }, version: { fontSize: 12 }, meta: { color: "var(--muted-foreground)", fontSize: 11 }, body: { display: "grid", gap: 7, paddingTop: 10, color: "var(--text-secondary)", fontSize: 12 }
});
