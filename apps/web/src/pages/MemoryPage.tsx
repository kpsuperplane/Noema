import { useQuery } from "@apollo/client/react";
import { useState, type ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";
import {
  MemoryPageDocument,
  MemoryTreeDocument,
  type MemoryPageQuery,
  type MemoryPageQueryVariables,
  type MemoryTreeQuery
} from "@/generated/graphql";
import { MemoryArticle } from "@/pages/MemoryArticle";
import { styles } from "@/pages/memoryPageStyles";

export function MemoryPage() {
  const treeResult = useQuery<MemoryTreeQuery>(MemoryTreeDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [selectedPageId, setSelectedPageId] = useState<string | null>(null);
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
  const pageError = pageResult.error?.message ?? null;

  return (
    <section data-slot="memory-surface" {...stylex.props(styles.surface)} aria-label="Memory">
      {error ? <MemoryNotice error>Could not load native memory: {error}</MemoryNotice> : null}
      {tree?.updateStatus.error ? <MemoryNotice error>The last memory update failed: {tree.updateStatus.error}</MemoryNotice> : null}
      {pageError ? <MemoryNotice error>Could not load this memory article: {pageError}</MemoryNotice> : null}
      {loading ? <MemoryNotice>Loading native memory…</MemoryNotice> : null}

      <div {...stylex.props(styles.articleScroller)}>
        {selectedPage && root ? (
          <MemoryArticle
            page={selectedPage}
            isRoot={selectedPage.id === root.id}
            onSelectPage={setSelectedPageId}
            onSelectRoot={() => setSelectedPageId(root.id)}
          />
        ) : pageResult.loading ? (
          <p {...stylex.props(styles.articleState)}>Loading article…</p>
        ) : (
          <p {...stylex.props(styles.articleState)}>No memory article is available.</p>
        )}
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
