import * as stylex from "@stylexjs/stylex";
import type { ShellMenuItem, ShellMenuLevel } from "./shellNavigation";

export function ShellSidebar({
  menuLevel,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  return (
    <div
      data-slot="shell-sidebar-menu-viewport"
      {...stylex.props(styles.viewport)}
    >
      <ShellSidebarNav menuLevel={menuLevel} onSelectItem={onSelectItem} />
    </div>
  );
}

function ShellSidebarNav({
  menuLevel,
  onSelectItem
}: {
  menuLevel: ShellMenuLevel;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  return (
    <nav
      aria-label={menuLevel.ariaLabel}
      data-slot="shell-sidebar-nav"
      {...stylex.props(styles.nav)}
    >
      <div {...stylex.props(styles.sideNavBody)}>
        {menuLevel.items.map((entry) =>
          entry.kind === "group" ? (
            <ShellSidebarGroupLabel key={`group-${entry.label}`} label={entry.label} />
          ) : (
            <ShellSidebarNavItem
              key={entry.item.itemId}
              item={entry.item}
              active={entry.item.itemId === menuLevel.activeItemId}
              onSelectItem={onSelectItem}
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
      {...stylex.props(styles.groupLabel)}
    >
      {label}
    </div>
  );
}

function ShellSidebarNavItem({
  item,
  active,
  onSelectItem
}: {
  item: ShellMenuItem;
  active: boolean;
  onSelectItem: (item: ShellMenuItem) => void;
}) {
  const Icon = item.icon;

  return (
    <div
      data-slot="shell-menu-item"
      data-shell-menu-item={item.itemId}
      data-current={active ? "true" : undefined}
      {...stylex.props(styles.menuItemFrame)}
    >
      <button
        type="button"
        aria-current={active ? "page" : undefined}
        {...stylex.props(styles.menuButton, active && styles.menuButtonActive)}
        onClick={() => onSelectItem(item)}
      >
        <span {...stylex.props(styles.menuIcon)} aria-hidden="true">
          <Icon size={16} />
        </span>
        <span {...stylex.props(styles.menuLabel)}>{item.label}</span>
      </button>
    </div>
  );
}

const styles = stylex.create({
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
    width: "100%",
    minWidth: 0,
    borderRadius: 8
  },
  menuButton: {
    display: "flex",
    alignItems: "center",
    gap: 10,
    width: "100%",
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
  menuButtonActive: {
    backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)",
    color: "var(--pine-700)",
    fontWeight: 500,
    ":hover": {
      "@media (hover: hover)": {
        backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)"
      }
    },
    ":active": {
      backgroundColor: "color-mix(in srgb, var(--pine-100) 72%, transparent)"
    }
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
