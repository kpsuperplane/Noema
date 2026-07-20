import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Markdown } from "@astryxdesign/core/Markdown";
import { useState } from "react";
import * as stylex from "@stylexjs/stylex";
import {
  MemoryPageDocument,
  MemoryTreeDocument,
  UpdateMemoryDocument,
  type MemoryPageQuery,
  type MemoryPageQueryVariables,
  type MemoryTreeQuery,
  type UpdateMemoryMutation
} from "@/generated/graphql";

type NativePage = NonNullable<MemoryTreeQuery["memoryTree"]["root"]>;

export function MemoryPage() {
  const treeResult = useQuery<MemoryTreeQuery>(MemoryTreeDocument, {
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true,
    pollInterval: 3000
  });
  const [selectedPageId, setSelectedPageId] = useState<string | null>(null);
  const [updateMemory, updateResult] = useMutation<UpdateMemoryMutation>(UpdateMemoryDocument, {
    refetchQueries: [{ query: MemoryTreeDocument }],
    awaitRefetchQueries: true
  });
  const root = treeResult.data?.memoryTree.root ?? null;
  const activePageId = selectedPageId ?? root?.id ?? null;
  const pageResult = useQuery<MemoryPageQuery, MemoryPageQueryVariables>(MemoryPageDocument, {
    variables: { pageId: activePageId ?? "" },
    skip: !activePageId || activePageId === root?.id,
    fetchPolicy: "cache-and-network"
  });
  const selectedPage =
    activePageId === root?.id
      ? root
      : pageResult.data?.memoryPage ?? null;

  const tree = treeResult.data?.memoryTree;
  const loading = treeResult.loading && !treeResult.data;
  const error = treeResult.error?.message ?? null;
  const status = tree?.updateStatus;
  const updating = Boolean(updateResult.loading || status?.active);
  const pendingCount = tree?.pendingCount ?? 0;
  const retryable = status?.state === "error" || Boolean(updateResult.error);
  const canUpdate = pendingCount > 0 || retryable;
  const selectedChildren = selectedPage?.children ?? [];

  return (
    <section data-slot="memory-surface" {...stylex.props(styles.surface)} aria-labelledby="memory-title">
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.heading)}>
          <span {...stylex.props(styles.eyebrow)}>Native memory</span>
          <h1 id="memory-title" {...stylex.props(styles.title)}>
            {selectedPage?.title ?? "Memory"}
          </h1>
          <p {...stylex.props(styles.path)}>{selectedPage?.path ?? "memory/human/root.md"}</p>
        </div>
        <div {...stylex.props(styles.headerActions)}>
          <span role="status" {...stylex.props(styles.status)}>
            {statusLabel(status?.state, pendingCount)}
          </span>
          {status?.updatedAt ? <span {...stylex.props(styles.updatedAt)}>Saved {formatDate(status.updatedAt)}</span> : null}
          <Button
            type="button"
            size="sm"
            variant="primary"
            label={retryable ? "Retry memory" : "Update memory"}
            isLoading={updating}
            isDisabled={updating || !tree || Boolean(error) || !canUpdate}
            onClick={() => void updateMemory()}
          />
        </div>
      </div>

      {error ? (
        <div role="alert" {...stylex.props(styles.notice, styles.errorNotice)}>
          Could not load native memory: {error}
        </div>
      ) : null}
      {status?.error ? (
        <div role="alert" {...stylex.props(styles.notice, styles.errorNotice)}>
          The last memory update failed: {status.error}
        </div>
      ) : null}
      {updateResult.error ? (
        <div role="alert" {...stylex.props(styles.notice, styles.errorNotice)}>
          Could not start the memory update: {updateResult.error.message}
        </div>
      ) : null}
      {loading ? <div role="status" {...stylex.props(styles.notice)}>Loading native memory…</div> : null}

      <div {...stylex.props(styles.layout)}>
        <aside {...stylex.props(styles.tree)} aria-label="Memory pages">
          <div {...stylex.props(styles.treeLabel)}>Pages</div>
          {root ? (
            <>
              <PageButton page={root} selected={activePageId === root.id} onSelect={setSelectedPageId} />
              {root.children.length > 0 ? (
                <div {...stylex.props(styles.children)}>
                  {root.children.map((child) => (
                    <PageButton key={child.id} page={child} selected={activePageId === child.id} onSelect={setSelectedPageId} />
                  ))}
                </div>
              ) : (
                <p {...stylex.props(styles.emptyTree)}>Child pages appear after the first update.</p>
              )}
              {selectedPage && selectedPage.id !== root.id && selectedChildren.length > 0 ? (
                <div {...stylex.props(styles.children)}>
                  {selectedChildren.map((child) => (
                    <PageButton key={child.id} page={child} selected={activePageId === child.id} onSelect={setSelectedPageId} />
                  ))}
                </div>
              ) : null}
            </>
          ) : !loading ? (
            <p {...stylex.props(styles.emptyTree)}>No native memory page is available.</p>
          ) : null}
        </aside>

        <article {...stylex.props(styles.page)}>
          {selectedPage ? (
            <>
              <div {...stylex.props(styles.pageMeta)}>
                <span>{selectedPage.path}</span>
                <code>{selectedPage.hash.slice(0, 12)}</code>
              </div>
              <Markdown autolink="gfm" contentWidth="100%" density="default" headingLevelStart={2}>
                {selectedPage.body}
              </Markdown>
            </>
          ) : pageResult.loading ? (
            <p {...stylex.props(styles.muted)}>Loading page…</p>
          ) : (
            <p {...stylex.props(styles.muted)}>Select a page to inspect its Markdown.</p>
          )}
        </article>
      </div>
    </section>
  );
}

