import { useQuery } from "@apollo/client/react";
import { Dialog, DialogHeader, type DialogProps } from "@astryxdesign/core/Dialog";
import React, { type ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";
import {
  MemoryPageDocument,
  MemoryTreeDocument,
  type MemoryPageQuery,
  type MemoryPageQueryVariables,
  type MemoryTreeQuery
} from "@/generated/graphql";
import { MemoryArticle } from "@/pages/MemoryArticle";
import { MemoryPageTree } from "@/pages/MemoryPageTree";
import { styles } from "@/pages/memoryPageStyles";
import { useShellSurface } from "@/components/shell/ShellSurfaceContext";

export function MemoryPage({ pagePath = null }: { pagePath?: string | null }) {
  const [isPageTreeOpen, setIsPageTreeOpen] = React.useState(false);
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
  const tree = treeResult.data?.memoryTree;
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
      current: articlePage.title
    });
    return () => setMemoryBreadcrumb(null);
  }, [articlePage, pagePath, setMemoryBreadcrumb]);

  return (
    <section data-slot="memory-surface" {...stylex.props(styles.surface)} aria-label="Memory">
      {error ? <MemoryNotice error>Could not load native memory: {error}</MemoryNotice> : null}
      {tree?.updateStatus.error ? <MemoryNotice error>The last memory update failed: {tree.updateStatus.error}</MemoryNotice> : null}
      {pageError ? <MemoryNotice error>Could not load this memory article: {pageError}</MemoryNotice> : null}
      {loading ? <MemoryNotice>Loading native memory…</MemoryNotice> : null}

      <div {...stylex.props(styles.memoryLayout)}>
        {root ? (
          <MemoryPageTree
            activePath={selectedPage?.path ?? pagePath ?? root.path}
            pages={tree?.pages ?? []}
            root={root}
            presentation="sidebar"
          />
        ) : null}
        <div {...stylex.props(styles.articleScroller)}>
          {selectedPage && root ? (
            <MemoryArticle page={selectedPage} onOpenPageTree={() => setIsPageTreeOpen(true)} />
          ) : pageResult.loading ? (
            <p {...stylex.props(styles.articleState)}>Loading article…</p>
          ) : (
            <p {...stylex.props(styles.articleState)}>No memory article is available.</p>
          )}
        </div>
      </div>
      {root ? (
        <Dialog
          aria-label="Memory pages"
          isOpen={isPageTreeOpen}
          maxHeight="min(75dvh, 640px)"
          onOpenChange={setIsPageTreeOpen}
          padding={0}
          position={{ bottom: 0, left: 0, right: 0 }}
          purpose="info"
          width="100dvw"
          xstyle={dialogXStyle(styles.pageTreeSheetDialog)}
        >
          <DialogHeader title="Memory pages" onOpenChange={setIsPageTreeOpen} hasDivider />
          <MemoryPageTree
            activePath={selectedPage?.path ?? pagePath ?? root.path}
            pages={tree?.pages ?? []}
            root={root}
            presentation="sheet"
            onNavigate={() => setIsPageTreeOpen(false)}
          />
        </Dialog>
      ) : null}
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

function dialogXStyle(...xstyle: unknown[]): DialogProps["xstyle"] {
  return xstyle as unknown as DialogProps["xstyle"];
}
