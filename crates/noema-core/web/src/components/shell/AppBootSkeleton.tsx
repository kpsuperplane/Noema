import * as stylex from "@stylexjs/stylex";
import { TranscriptLoadingSkeleton } from "@/components/transcript/TranscriptLoadingSkeleton";

export function AppBootSkeleton() {
  return (
    <main {...stylex.props(styles.shellRoot)} aria-label="Loading Noema">
      <aside {...stylex.props(styles.sidebar)}>
        <div {...stylex.props(styles.sidebarBrand)}>
          <img src="/assets/noema-mark.svg" width="32" height="32" alt="" />
          <div {...stylex.props(styles.sidebarBrandText)}>
            <div data-slot="skeleton-glimmer" {...stylex.props(styles.sidebarTitle)} />
            <div data-slot="skeleton-glimmer" {...stylex.props(styles.sidebarSubtitle)} />
          </div>
        </div>
        <div {...stylex.props(styles.sidebarNav)}>
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.sidebarItem, styles.sidebarItemActive)} />
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.sidebarItem)} />
        </div>
        <div data-slot="skeleton-glimmer" {...stylex.props(styles.sidebarFooter)} />
      </aside>
      <section {...stylex.props(styles.deck)} aria-label="Home">
        <header {...stylex.props(styles.deckHeader)}>
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.headerButton)} />
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.headerTitle)} />
        </header>
        <div {...stylex.props(styles.chatSurface)}>
          <TranscriptLoadingSkeleton />
          <div {...stylex.props(styles.composerDock)}>
            <div {...stylex.props(styles.composerShell)}>
              <div data-slot="skeleton-glimmer" {...stylex.props(styles.composerBubble)}>
                <span {...stylex.props(styles.composerTextLine)} />
                <span {...stylex.props(styles.composerSubmit)} />
              </div>
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}

const styles = stylex.create({
  shellRoot: {
    position: "relative",
    display: "grid",
    height: "100dvh",
    gridTemplateColumns: "216px minmax(0, 1fr)",
    overflow: "hidden",
    backgroundColor: "var(--pine-50)",
    color: "var(--foreground)",
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr"
    }
  },
  sidebar: {
    display: "grid",
    minHeight: 0,
    gridTemplateRows: "auto minmax(0, 1fr) auto",
    gap: 18,
    padding: "18px 12px 14px",
    "@media (max-width: 760px)": {
      display: "none"
    }
  },
  sidebarBrand: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: 10,
    paddingInline: 8
  },
  sidebarBrandText: {
    display: "grid",
    minWidth: 0,
    gap: 6
  },
  sidebarTitle: {
    width: 72,
    height: 13,
    borderRadius: 7,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  sidebarSubtitle: {
    width: 104,
    height: 10,
    borderRadius: 6,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  sidebarNav: {
    display: "grid",
    alignContent: "start",
    gap: 8
  },
  sidebarItem: {
    height: 36,
    borderRadius: 8,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  sidebarItemActive: {
    opacity: 0.9
  },
  sidebarFooter: {
    height: 36,
    borderRadius: 8,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  deck: {
    display: "grid",
    minHeight: 0,
    gridTemplateRows: "44px minmax(0, 1fr)",
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 12,
    backgroundColor: "var(--background)",
    boxShadow: "0 0 24px color-mix(in srgb, var(--pine-700), transparent 80%)",
    margin: 8,
    marginLeft: 0,
    "@media (max-width: 760px)": {
      margin: 0,
      borderWidth: 0,
      borderRadius: 0
    }
  },
  deckHeader: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: 12,
    paddingInline: 16,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)"
  },
  headerButton: {
    width: 32,
    height: 32,
    borderRadius: 8,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  headerTitle: {
    width: 72,
    height: 14,
    borderRadius: 7,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  chatSurface: {
    "--chat-column-width": {
      default: "min(860px, calc(100% - 48px))",
      "@media (max-width: 760px)": "calc(100% - 40px)"
    },
    "--chat-composer-dock-height": "96px",
    position: "relative",
    display: "grid",
    minHeight: 0,
    overflow: "hidden"
  },
  composerDock: {
    position: "absolute",
    right: 0,
    bottom: 0,
    left: 0,
    display: "grid",
    height: "var(--chat-composer-dock-height)",
    alignItems: "end",
    padding: "18px 24px 26px",
    background:
      "linear-gradient(to bottom, rgb(255 255 255 / 0), rgb(255 255 255 / 0.74) 42px, var(--background) 96px)"
  },
  composerShell: {
    display: "flex",
    justifyContent: "flex-end",
    width: "var(--chat-column-width)",
    maxWidth: "100%",
    marginInline: "auto",
    paddingTop: 14,
    transform: "translateY(-6px)"
  },
  composerBubble: {
    position: "relative",
    display: "flex",
    alignItems: "center",
    width: "min(13rem, 100%)",
    maxWidth: "100%",
    height: 48,
    borderRadius: "calc(var(--radius) * 2.6)",
    padding: 6,
    paddingRight: {
      default: 48,
      "@media (hover: none) and (pointer: coarse)": 56
    },
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  composerTextLine: {
    display: "block",
    width: "min(176px, calc(100% - 12px))",
    height: 10,
    marginInline: 10,
    borderRadius: 6,
    backgroundColor: "var(--skeleton-glimmer-line)",
    opacity: 0.78
  },
  composerSubmit: {
    position: "absolute",
    right: {
      default: 6,
      "@media (hover: none) and (pointer: coarse)": 4
    },
    bottom: {
      default: 6,
      "@media (hover: none) and (pointer: coarse)": 4
    },
    width: {
      default: 36,
      "@media (hover: none) and (pointer: coarse)": 44
    },
    height: {
      default: 36,
      "@media (hover: none) and (pointer: coarse)": 44
    },
    borderRadius: 999,
    backgroundColor: "color-mix(in srgb, var(--background) 74%, var(--skeleton-glimmer-base))",
    opacity: 0.86
  }
});
