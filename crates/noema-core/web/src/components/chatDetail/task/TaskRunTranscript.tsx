import { useQuery } from "@apollo/client/react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { FileText, Wrench } from "lucide-react";
import * as React from "react";
import { TaskRunItemsDocument, type TaskRunItemsQuery } from "@/generated/graphql";
import { mapTaskRunItem, mergeTaskRunItems } from "./taskRunItemMapper";
import type { TaskRun, TaskRunItem } from "./taskTypes";

type MarkdownXStyle = MarkdownProps["xstyle"];
type RunItemNode = TaskRunItemsQuery["taskRunItems"]["items"][number];

export function TaskRunTranscript({
  run,
  liveItems = []
}: {
  run: TaskRun;
  liveItems?: readonly TaskRunItem[];
}) {
  const { data, error, loading, fetchMore } = useQuery(TaskRunItemsDocument, {
    fetchPolicy: "cache-and-network",
    variables: { runId: run.id, first: 50 }
  });
  const [olderItems, setOlderItems] = React.useState<readonly TaskRunItem[]>([]);
  const [pageInfoOverride, setPageInfoOverride] = React.useState<{
    endCursor: string | null;
    hasNextPage: boolean;
  } | null>(null);
  const [loadingOlder, setLoadingOlder] = React.useState(false);
  const pageInfo = pageInfoOverride ?? data?.taskRunItems.pageInfo ?? null;

  const currentItems = React.useMemo(
    () => data?.taskRunItems.items.map((item) => mapNode(item, run)) ?? [],
    [data?.taskRunItems.items, run]
  );
  const items = React.useMemo(
    () => mergeTaskRunItems(olderItems, currentItems, liveItems),
    [currentItems, liveItems, olderItems]
  );
  const groups = React.useMemo(() => groupByRound(items), [items]);

  const loadOlder = async () => {
    if (!pageInfo?.hasNextPage || loadingOlder) {
      return;
    }
    setLoadingOlder(true);
    try {
      const result = await fetchMore({
        variables: { runId: run.id, after: pageInfo.endCursor, first: 50 }
      });
      const next = result.data?.taskRunItems;
      if (!next) {
        return;
      }
      setOlderItems((previous) =>
        mergeTaskRunItems(next.items.map((item) => mapNode(item, run)), previous)
      );
      setPageInfoOverride(next.pageInfo);
    } finally {
      setLoadingOlder(false);
    }
  };

  if (loading && items.length === 0) {
    return <p {...stylex.props(styles.state)}>Loading agent transcript...</p>;
  }
  if (error && items.length === 0) {
    return <p role="alert" {...stylex.props(styles.error)}>Agent transcript could not be loaded.</p>;
  }
  if (items.length === 0) {
    return null;
  }

  return (
    <section aria-label={`${runRoleLabel(run)} transcript`} {...stylex.props(styles.transcript)}>
      <div {...stylex.props(styles.header)}>
        <h4 {...stylex.props(styles.title)}>Agent transcript</h4>
        <span {...stylex.props(styles.hint)}>{items.length} events</span>
      </div>
      {pageInfo?.hasNextPage ? (
        <Button
          clickAction={loadOlder}
          isLoading={loadingOlder}
          label="Load older activity"
          size="sm"
          variant="ghost"
        />
      ) : null}
      <div role="log" aria-live="polite" {...stylex.props(styles.rounds)}>
        {groups.map((group) => (
          <section key={group.roundIndex ?? "unassigned"} {...stylex.props(styles.round)}>
            <div {...stylex.props(styles.roundHeader)}>
              <span>{roundLabel(group.roundIndex)}</span>
              <span>{group.items.length}</span>
            </div>
            <div {...stylex.props(styles.items)}>
              {group.items.map((item) => (
                <TranscriptItem key={item.id} item={item} items={items} />
              ))}
            </div>
          </section>
        ))}
      </div>
    </section>
  );
}

