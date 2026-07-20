import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { useState, type ReactNode } from "react";
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
import { MemoryArticle } from "@/pages/MemoryArticle";
import { MemoryPageButton } from "@/pages/MemoryPageButton";
import { styles } from "@/pages/memoryPageStyles";

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
  const selectedPage = activePageId === root?.id ? root : pageResult.data?.memoryPage ?? null;
  const tree = treeResult.data?.memoryTree;
  const loading = treeResult.loading && !treeResult.data;
  const error = treeResult.error?.message ?? null;
  const status = tree?.updateStatus;
  const updating = Boolean(updateResult.loading || status?.active);
  const pendingCount = tree?.pendingCount ?? 0;
  const retryable = status?.state === "error" || Boolean(updateResult.error);
  const canUpdate = pendingCount > 0 || retryable;

  return (
    <section data-slot="memory-surface" {...stylex.props(styles.surface)} aria-labelledby="memory-title">
      <header {...stylex.props(styles.toolbar)}>
        <div {...stylex.props(styles.toolbarTitle)}>
          <strong id="memory-title">Memory</strong>
          <span {...stylex.props(styles.toolbarPath)}>{selectedPage?.path ?? "memory/human/root.md"}</span>
        </div>
        <div {...stylex.props(styles.toolbarActions)}>
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
      </header>

      {error ? <MemoryNotice error>Could not load native memory: {error}</MemoryNotice> : null}
      {status?.error ? <MemoryNotice error>The last memory update failed: {status.error}</MemoryNotice> : null}
      {updateResult.error ? <MemoryNotice error>Could not start the memory update: {updateResult.error.message}</MemoryNotice> : null}
      {loading ? <MemoryNotice>Loading native memory…</MemoryNotice> : null}

      <div {...stylex.props(styles.layout)}>
        <aside {...stylex.props(styles.tree)} aria-label="Memory pages">
          <div {...stylex.props(styles.treeLabel)}>Contents</div>
          {root ? (
            <>
              <MemoryPageButton page={root} selected={activePageId === root.id} onSelect={setSelectedPageId} />
              {root.children.length > 0 ? (
                <div {...stylex.props(styles.children)}>
                  {root.children.map((child) => (
                    <MemoryPageButton key={child.id} page={child} selected={activePageId === child.id} onSelect={setSelectedPageId} />
                  ))}
                </div>
              ) : (
                <p {...stylex.props(styles.emptyTree)}>Topic articles appear as memory develops.</p>
              )}
              {selectedPage && selectedPage.id !== root.id && selectedPage.children.length > 0 ? (
                <div {...stylex.props(styles.children)}>
                  {selectedPage.children.map((child) => (
                    <MemoryPageButton key={child.id} page={child} selected={activePageId === child.id} onSelect={setSelectedPageId} />
                  ))}
                </div>
              ) : null}
            </>
          ) : !loading ? (
            <p {...stylex.props(styles.emptyTree)}>No memory article is available.</p>
          ) : null}
        </aside>

        <div {...stylex.props(styles.articleScroller)}>
          {selectedPage ? (
            <MemoryArticle page={selectedPage} />
          ) : pageResult.loading ? (
            <p {...stylex.props(styles.articleState)}>Loading article…</p>
          ) : (
            <p {...stylex.props(styles.articleState)}>Select an article from the contents.</p>
          )}
        </div>
      </div>
    </section>
  );
}

function MemoryNotice({ children, error = false }: { children: ReactNode; error?: boolean }) {
  return (
    <div role={error ? "alert" : "status"} {...stylex.props(styles.notice, error && styles.errorNotice)}>
      {children}
    </div>
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