function PageButton({ page, selected, onSelect }: { page: NativePage | NativePage["children"][number]; selected: boolean; onSelect: (id: string) => void }) {
  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      {...stylex.props(styles.pageButton, selected && styles.selectedPageButton)}
      onClick={() => onSelect(page.id)}
    >
      <span {...stylex.props(styles.pageTitle)}>{page.title}</span>
      <span {...stylex.props(styles.pagePath)}>{page.path}</span>
    </button>
  );
}

function statusLabel(state: string | null | undefined, pendingCount: number): string {
  if (state === "running") return "Updating memory…";
  if (pendingCount > 0) return `${pendingCount} conversation item${pendingCount === 1 ? "" : "s"} pending`;
  return "Up to date";
}

function formatDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp)
    ? value
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

const styles = stylex.create({
  surface: {
    display: "grid",
    gridTemplateRows: "auto 1fr",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    overflow: "hidden",
    backgroundColor: "var(--surface-base)"
  },
  header: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "end",
    justifyContent: "space-between",
    gap: "var(--spacing-3)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)",
    padding: "var(--spacing-4) var(--spacing-5)"
  },
  heading: { display: "grid", gap: "var(--spacing-0-5)", minWidth: 0 },
  eyebrow: { color: "var(--muted-foreground)", fontSize: 11, fontWeight: 650, letterSpacing: "0.06em", textTransform: "uppercase" },
  title: { margin: 0, color: "var(--foreground)", fontFamily: "var(--font-heading)", fontSize: 24, lineHeight: 1.2, overflowWrap: "anywhere" },
  path: { margin: 0, color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", fontSize: 11, overflowWrap: "anywhere" },
  headerActions: { display: "flex", alignItems: "center", justifyContent: "end", flexWrap: "wrap", gap: "var(--spacing-2)" },
  status: { color: "var(--muted-foreground)", fontSize: 12 },
  updatedAt: { color: "var(--muted-foreground)", fontSize: 11 },
  notice: { margin: "var(--spacing-3) var(--spacing-5) 0", borderRadius: 6, backgroundColor: "var(--surface-sunken)", padding: "var(--spacing-2) var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.45 },
  errorNotice: { backgroundColor: "color-mix(in srgb, var(--destructive) 8%, var(--surface-base))", color: "var(--destructive)" },
  layout: { display: "grid", gridTemplateColumns: "minmax(180px, 240px) minmax(0, 1fr)", minHeight: 0, overflow: "hidden", "@media (max-width: 760px)": { gridTemplateColumns: "1fr", gridTemplateRows: "auto 1fr" } },
  tree: { minWidth: 0, overflowY: "auto", borderRightWidth: 1, borderRightStyle: "solid", borderRightColor: "var(--border-subtle)", padding: "var(--spacing-4)", "@media (max-width: 760px)": { borderRightStyle: "none", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", maxHeight: 220 } },
  treeLabel: { marginBottom: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: 11, fontWeight: 650, letterSpacing: "0.06em", textTransform: "uppercase" },
  children: { display: "grid", gap: "var(--spacing-1)", marginTop: "var(--spacing-1)", paddingLeft: "var(--spacing-3)" },
  pageButton: { display: "grid", gap: "var(--spacing-0-5)", width: "100%", minWidth: 0, borderWidth: 1, borderStyle: "solid", borderColor: "transparent", borderRadius: 6, backgroundColor: "transparent", padding: "var(--spacing-2)", color: "var(--foreground)", textAlign: "start", cursor: "pointer", ":hover": { backgroundColor: "var(--surface-hover)" } },
  selectedPageButton: { borderColor: "var(--border-subtle)", backgroundColor: "var(--surface-raised)" },
  pageTitle: { fontSize: 13, fontWeight: 600, overflowWrap: "anywhere" },
  pagePath: { color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", fontSize: 10, overflowWrap: "anywhere" },
  emptyTree: { margin: "var(--spacing-3) 0 0", color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.45 },
  page: { minWidth: 0, minHeight: 0, overflowY: "auto", padding: "var(--spacing-5)", "@media (max-width: 760px)": { padding: "var(--spacing-4)" } },
  pageMeta: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", gap: "var(--spacing-2)", marginBottom: "var(--spacing-4)", color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", fontSize: 11 },
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }
});
