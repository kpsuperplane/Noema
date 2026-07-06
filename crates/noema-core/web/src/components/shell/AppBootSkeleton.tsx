import * as stylex from "@stylexjs/stylex";

const previewMessages = [
  { lane: "assistant", lines: [0.72, 0.48] },
  { lane: "human", lines: [0.54] },
  { lane: "assistant", lines: [0.84, 0.66, 0.36] }
] as const;

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
          <div {...stylex.props(styles.transcriptPreview)}>
            {previewMessages.map((message, index) =>
              renderSkeletonMessage(message.lane, message.lines, index)
            )}
          </div>
          <div {...stylex.props(styles.composerDock)}>
            <div data-slot="skeleton-glimmer" {...stylex.props(styles.composer)} />
          </div>
        </div>
      </section>
    </main>
  );
}

function renderSkeletonMessage(
  lane: "assistant" | "human",
  lines: readonly number[],
  rowIndex: number
) {
  return (
    <div
      key={`${lane}-${rowIndex}`}
      {...stylex.props(styles.messageRow, lane === "human" && styles.messageRowHuman)}
    >
      <div data-slot="skeleton-glimmer" {...stylex.props(styles.avatar)} />
      <div {...stylex.props(styles.messageStack, lane === "human" && styles.messageStackHuman)}>
        <div
          {...stylex.props(styles.messageBubble, lane === "human" && styles.messageBubbleHuman)}
          data-slot="skeleton-glimmer"
        >
          {lines.map((lineWidth, index) => (
            <div
              key={`${lane}-${rowIndex}-${index}`}
              data-slot="skeleton-glimmer"
              {...stylex.props(styles.messageLine)}
              style={{ width: `${Math.round(lineWidth * 100)}%` }}
            />
          ))}
        </div>
      </div>
    </div>
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
    position: "relative",
    display: "grid",
    minHeight: 0,
    overflow: "hidden"
  },
  transcriptPreview: {
    display: "grid",
    alignContent: "end",
    gap: 18,
    minHeight: 0,
    padding: "64px 24px 132px",
    overflow: "hidden",
    "@media (max-width: 760px)": {
      paddingInline: 20
    }
  },
  messageRow: {
    display: "flex",
    width: "min(760px, 100%)",
    alignItems: "flex-end",
    gap: 8,
    marginInline: "auto"
  },
  messageRowHuman: {
    flexDirection: "row-reverse"
  },
  avatar: {
    width: 32,
    height: 32,
    flexShrink: 0,
    borderRadius: "50%",
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  messageStack: {
    display: "flex",
    flex: 1,
    minWidth: 0,
    flexDirection: "column"
  },
  messageStackHuman: {
    alignItems: "flex-end"
  },
  messageBubble: {
    display: "grid",
    width: "min(420px, 82%)",
    minHeight: 42,
    gap: 8,
    borderRadius: 14,
    padding: "12px 14px",
    backgroundColor: "var(--skeleton-glimmer-base)"
  },
  messageBubbleHuman: {
    width: "min(330px, 72%)"
  },
  messageLine: {
    height: 10,
    borderRadius: 6,
    backgroundColor: "var(--skeleton-glimmer-line)"
  },
  composerDock: {
    position: "absolute",
    right: 0,
    bottom: 0,
    left: 0,
    display: "grid",
    justifyItems: "center",
    padding: "24px 24px 22px",
    background:
      "linear-gradient(to bottom, rgb(255 255 255 / 0), rgb(255 255 255 / 0.74) 42px, var(--background) 96px)"
  },
  composer: {
    width: "min(860px, calc(100% - 48px))",
    height: 56,
    borderRadius: 18,
    backgroundColor: "var(--skeleton-glimmer-base)",
    "@media (max-width: 760px)": {
      width: "calc(100% - 40px)"
    }
  }
});
