import { useQuery } from "@apollo/client/react";
import React, { type ReactNode } from "react";
import { VStack } from "@astryxdesign/core/Stack";
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
import { useShellSurface } from "@/components/shell/ShellSurfaceContext";

export function MemoryPage({ pagePath = null }: { pagePath?: string | null }) {
  const treeResult = useQuery<MemoryTreeQuery>(MemoryTreeDocument, {
    fetchPolicy: "cache-and-network"
  });
  const root = treeResult.data?.memoryTree.root ?? null;
  const activePageId = pagePath ?? root?.id ?? null;
  const pageResult = useQuery<MemoryPageQuery, MemoryPageQueryVariables>(MemoryPageDocument, {
    variables: { pageId: activePageId ?? "" },
    skip: !activePageId || pagePath === null,
    fetchPolicy: "cache-and-network"
  });
  const articlePage = pageResult.data?.memoryPage ?? null;
  const selectedPage = pagePath === null ? root : articlePage;
  const { setMemoryBreadcrumb } = useShellSurface();
  const loading = treeResult.loading && !treeResult.data;
  const error = treeResult.error?.message ?? null;
  const pageError = pageResult.error?.message ?? null;

  React.useEffect(() => {
    if (!pagePath || !articlePage || articlePage.path !== pagePath) {
      setMemoryBreadcrumb(null);
      return;
    }
    setMemoryBreadcrumb({
      ancestors: articlePage.ancestors.map(({ path, title }) => ({ path, title })),
      current: articlePage.title,
      currentPath: articlePage.path
    });
    return () => setMemoryBreadcrumb(null);
  }, [articlePage, pagePath, setMemoryBreadcrumb]);

  return (
    <VStack as="section" data-slot="memory-surface" {...stylex.props(styles.surface)} aria-label="Memory">
      <div {...stylex.props(styles.articleScroller)}>
        {error ? <MemoryNotice error>Could not load native memory: {error}</MemoryNotice> : null}
        {pageError ? <MemoryNotice error>Could not load this memory article: {pageError}</MemoryNotice> : null}
        {loading ? <MemoryNotice>Loading native memory…</MemoryNotice> : null}

        {selectedPage && root ? (
          <MemoryArticle page={selectedPage} />
        ) : pageResult.loading ? (
          <p {...stylex.props(styles.articleState)}>Loading article…</p>
        ) : (
          <p {...stylex.props(styles.articleState)}>No memory article is available.</p>
        )}
      </div>
    </VStack>
  );
}

function MemoryNotice({ children, error = false }: { children: ReactNode; error?: boolean }) {
  return (
    <div role={error ? "alert" : "status"} {...stylex.props(styles.notice, error && styles.errorNotice)}>
      {children}
    </div>
  );
}
