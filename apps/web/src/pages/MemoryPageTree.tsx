import React from "react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { memoryPageUrlPath } from "@/app/routes";
import {
  ShellSidebarMenuLabel,
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

  return (
    <nav
      aria-label="Memory pages"
      data-slot="shell-sidebar-nav"
      {...stylex.props(shellSidebarStyles.nav, styles.pageTree)}
    >
      <ul {...stylex.props(shellSidebarStyles.sideNavBody, styles.pageTreeList)}>
        <MemoryTreeItem
          activePath={activePath}
          node={tree}
          rootPath={root.path}
          onNavigate={onNavigate}
        />
      </ul>
    </nav>
  );
}

function MemoryTreeItem({
  node,
  rootPath,
  activePath,
  onNavigate
}: {
  node: TreeNode;
  rootPath: string;
  activePath: string;
  onNavigate?: () => void;
}) {
  const active = node.path === activePath;

  return (
    <li {...stylex.props(styles.pageTreeItem)}>
      <Link
        data-slot="shell-sidebar-control"
        to={node.path === rootPath ? "/memory" : "/memory/$"}
        params={node.path === rootPath ? undefined : { _splat: memoryPageUrlPath(node.path) }}
        aria-current={active ? "page" : undefined}
        onClick={onNavigate}
        {...stylex.props(
          shellSidebarStyles.menuButton,
          active && shellSidebarStyles.menuButtonActive,
          styles.pageTreeLink
        )}
      >
        <span {...stylex.props(shellSidebarStyles.menuIcon)}>
          <MemoryPageIcon icon={node.icon} size={16} />
        </span>
        <ShellSidebarMenuLabel>{node.title}</ShellSidebarMenuLabel>
      </Link>
      {node.children.length > 0 ? (
        <ul {...stylex.props(styles.pageTreeChildren)}>
          {node.children.map((child) => (
            <MemoryTreeItem
              key={child.id}
              activePath={activePath}
              node={child}
              rootPath={rootPath}
              onNavigate={onNavigate}
            />
          ))}
        </ul>
      ) : null}
    </li>
  );
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
