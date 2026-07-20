import React from "react";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { ChevronDown, ChevronRight } from "lucide-react";
import { memoryPageUrlPath } from "@/app/routes";
import { styles } from "@/pages/memoryPageStyles";

type TreePage = {
  id: string;
  path: string;
  title: string;
};

type TreeNode = TreePage & {
  children: TreeNode[];
};

export function MemoryPageTree({
  root,
  pages,
  activePath,
  presentation = "sidebar",
  onNavigate
}: {
  root: TreePage;
  pages: TreePage[];
  activePath: string;
  presentation?: "sidebar" | "sheet";
  onNavigate?: () => void;
}) {
  const tree = React.useMemo(() => buildTree(root, pages), [pages, root]);
  const [collapsedPaths, setCollapsedPaths] = React.useState<Set<string>>(() => new Set());

  const toggle = React.useCallback((path: string) => {
    setCollapsedPaths((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  return (
    <nav
      aria-label="Memory pages"
      {...stylex.props(
        styles.pageTree,
        presentation === "sidebar" ? styles.pageTreeSidebar : styles.pageTreeSheet
      )}
    >
      <ul {...stylex.props(styles.pageTreeList)}>
        <MemoryTreeItem
          activePath={activePath}
          collapsedPaths={collapsedPaths}
          node={tree}
          rootPath={root.path}
          onNavigate={onNavigate}
          onToggle={toggle}
        />
      </ul>
    </nav>
  );
}

function MemoryTreeItem({
  node,
  rootPath,
  activePath,
  collapsedPaths,
  onNavigate,
  onToggle
}: {
  node: TreeNode;
  rootPath: string;
  activePath: string;
  collapsedPaths: Set<string>;
  onNavigate?: () => void;
  onToggle: (path: string) => void;
}) {
  const hasChildren = node.children.length > 0;
  const collapsed = collapsedPaths.has(node.path);
  const active = node.path === activePath;

  return (
    <li {...stylex.props(styles.pageTreeItem)}>
      <div {...stylex.props(styles.pageTreeRow)}>
        {hasChildren ? (
          <button
            type="button"
            aria-expanded={!collapsed}
            aria-label={`${collapsed ? "Expand" : "Collapse"} ${node.title}`}
            {...stylex.props(styles.pageTreeDisclosure)}
            onClick={() => onToggle(node.path)}
          >
            {collapsed ? <ChevronRight aria-hidden="true" size={15} /> : <ChevronDown aria-hidden="true" size={15} />}
          </button>
        ) : (
          <span aria-hidden="true" {...stylex.props(styles.pageTreeDisclosureSpacer)} />
        )}
        <Link
          to={node.path === rootPath ? "/memory" : "/memory/$"}
          params={node.path === rootPath ? undefined : { _splat: memoryPageUrlPath(node.path) }}
          aria-current={active ? "page" : undefined}
          onClick={onNavigate}
          {...stylex.props(styles.pageTreeLink, active && styles.pageTreeLinkActive)}
        >
          {node.title}
        </Link>
      </div>
      {hasChildren && !collapsed ? (
        <ul {...stylex.props(styles.pageTreeChildren)}>
          {node.children.map((child) => (
            <MemoryTreeItem
              key={child.id}
              activePath={activePath}
              collapsedPaths={collapsedPaths}
              node={child}
              rootPath={rootPath}
              onNavigate={onNavigate}
              onToggle={onToggle}
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
