import * as stylex from "@stylexjs/stylex";
import { TranscriptLoadingSkeleton } from "@/components/transcript/TranscriptLoadingSkeleton";
import { shellRootStyle } from "./AppShell";

export function AppBootSkeleton() {
  return (
    <main style={shellRootStyle()} {...stylex.props(styles.shellRoot)} aria-label="Loading Noema">
      <div data-slot="skeleton-glimmer" {...stylex.props(styles.settingsButton)} />
      <section {...stylex.props(styles.deck)} aria-label="Home">
        <header {...stylex.props(styles.deckHeader)}>
          <div {...stylex.props(styles.primaryNavigation)}>
            <div data-slot="skeleton-glimmer" {...stylex.props(styles.primaryNavigationActive)} />
            <div data-slot="skeleton-glimmer" {...stylex.props(styles.primaryNavigationItem)} />
            <div data-slot="skeleton-glimmer" {...stylex.props(styles.primaryNavigationItem)} />
          </div>
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
    height: "100dvh",
    overflow: "hidden",
    backgroundColor: "var(--pine-50)",
    color: "var(--foreground)"
  },
  settingsButton: {
    position: "absolute",
    bottom: 16,
    left: 16,
    zIndex: 40,
    width: 36,
    height: 36,
    borderRadius: 999,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  deck: {
    position: "absolute",
    inset: 8,
    zIndex: 30,
    display: "grid",
    minHeight: 0,
    gridTemplateRows: "52px minmax(0, 1fr)",
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 18,
    backgroundColor: "var(--background)",
    boxShadow: "0 0 24px color-mix(in srgb, var(--pine-700), transparent 80%)",
    "@media (max-width: 760px)": {
      inset: 0,
      borderWidth: 0,
      borderRadius: 0
    }
  },
  deckHeader: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-2)"
  },
  primaryNavigation: {
    display: "flex",
    alignItems: "center",
    gap: "var(--spacing-1)",
    transform: {
      default: "none",
      "@media (min-width: 761px)": "translateX(var(--shell-desktop-chrome-offset))"
    }
  },
  primaryNavigationActive: {
    width: 80,
    height: 36,
    borderRadius: 999,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  primaryNavigationItem: {
    width: 36,
    height: 36,
    borderRadius: 999,
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  chatSurface: {
    "--chat-column-width": {
      default: "min(860px, calc(100% - 48px))",
      "@media (max-width: 760px)": "calc(100% - 40px)"
    },
    "--chat-composer-dock-height": {
      default: "96px",
      "@media (hover: none) and (pointer: coarse)": "92px"
    },
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
