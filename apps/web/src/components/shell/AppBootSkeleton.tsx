import * as stylex from "@stylexjs/stylex";
import { routeFromPathname } from "@/app/routes";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { TaskLoadingSkeleton } from "@/components/chatDetail/task/TaskBody";
import { skeletonGlimmerStyles } from "@/components/skeletonGlimmerStyles";
import { masterDetailStyles } from "./MasterDetailLayout";
import { Composer } from "@/components/Composer";
import { TranscriptLoadingSkeleton } from "@/components/transcript/TranscriptLoadingSkeleton";
import { PrimarySurfaceNavigation, shellRootStyle } from "./AppShell";

export function AppBootSkeleton({ animateGlimmer = true }: { animateGlimmer?: boolean }) {
  const pathname = typeof window === "undefined" ? "/" : window.location.pathname;
  const route = routeFromPathname(pathname);
  const taskPage = route.kind === "tasks";
  const taskDetail = /^\/tasks\/[^/]+\/?$/.test(pathname) && pathname !== "/tasks/new";
  return (
    <main style={shellRootStyle()} {...stylex.props(styles.shellRoot)} aria-label="Loading Noema">
      <header {...stylex.props(styles.navbar, taskDetail && styles.taskDetailNavbar)}>
        <div aria-hidden="true" inert {...stylex.props(styles.headerOffset)}>
          <PrimarySurfaceNavigation
            route={taskPage ? route : { kind: "chat" }}
            agentName={null}
            agentAvatarActivity="idle"
            attention={null}
            onNavigate={() => undefined}
          />
        </div>
      </header>
      <section {...stylex.props(styles.deck, taskPage && styles.tasksDeck)} aria-label={taskPage ? "Tasks" : "Home"}>
        {taskPage ? <div {...stylex.props(masterDetailStyles.layout)}>
          <VStack aria-hidden="true" gap={4} className={stylex.props(masterDetailStyles.listPane, styles.taskList).className}>
            {[0, 1, 2, 3].map((index) => <HStack key={index} xstyle={[styles.taskRow, animateGlimmer && skeletonGlimmerStyles.animated]} />)}
          </VStack>
          {taskDetail ? <div {...stylex.props(masterDetailStyles.detailPane, styles.loadingTaskDetail)}><TaskLoadingSkeleton animateGlimmer={animateGlimmer} /></div> : null}
        </div> : <>
        <div {...stylex.props(styles.chatSurface)}>
          <div {...stylex.props(styles.contentLayer)}>
            <TranscriptLoadingSkeleton animateGlimmer={animateGlimmer} />
          </div>
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
        </>}
      </section>
    </main>
  );
}

const styles = stylex.create({
  taskDetailNavbar: { "@media (max-width: 979px)": { visibility: "hidden" } },
  tasksDeck: { left: { default: "var(--shell-sidebar-width)", "@media (max-width: 760px)": 0 } },
  taskList: { padding: "var(--spacing-4)" },
  taskRow: { width: "100%", height: "var(--spacing-12)", borderRadius: "var(--radius-element)", backgroundColor: "var(--skeleton-glimmer-line)" },
  loadingTaskDetail: { "@media (max-width: 979px)": { display: "block", position: "fixed", insetInline: 0, top: "var(--spacing-6)", bottom: 0, zIndex: 50, borderWidth: 0, borderStartStartRadius: "var(--radius-page)", borderStartEndRadius: "var(--radius-page)", overflow: "hidden", backgroundColor: "var(--noema-surface-card)" } },
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
    "--chat-opposite-avatar-gutter": {
      default: "96px",
      "@media (max-width: 760px)": "72px"
    },
    "--chat-composer-dock-height": {
      default: "96px",
      "@media (hover: none) and (pointer: coarse)": "92px"
    },
    "--chat-composer-scrim-height": "var(--spacing-12)",
    position: "relative",
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr)",
    gridTemplateRows: "minmax(0, 1fr)",
    minHeight: 0,
    height: "100%",
    width: "100%",
    overflow: "hidden"
  },
  contentLayer: {
    containerName: "chat-transcript",
    containerType: "inline-size",
    gridArea: "1 / 1",
    minHeight: 0,
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
    top: "calc(-1 * var(--chat-composer-scrim-height))",
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
