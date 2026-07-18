import { Markdown } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { WorkTaskDetail } from "../workTypes";
import { sentenceCase } from "../workModel";

type Contract = NonNullable<WorkTaskDetail["currentContract"]>;

export function TaskContractSection({ contract }: { contract: Contract | null }) {
  if (!contract) return <section aria-labelledby="task-contract-title" {...stylex.props(styles.section)}><h2 id="task-contract-title" {...stylex.props(styles.title)}>Current contract</h2><p {...stylex.props(styles.empty)}>Planning has not produced an execution contract yet.</p></section>;
  return (
    <section aria-labelledby="task-contract-title" {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.heading)}><h2 id="task-contract-title" {...stylex.props(styles.title)}>Current contract</h2><span {...stylex.props(styles.meta)}>Version {contract.version} · {sentenceCase(contract.complexity)}</span></div>
      <div {...stylex.props(styles.markdown)}><Markdown density="default" headingLevelStart={3}>{contract.requestMarkdown}</Markdown></div>
      {contract.executionPlanMarkdown ? <details><summary {...stylex.props(styles.summary)}>Execution plan</summary><div {...stylex.props(styles.markdown)}><Markdown density="default" headingLevelStart={3}>{contract.executionPlanMarkdown}</Markdown></div></details> : null}
      <div {...stylex.props(styles.criteria)}><h3 {...stylex.props(styles.criteriaTitle)}>Acceptance criteria</h3><ol {...stylex.props(styles.criteriaList)}>{contract.criteria.map((criterion) => <li key={criterion.criterionId} {...stylex.props(styles.criterion)}><strong {...stylex.props(styles.criterionText)}>{criterion.description}</strong>{criterion.expectedEvidence ? <span {...stylex.props(styles.evidence)}>{criterion.expectedEvidence}</span> : null}</li>)}</ol></div>
      <details><summary {...stylex.props(styles.summary)}>Frozen execution context</summary><dl {...stylex.props(styles.details)}><div {...stylex.props(styles.detailRow)}><dt {...stylex.props(styles.term)}>Executor</dt><dd {...stylex.props(styles.definition)}>{contract.executorModel.providerKind} · {contract.executorModel.modelProfile ?? "provider default"}</dd></div><div {...stylex.props(styles.detailRow)}><dt {...stylex.props(styles.term)}>Reviewer</dt><dd {...stylex.props(styles.definition)}>{contract.reviewerModel.providerKind} · {contract.reviewerModel.modelProfile ?? "provider default"}</dd></div><div {...stylex.props(styles.detailRow)}><dt {...stylex.props(styles.term)}>Workspace</dt><dd {...stylex.props(styles.definition)}>{contract.workspaceContext.name}</dd></div>{contract.projectContext ? <div {...stylex.props(styles.detailRow)}><dt {...stylex.props(styles.term)}>Project</dt><dd {...stylex.props(styles.definition)}>{contract.projectContext.name}</dd></div> : null}<div {...stylex.props(styles.detailRow)}><dt {...stylex.props(styles.term)}>Limits</dt><dd {...stylex.props(styles.definition)}>{contract.executionPolicy.maxToolCalls} tools · {contract.executionPolicy.maxActiveMinutes} active min · {contract.executionPolicy.maxReviewRounds} review rounds</dd></div></dl></details>
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 14, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 20 }, title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 }, summary: { width: "fit-content", color: "var(--pine-700)", fontSize: 12, fontWeight: 700 }, heading: { display: "flex", flexWrap: "wrap", alignItems: "baseline", justifyContent: "space-between", gap: 8 }, meta: { color: "var(--muted-foreground)", fontSize: 11 }, markdown: { maxWidth: 820, color: "var(--text-secondary)", fontSize: 13, lineHeight: 1.55 }, criteria: { display: "grid", gap: 8 }, criteriaTitle: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 14 }, criteriaList: { display: "grid", gap: 8, margin: 0, paddingLeft: 24 }, criterion: { paddingLeft: 4, color: "var(--text-secondary)", fontSize: 13 }, criterionText: { display: "block" }, evidence: { display: "block", marginTop: 2, color: "var(--muted-foreground)", fontSize: 11 }, details: { display: "grid", gap: 7, margin: "10px 0 0" }, detailRow: { display: "grid", gridTemplateColumns: "110px minmax(0, 1fr)", gap: 10 }, term: { color: "var(--muted-foreground)", fontSize: 11 }, definition: { margin: 0, overflowWrap: "anywhere", fontSize: 12 }, empty: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }
});
