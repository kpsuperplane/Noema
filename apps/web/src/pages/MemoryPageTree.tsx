import React from "react";
import { VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import { useNavigate } from "@tanstack/react-router";
import { memoryPageUrlPath } from "@/app/routes";
import {
  ShellSidebarItem,
  shellSidebarStyles
} from "@/components/shell/ShellSidebar";
import { MemoryPageIcon } from "@/pages/MemoryPageIcon";
import { styles } from "@/pages/memoryPageStyles";

type TreePage = {
  id: string;
  path: string;
  title: string;
  icon: string;
};

type TreeNode = TreePage & {
  children: TreeNode[];
};

export function MemoryPageTree({
  root,
  pages,
  activePath,
  onNavigate
}: {
  root: TreePage;
  pages: TreePage[];
  activePath: string;
  onNavigate?: () => void;
}) {
  const tree = React.useMemo(() => buildTree(root, pages), [pages, root]);
  const navigate = useNavigate();
  const selectPage = (path: string) => {
    onNavigate?.();
    if (path === root.path) {
      void navigate({ to: "/memory" });
      return;
    }
    void navigate({ to: "/memory/$", params: { _splat: memoryPageUrlPath(path) } });
  };
  const pageHref = (path: string) => path === root.path
    ? "/memory"
    : `/memory/${memoryPageUrlPath(path).split("/").map(encodeURIComponent).join("/")}`;

  return (
    <nav
      aria-label="Memory pages"
      data-slot="shell-sidebar-nav"
      {...stylex.props(shellSidebarStyles.nav)}
    >
      <VStack
        as="ul"
        data-slot="shell-sidebar-items"
        gap={1}
        {...stylex.props(shellSidebarStyles.sideNavBody, styles.pageTreeList)}
      >
        <MemoryTreeItem
          activePath={activePath}
          depth={0}
          node={tree}
          pageHref={pageHref}
          onSelectPage={selectPage}
        />
      </VStack>
    </nav>
  );
}

function MemoryTreeItem({
  node,
  depth,
  activePath,
  onSelectPage,
  pageHref
}: {
  node: TreeNode;
  depth: number;
  activePath: string;
  onSelectPage: (path: string) => void;
  pageHref: (path: string) => string;
}) {
  const active = node.path === activePath;
  const deeperPadding = memoryPagePadding(depth);

  return (
    <VStack as="li" gap={1} {...stylex.props(styles.pageTreeItem)}>
      <ShellSidebarItem
        active={active}
        depth={depth}
        icon={<MemoryPageIcon icon={node.icon} size={16} />}
        indent={deeperPadding}
        itemId={node.id}
        label={node.title}
        href={pageHref(node.path)}
        onSelect={() => onSelectPage(node.path)}
      />
      {node.children.length > 0 ? (
        <VStack as="ul" gap={1} {...stylex.props(styles.pageTreeChildren)}>
          {node.children.map((child) => (
            <MemoryTreeItem
              key={child.id}
              activePath={activePath}
              depth={depth + 1}
              node={child}
              pageHref={pageHref}
              onSelectPage={onSelectPage}
            />
          ))}
        </VStack>
      ) : null}
    </VStack>
  );
}

function memoryPagePadding(depth: number): string | undefined {
  if (depth <= 1) return undefined;
  return `calc(var(--spacing-5) + ${Array.from({ length: depth - 1 }, () => "var(--spacing-3)").join(" + ")})`;
}

function buildTree(root: TreePage, pages: TreePage[]): TreeNode {
  const nodes = new Map<string, TreeNode>();
  for (const page of [...pages, root]) {
    nodes.set(page.path, { ...page, children: [] });
  }
  const rootNode = nodes.get(root.path) ?? { ...root, children: [] };
  for (const node of nodes.values()) {
    if (node.path === root.path) continue;
    const parent = nodes.get(parentPagePath(node.path) ?? root.path) ?? rootNode;
    parent.children.push(node);
  }
  for (const node of nodes.values()) {
    node.children.sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
  }
  return rootNode;
}

function parentPagePath(path: string): string | null {
  if (path === "root.md") return null;
  const stem = path.endsWith(".md") ? path.slice(0, -3) : path;
  const separator = stem.lastIndexOf("/");
  return separator < 0 ? "root.md" : `${stem.slice(0, separator)}.md`;
}
