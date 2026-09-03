import * as React from "react";
import { VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import type { ShellMenuEntry, ShellMenuItem, ShellMenuLevel } from "./shellNavigation";
import { hrefForRoute } from "@/app/routes";

export function ShellSidebar({
  menuLevel,
  onSelectItem,
  renderItemContent
}: {
  menuLevel: ShellMenuLevel;
  onSelectItem: (item: ShellMenuItem) => void;
  renderItemContent?: (
    item: ShellMenuItem,
    defaultControl: React.ReactNode
  ) => React.ReactNode;
}) {
  return (
    <div
      data-slot="shell-sidebar-menu-viewport"
      {...stylex.props(shellSidebarStyles.viewport)}
    >
      <ShellSidebarNav
        menuLevel={menuLevel}
        onSelectItem={onSelectItem}
        renderItemContent={renderItemContent}
      />
    </div>
  );
}

function ShellSidebarNav({
  menuLevel,
  onSelectItem,
  renderItemContent
}: {
  menuLevel: ShellMenuLevel;
  onSelectItem: (item: ShellMenuItem) => void;
  renderItemContent?: (
    item: ShellMenuItem,
    defaultControl: React.ReactNode
  ) => React.ReactNode;
}) {
  const firstPinnedIndex = menuLevel.items.findIndex(
    (entry) => entry.kind === "item" && entry.item.pinned
  );
  const bodyItems = firstPinnedIndex === -1
    ? menuLevel.items
    : menuLevel.items.slice(0, firstPinnedIndex);
  const pinnedItems = firstPinnedIndex === -1
    ? []
    : menuLevel.items.slice(firstPinnedIndex);
  const renderEntry = (entry: ShellMenuEntry) => (
    <li key={entry.kind === "group" ? `group-${entry.label}` : entry.item.itemId} {...stylex.props(shellSidebarStyles.listItem)}>
      {entry.kind === "group" ? (
        <ShellSidebarGroupLabel label={entry.label} />
      ) : (
        <ShellSidebarNavItem
          item={entry.item}
          active={entry.item.itemId === menuLevel.activeItemId}
          onSelectItem={onSelectItem}
          renderItemContent={renderItemContent}
        />
      )}
    </li>
  );

  return (
    <nav
      aria-label={menuLevel.ariaLabel}
      data-slot="shell-sidebar-nav"
      {...stylex.props(shellSidebarStyles.nav)}
    >
      <VStack
        as="ul"
        data-slot="shell-sidebar-items"
        gap={1}
        {...stylex.props(shellSidebarStyles.sideNavBody)}
      >
        {bodyItems.map(renderEntry)}
      </VStack>
      {pinnedItems.length > 0 ? (
        <VStack
          as="ul"
          data-slot="shell-sidebar-footer"
          gap={1}
          {...stylex.props(shellSidebarStyles.footer)}
        >
          {pinnedItems.map(renderEntry)}
        </VStack>
      ) : null}
    </nav>
  );
}

function ShellSidebarGroupLabel({ label }: { label: string }) {
  return (
    <div
      data-slot="shell-menu-group-label"
      {...stylex.props(shellSidebarStyles.groupLabel)}
    >
      {label}
    </div>
  );
}

function ShellSidebarNavItem({
  item,
  active,
  onSelectItem,
  renderItemContent
}: {
  item: ShellMenuItem;
  active: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
  renderItemContent?: (
    item: ShellMenuItem,
    defaultControl: React.ReactNode
  ) => React.ReactNode;
}) {
  const Icon = item.icon;
  return (
    <ShellSidebarItem
      active={active}
      depth={item.depth ?? 0}
      icon={<Icon size={16} />}
      itemId={item.itemId}
      label={item.label}
      href={item.route ? hrefForRoute(item.route) : undefined}
      ariaExpanded={item.ariaExpanded}
      onSelect={() => onSelectItem(item)}
      renderContent={renderItemContent
        ? (defaultControl) => renderItemContent(item, defaultControl)
        : undefined}
    />
  );
}

export function ShellSidebarItem({
  active,
  depth = 0,
  icon,
  itemId,
  label,
  href,
  ariaExpanded,
  indent,
  onSelect,
  renderContent
}: {
  active: boolean;
  depth?: number;
  icon: React.ReactNode;
  itemId: string;
  label: string;
  href?: string;
  ariaExpanded?: boolean;
  indent?: string;
  onSelect: () => void;
  renderContent?: (defaultControl: React.ReactNode) => React.ReactNode;
}) {
  const content = (
    <>
      <span {...stylex.props(shellSidebarStyles.menuIcon)} aria-hidden="true">
        {icon}
      </span>
      <ShellSidebarMenuLabel>{label}</ShellSidebarMenuLabel>
    </>
  );
  const controlStyles = stylex.props(
    shellSidebarStyles.menuButton,
    shellSidebarStyles.menuButtonEmbedded,
    depth > 0 && shellSidebarStyles.menuButtonIndented,
    active && shellSidebarStyles.menuButtonEmbeddedActive
  );
  const defaultControl = href ? (
    <a
      href={href}
      data-slot="shell-sidebar-control"
      aria-current={active ? "page" : undefined}
      aria-expanded={ariaExpanded}
      {...controlStyles}
      style={indent ? { paddingInlineStart: indent } : undefined}
      onClick={(event) => {
        if (!shouldHandleSidebarLink(event)) return;
        event.preventDefault();
        onSelect();
      }}
    >
      {content}
    </a>
  ) : (
    <button
      type="button"
      data-slot="shell-sidebar-control"
      aria-current={active ? "page" : undefined}
      aria-expanded={ariaExpanded}
      {...controlStyles}
      style={indent ? { paddingInlineStart: indent } : undefined}
      onClick={onSelect}
    >
      {content}
    </button>
  );

  return (
    <div
      data-slot="shell-menu-item"
      data-shell-menu-item={itemId}
      data-current={active ? "true" : undefined}
      {...stylex.props(
        shellSidebarStyles.menuItemFrame,
        active && shellSidebarStyles.menuItemFrameActive
      )}
    >
      {renderContent?.(defaultControl) ?? defaultControl}
    </div>
  );
}

function shouldHandleSidebarLink(event: React.MouseEvent<HTMLAnchorElement>) {
  return event.button === 0
    && !event.metaKey
    && !event.ctrlKey
    && !event.shiftKey
    && !event.altKey;
}

export function ShellSidebarMenuLabel({ children }: { children: React.ReactNode }) {
  return <span {...stylex.props(shellSidebarStyles.menuLabel)}>{children}</span>;
}

export const shellSidebarStyles = stylex.create({
  viewport: {
    position: "relative",
    height: "100%",
    minHeight: 0,
    overflow: "hidden"
  },
  nav: {
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr) auto",
    gap: "var(--spacing-2)",
    height: "100%",
    minHeight: 0,
    width: "100%",
    minWidth: 0,
    paddingBlock: "var(--spacing-3)",
    paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))",
    color: "var(--foreground)",
    "@media (max-width: 760px)": {
      height: "fit-content",
      maxHeight: "100%"
    }
  },
  sideNavBody: {
    minHeight: 0,
    overflowY: "auto",
    overscrollBehavior: "contain",
    paddingBlock: "var(--spacing-0-5)",
    scrollbarWidth: "none"
  },
  listItem: {
    minWidth: 0,
    listStyle: "none"
  },
  footer: {
    minHeight: 0,
    maxHeight: "50dvh",
    overflowY: "auto",
    paddingTop: "var(--spacing-2)",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)",
    scrollbarWidth: "none"
  },
  groupLabel: {
    paddingBlock: "calc(var(--spacing-2) + var(--spacing-0-5))",
    paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))",
    fontSize: 12,
    fontWeight: 600,
    lineHeight: 1.2,
    letterSpacing: 0,
    color: "var(--muted-foreground)",
    textTransform: "uppercase"
  },
  menuItemFrame: {
    display: "flex",
    alignItems: "center",
    width: "100%",
    minWidth: 0,
    minHeight: 34,
    borderRadius: 8,
    backgroundColor: "transparent",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "background-color",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 44%, transparent)"
      }
    }
  },
  menuItemFrameActive: {
    backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)"
      }
    }
  },
  menuButton: {
    display: "flex",
    alignItems: "center",
    gap: "calc(var(--spacing-2) + var(--spacing-0-5))",
    width: "100%",
    flex: 1,
    minWidth: 0,
    height: 34,
    borderWidth: 0,
    borderRadius: 8,
    backgroundColor: "transparent",
    color: "color-mix(in srgb, var(--pine-700) 78%, var(--foreground))",
    paddingBlock: "var(--spacing-0)",
    paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))",
    textAlign: "left",
    fontFamily: "inherit",
    textDecoration: "none",
    boxSizing: "border-box",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "background-color, color",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 44%, transparent)"
      }
    },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 2
    },
    ":disabled": {
      opacity: 0.55
    }
  },
  menuButtonEmbedded: {
    backgroundColor: "transparent",
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "transparent"
      }
    },
    ":active": {
      backgroundColor: "transparent"
    }
  },
  menuButtonEmbeddedActive: {
    color: "var(--pine-700)",
    fontWeight: 500
  },
  menuButtonIndented: {
    paddingInlineStart: "var(--spacing-5)"
  },
  menuIcon: {
    display: "grid",
    width: 18,
    height: 18,
    flexShrink: 0,
    placeItems: "center"
  },
  menuLabel: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 14,
    fontWeight: "inherit",
    lineHeight: "20px"
  }
});
