import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { normalizeWorkSearch, type WorkAttention, type WorkProject } from "./workTypes";
import { AttentionBadge } from "./AttentionBadge";
import { relativeTime } from "./workModel";
import { TaskActions } from "./TaskActions";

export function AttentionGroup({ title, items, projects, onUpdated }: { title: string; items: readonly WorkAttention[]; projects: readonly WorkProject[]; onUpdated: () => void | Promise<void> }) {
  const id = `attention-${title.toLowerCase().replaceAll(" ", "-")}`;
  const focusAfterUpdate = React.useCallback(async (taskId: string, nextTaskId?: string) => {
    await onUpdated();
    window.requestAnimationFrame(() => {
      const current = document.getElementById(attentionItemId(taskId));
      const next = nextTaskId ? document.getElementById(attentionItemId(nextTaskId)) : null;
      const target = current
        ?? next
        ?? document.getElementById(id)
        ?? document.getElementById("needs-you-title")
        ?? document.getElementById("needs-you-empty-title");
      target?.focus({ preventScroll: true });
    });
  }, [id, onUpdated]);

  return (
    <section aria-labelledby={id} {...stylex.props(styles.group)}>
      <header {...stylex.props(styles.header)}><h3 id={id} tabIndex={-1} {...stylex.props(styles.headerTitle)}>{title}</h3><span {...stylex.props(styles.count)}>{items.length}</span></header>
      <div {...stylex.props(styles.items)}>{items.map((item, index) => <article id={attentionItemId(item.task.taskId)} tabIndex={-1} key={`${item.kind}:${item.task.taskId}`} {...stylex.props(styles.item)}><div {...stylex.props(styles.copy)}><AttentionBadge kind={item.kind} label={item.title} /><Link to="/work/tasks/$taskId" params={{ taskId: item.task.taskId }} search={(current) => normalizeWorkSearch(current)} {...stylex.props(styles.title)}>{item.task.title}</Link><p {...stylex.props(styles.summary)}>{item.summary}</p><span {...stylex.props(styles.meta)}>{item.task.project?.name ?? "No project"} · {relativeTime(item.task.updatedAt)}</span></div><TaskActions task={item.task} validActions={item.validActions} projects={projects} onUpdated={() => focusAfterUpdate(item.task.taskId, items[index + 1]?.task.taskId)} /></article>)}</div>
    </section>
  );
}

function attentionItemId(taskId: string): string {
  return `attention-item-${taskId.replaceAll(":", "-")}`;
}

const styles = stylex.create({
  group: { display: "grid", gap: 9 },
  header: { display: "flex", alignItems: "center", gap: 8 },
  headerTitle: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 14 },
  count: { color: "var(--muted-foreground)", fontSize: 12 },
  items: { display: "grid", gap: 8 },
  item: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", alignItems: "center", gap: 18, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 12, padding: 14, outline: "none", ":focus-visible": { borderColor: "var(--ring)", boxShadow: "0 0 0 2px var(--ring)" }, "@media (max-width: 760px)": { gridTemplateColumns: "1fr" } },
  copy: { display: "grid", justifyItems: "start", gap: 6, minWidth: 0 },
  summary: { margin: 0, color: "var(--text-secondary)", fontSize: 13, lineHeight: 1.45 },
  meta: { color: "var(--muted-foreground)", fontSize: 11 },
  title: { color: "var(--foreground)", fontFamily: "var(--font-heading)", fontSize: 15, fontWeight: 700, textDecoration: "none", ":hover": { textDecoration: "underline" } }
});
