import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import { Markdown } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { WorkTaskDetail } from "../workTypes";
import { sentenceCase, timestampLabel } from "../workModel";
import { AttentionBadge } from "../AttentionBadge";
import { WorkTaskGatesPageDocument, WorkTaskMessagesPageDocument } from "@/generated/graphql";
import { WorkHistoryLoadMore } from "./WorkHistoryLoadMore";

export function TaskAttentionHistory({ task }: { task: WorkTaskDetail }) {
  type Gate = WorkTaskDetail["gates"]["edges"][number]["node"];
  type Message = WorkTaskDetail["messages"]["edges"][number]["node"];
  const [olderGates, setOlderGates] = React.useState<Gate[]>([]);
  const [olderMessages, setOlderMessages] = React.useState<Message[]>([]);
  const [gatePageOverride, setGatePage] = React.useState<typeof task.gates.pageInfo | null>(null);
  const [messagePageOverride, setMessagePage] = React.useState<typeof task.messages.pageInfo | null>(null);
  const gatePage = gatePageOverride ?? task.gates.pageInfo;
  const messagePage = messagePageOverride ?? task.messages.pageInfo;
  const [loadGates, gateLoad] = useLazyQuery(WorkTaskGatesPageDocument, { fetchPolicy: "network-only" });
  const [loadMessages, messageLoad] = useLazyQuery(WorkTaskMessagesPageDocument, { fetchPolicy: "network-only" });
  const initialGates = task.gates.edges.map((edge) => edge.node);
  const initialMessages = task.messages.edges.map((edge) => edge.node);
  const gates = [...new Map([...initialGates, ...olderGates].map((item) => [item.gateId, item])).values()];
  const messages = [...new Map([...initialMessages, ...olderMessages].map((item) => [item.messageId, item])).values()];
  const loadMoreGates = async () => {
    const result = await loadGates({ variables: { taskId: task.taskId, after: gatePage.endCursor, first: 20 } });
    const next = result.data?.task?.gates;
    if (!next) return;
    setOlderGates((current) => [...new Map([...current, ...initialGates, ...next.edges.map((edge) => edge.node)].map((item) => [item.gateId, item])).values()]);
    setGatePage(next.pageInfo);
  };
  const loadMoreMessages = async () => {
    const result = await loadMessages({ variables: { taskId: task.taskId, after: messagePage.endCursor, first: 20 } });
    const next = result.data?.task?.messages;
    if (!next) return;
    setOlderMessages((current) => [...new Map([...current, ...initialMessages, ...next.edges.map((edge) => edge.node)].map((item) => [item.messageId, item])).values()]);
    setMessagePage(next.pageInfo);
  };
  return (
    <section aria-labelledby="task-attention-title" {...stylex.props(styles.section)}>
      <h2 id="task-attention-title" {...stylex.props(styles.title)}>Attention and messages</h2>
      {task.activeGate ? <div {...stylex.props(styles.active)}><AttentionBadge kind={task.activeGate.kind} label={task.attention?.title ?? sentenceCase(task.activeGate.kind)} /><strong {...stylex.props(styles.activeTitle)}>{task.activeGate.prompt}</strong>{task.activeGate.contextMarkdown ? <Markdown density="compact" headingLevelStart={3}>{task.activeGate.contextMarkdown}</Markdown> : null}<span {...stylex.props(styles.meta)}>Opened {timestampLabel(task.activeGate.openedAt)}</span></div> : null}
      {messages.length > 0 ? <ol {...stylex.props(styles.timeline)}>{messages.map((message) => <li key={message.messageId} {...stylex.props(styles.timelineItem)}><div {...stylex.props(styles.timelineHeading)}><strong {...stylex.props(styles.messageTitle)}>{sentenceCase(message.kind)}</strong><span {...stylex.props(styles.meta)}>{timestampLabel(message.createdAt)}</span></div><Markdown density="compact" headingLevelStart={3}>{message.bodyMarkdown}</Markdown>{message.approvalDecision ? <small {...stylex.props(styles.meta)}>Decision: {sentenceCase(message.approvalDecision)}</small> : null}</li>)}</ol> : <p {...stylex.props(styles.empty)}>No human messages have been recorded.</p>}
      {messagePage.hasNextPage ? <WorkHistoryLoadMore label="Load older messages" loading={messageLoad.loading} error={Boolean(messageLoad.error)} onClick={() => { void loadMoreMessages().catch(() => undefined); }} /> : null}
      {gates.length > 0 ? <details><summary {...stylex.props(styles.summary)}>Gate history ({gates.length})</summary><ol {...stylex.props(styles.gates)}>{gates.map((gate) => <li key={gate.gateId} {...stylex.props(styles.gate)}><strong {...stylex.props(styles.gateTitle)}>{sentenceCase(gate.kind)} · {sentenceCase(gate.state)}</strong><span {...stylex.props(styles.gatePrompt)}>{gate.prompt}</span><small {...stylex.props(styles.gateMeta)}>{timestampLabel(gate.openedAt)}{gate.recoveryReason ? ` · ${sentenceCase(gate.recoveryReason)}` : ""}</small></li>)}</ol></details> : null}
      {gatePage.hasNextPage ? <WorkHistoryLoadMore label="Load older gates" loading={gateLoad.loading} error={Boolean(gateLoad.error)} onClick={() => { void loadMoreGates().catch(() => undefined); }} /> : null}
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 20 }, title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 }, summary: { width: "fit-content", color: "var(--pine-700)", fontSize: 12, fontWeight: 700 }, active: { display: "grid", justifyItems: "start", gap: 7, borderRadius: 12, backgroundColor: "var(--clay-50)", padding: 14 }, activeTitle: { fontSize: 14 }, meta: { color: "var(--muted-foreground)", fontSize: 11 }, timeline: { display: "grid", gap: 8, margin: 0, padding: 0, listStyle: "none" }, timelineItem: { display: "grid", gap: 5, borderLeftWidth: 2, borderLeftStyle: "solid", borderLeftColor: "var(--pine-100)", paddingLeft: 12 }, timelineHeading: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", gap: 8 }, messageTitle: { fontSize: 12 }, gates: { display: "grid", gap: 7, margin: "10px 0 0", padding: 0, listStyle: "none" }, gate: { display: "grid", gap: 3, borderRadius: 9, backgroundColor: "var(--paper-100)", padding: 10 }, gateTitle: { fontSize: 11 }, gatePrompt: { fontSize: 12 }, gateMeta: { color: "var(--muted-foreground)", fontSize: 10 }, empty: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }
});