function TranscriptItem({ item, items }: { item: TaskRunItem; items: readonly TaskRunItem[] }) {
  const isMessage = item.kind === "message";
  const isInput = item.kind === "input";
  const content = item.details || item.summary;
  const status = correlatedStatus(item, items);
  return (
    <article {...stylex.props(styles.item, isMessage && styles.message)}>
      <div {...stylex.props(styles.itemHeader)}>
        <span {...stylex.props(styles.itemTitle)}>
          {item.kind === "tool" || item.kind === "result" ? (
            <Wrench aria-hidden="true" size={12} />
          ) : (
            <FileText aria-hidden="true" size={12} />
          )}
          {item.title}
        </span>
        {status ? <Badge label={status} variant={statusVariant(status)} {...stylex.props(styles.badge)} /> : null}
      </div>
      {isMessage && item.summary ? (
        <Markdown
          autolink="gfm"
          contentWidth="100%"
          density="compact"
          headingLevelStart={5}
          xstyle={markdownXStyle(styles.markdown)}
        >
          {item.summary}
        </Markdown>
      ) : content ? (
        <pre {...stylex.props(styles.pre)}>{content}</pre>
      ) : isInput ? (
        <p {...stylex.props(styles.itemEmpty)}>No input text recorded.</p>
      ) : null}
    </article>
  );
}

function mapNode(node: RunItemNode, run: TaskRun): TaskRunItem {
  return mapTaskRunItem(node, run.role);
}

function groupByRound(items: readonly TaskRunItem[]) {
  const groups = new Map<number | null, TaskRunItem[]>();
  for (const item of items) {
    const key = item.roundIndex ?? null;
    const group = groups.get(key) ?? [];
    group.push(item);
    groups.set(key, group);
  }
  return [...groups.entries()].map(([roundIndex, roundItems]) => ({ roundIndex, items: roundItems }));
}

function correlatedStatus(item: TaskRunItem, items: readonly TaskRunItem[]): TaskRunItem["status"] {
  if (item.status || item.kind !== "tool" || !item.correlationId) {
    return item.status;
  }
  return items.find(
    (candidate) => candidate.kind === "result" && candidate.correlationId === item.correlationId
  )?.status ?? null;
}

function statusVariant(status: NonNullable<TaskRunItem["status"]>) {
  if (status === "completed") return "success" as const;
  if (status === "failed") return "error" as const;
  if (status === "running") return "info" as const;
  if (status === "cancelled" || status === "skipped") return "warning" as const;
  return "neutral" as const;
}

function roundLabel(roundIndex: number | null): string {
  return roundIndex === null ? "Run setup" : `Round ${roundIndex + 1}`;
}

function runRoleLabel(run: TaskRun): string {
  if (run.role === "reviewer") return "Reviewer";
  if (run.role === "completion_delivery") return "Delivery";
  return "Executor";
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

const styles = stylex.create({
  transcript: { display: "grid", gap: 7, minWidth: 0 },
  header: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 7 },
  title: { margin: 0, color: "var(--noema-text-primary)", fontSize: 11, fontWeight: 700 },
  hint: { color: "var(--noema-text-muted)", fontSize: 10, fontVariantNumeric: "tabular-nums" },
  rounds: { display: "grid", gap: 9, maxHeight: 560, overflowY: "auto", minWidth: 0, padding: 1 },
  round: { display: "grid", gap: 5, minWidth: 0 },
  roundHeader: {
    display: "flex",
    justifyContent: "space-between",
    color: "var(--noema-text-muted)",
    fontSize: 9,
    fontWeight: 700,
    letterSpacing: "0.06em",
    textTransform: "uppercase"
  },
  items: { display: "grid", gap: 5 },
  item: { display: "grid", gap: 6, minWidth: 0, borderRadius: 7, backgroundColor: "var(--noema-surface-card)", padding: 8 },
  message: { backgroundColor: "color-mix(in srgb, var(--noema-pine-100) 24%, var(--noema-surface-card))" },
  itemHeader: { display: "flex", alignItems: "start", justifyContent: "space-between", gap: 7, minWidth: 0 },
  itemTitle: { display: "inline-flex", alignItems: "center", gap: 5, minWidth: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  badge: { flexShrink: 0, fontSize: 9 },
  markdown: { color: "var(--noema-text-primary)", fontSize: 12, lineHeight: 1.5 },
  pre: { maxWidth: "100%", margin: 0, overflowX: "auto", whiteSpace: "pre-wrap", overflowWrap: "anywhere", color: "var(--noema-text-secondary)", fontFamily: "var(--noema-font-mono)", fontSize: 10, lineHeight: 1.45 },
  itemEmpty: { margin: 0, color: "var(--noema-text-muted)", fontSize: 10 },
  state: { margin: 0, color: "var(--noema-text-muted)", fontSize: 11 },
  error: { margin: 0, color: "var(--noema-red-700)", fontSize: 11, lineHeight: 1.35 }
});
