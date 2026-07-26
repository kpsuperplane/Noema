import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import type { ShellMenuItem, ShellMenuLevel } from "./shellNavigation";

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
  return (
    <nav
      aria-label={menuLevel.ariaLabel}
      data-slot="shell-sidebar-nav"
      {...stylex.props(shellSidebarStyles.nav)}
    >
      <div {...stylex.props(shellSidebarStyles.sideNavBody)}>
        {menuLevel.items.map((entry) =>
          entry.kind === "group" ? (
            <ShellSidebarGroupLabel key={`group-${entry.label}`} label={entry.label} />
          ) : (
            <ShellSidebarNavItem
              key={entry.item.itemId}
              item={entry.item}
              active={entry.item.itemId === menuLevel.activeItemId}
              onSelectItem={onSelectItem}
              renderItemContent={renderItemContent}
            />
          )
        )}
      </div>
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
  indent,
  onSelect,
  renderContent
}: {
  active: boolean;
  depth?: number;
  icon: React.ReactNode;
  itemId: string;
  label: string;
  indent?: string;
  onSelect: () => void;
  renderContent?: (defaultControl: React.ReactNode) => React.ReactNode;
}) {
  const defaultControl = (
    <button
      type="button"
      data-slot="shell-sidebar-control"
      aria-current={active ? "page" : undefined}
      {...stylex.props(
        shellSidebarStyles.menuButton,
        shellSidebarStyles.menuButtonEmbedded,
        depth > 0 && shellSidebarStyles.menuButtonIndented,
        active && shellSidebarStyles.menuButtonEmbeddedActive
      )}
      style={indent ? { paddingInlineStart: indent } : undefined}
      onClick={onSelect}
    >
      <span {...stylex.props(shellSidebarStyles.menuIcon)} aria-hidden="true">
        {icon}
      </span>
      <ShellSidebarMenuLabel>{label}</ShellSidebarMenuLabel>
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
    gridTemplateRows: "minmax(0, 1fr)",
    gap: 8,
    height: "100%",
    minHeight: 0,
    width: "100%",
    minWidth: 0,
    paddingBlock: 12,
    paddingInline: 10,
    color: "var(--foreground)"
  },
  sideNavBody: {
    display: "flex",
    minHeight: 0,
    flexDirection: "column",
    gap: 4,
    overflowY: "auto",
    overscrollBehavior: "contain",
    paddingBlock: 2,
    scrollbarWidth: "none"
  },
  groupLabel: {
    paddingBlock: 10,
    paddingInline: 10,
    fontSize: 11,
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
    gap: 10,
    width: "100%",
    flex: 1,
    minWidth: 0,
    height: 34,
    borderWidth: 0,
    borderRadius: 8,
    backgroundColor: "transparent",
    color: "color-mix(in srgb, var(--pine-700) 78%, var(--foreground))",
    paddingBlock: 0,
    paddingInline: 10,
    textAlign: "left",
    fontFamily: "inherit",
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
