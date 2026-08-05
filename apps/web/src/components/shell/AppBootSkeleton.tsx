import * as stylex from "@stylexjs/stylex";
import { Composer } from "@/components/Composer";
import { TranscriptLoadingSkeleton } from "@/components/transcript/TranscriptLoadingSkeleton";
import { PrimarySurfaceNavigation, shellRootStyle } from "./AppShell";

export function AppBootSkeleton({ animateGlimmer = true }: { animateGlimmer?: boolean }) {
  return (
    <main style={shellRootStyle()} {...stylex.props(styles.shellRoot)} aria-label="Loading Noema">
      <header {...stylex.props(styles.navbar)}>
        <div aria-hidden="true" inert {...stylex.props(styles.headerOffset)}>
          <PrimarySurfaceNavigation
            route={{ kind: "chat" }}
            agentName={null}
            agentAvatarActivity="idle"
            attention={null}
            onNavigate={() => undefined}
          />
        </div>
      </header>
      <section {...stylex.props(styles.deck)} aria-label="Home">
        <div {...stylex.props(styles.chatSurface)}>
          <TranscriptLoadingSkeleton animateGlimmer={animateGlimmer} />
          <div {...stylex.props(styles.composerDock)}>
            <div aria-hidden="true" {...stylex.props(styles.composerScrim)} />
            <div aria-hidden="true" inert {...stylex.props(styles.composerLayer)}>
              <Composer
                value=""
                ready={false}
                editable={false}
                pending={false}
                placeholder="Send a message"
                onChange={() => undefined}
                onSubmit={() => undefined}
              />
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
  deck: {
    "--shell-deck-header-height": "0px",
    position: "absolute",
    top: "calc(52px + var(--shell-safe-top))",
    right: 8,
    bottom: 8,
    left: 8,
    zIndex: 30,
    display: "grid",
    minHeight: 0,
    gridTemplateRows: "minmax(0, 1fr)",
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: "var(--radius-page)",
    cornerShape: "var(--corner-shape-page)",
    backgroundColor: "var(--background)",
    boxShadow: "var(--shadow-shell-frame)",
    "@media (max-width: 760px)": {
      right: 0,
      bottom: 0,
      left: 0,
      borderWidth: 0,
      borderRadius: "var(--radius-page) var(--radius-page) 0 0"
    }
  },
  navbar: {
    position: "absolute",
    top: "var(--shell-safe-top)",
    right: 0,
    left: 0,
    zIndex: 40,
    display: "flex",
    boxSizing: "border-box",
    minWidth: 0,
    height: 52,
    alignItems: "center"
  },
  headerOffset: {
    position: "relative",
    zIndex: 1,
    display: "flex",
    flex: 1,
    minWidth: 0,
    alignItems: "center",
    boxSizing: "border-box"
  },
  chatSurface: {
    "--chat-column-width": {
      default: "min(var(--shell-content-max-width), calc(100% - 48px))",
      "@media (max-width: 760px)": "calc(100% - 40px)"
    },
    "--chat-composer-dock-height": {
      default: "96px",
      "@media (hover: none) and (pointer: coarse)": "92px"
    },
    position: "relative",
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr)",
    minHeight: 0,
    height: "100%",
    width: "100%",
    overflow: "hidden"
  },
  composerDock: {
    position: "relative",
    zIndex: 2,
    display: "grid",
    gridArea: "1 / 1",
    alignSelf: "end",
    paddingBottom: {
      default: 22,
      "@media (hover: none) and (pointer: coarse)": "max(18px, env(safe-area-inset-bottom))"
    }
  },
  composerScrim: {
    position: "absolute",
    top: "calc(-1 * var(--spacing-12))",
    right: "var(--spacing-4)",
    bottom: 0,
    left: 0,
    zIndex: 0,
    backgroundImage:
      "linear-gradient(to bottom, rgb(255 255 255 / 0), rgb(255 255 255 / 0.38) 50%, var(--background) 100%)"
  },
  composerLayer: {
    position: "relative",
    zIndex: 1
  }
});
