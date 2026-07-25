import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { ArrowDownIcon } from "lucide-react";
import { AnimatePresence, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { animateScrollToBottom } from "@/motion/scroll";
import { springs } from "@/motion/springs";
import { renderedEntryMessageId, type RenderTranscriptEntry } from "./renderModel";
import { BOTTOM_SCROLL_THRESHOLD_PX } from "./scrollModel";

const LOAD_BEFORE_THRESHOLD_PX = 320;

type ScrollToEndOptions = {
  behavior?: ScrollBehavior;
};

type TranscriptScrollerContextValue = {
  contentRef: React.RefObject<HTMLDivElement | null>;
  viewportRef: React.RefObject<HTMLDivElement | null>;
  scrollToEnd: (options?: ScrollToEndOptions) => void;
};

type TranscriptScrollerProviderProps = {
  children: React.ReactNode;
};

type TranscriptScrollerProps = {
  entries: RenderTranscriptEntry[];
  density?: "full" | "embedded";
  hasMoreBefore: boolean;
  loadingBefore: boolean;
  loadBeforeError: string | null;
  renderEntry: (entry: RenderTranscriptEntry, index: number) => React.ReactNode;
  onLoadBefore: () => void;
  "aria-label"?: string;
  onViewportScroll?: React.UIEventHandler<HTMLDivElement>;
};

type TranscriptScrollerItemProps = React.HTMLAttributes<HTMLDivElement> & {
  align?: "start" | "end";
  compact?: boolean;
  messageId: string;
  scrollAnchor?: boolean;
};

const TranscriptScrollerContext = React.createContext<TranscriptScrollerContextValue | null>(null);

const styles = stylex.create({
  root: {
    position: "relative",
    display: "flex",
    width: "100%",
    height: "100%",
    minHeight: 0,
    flexDirection: "column",
    overflow: "hidden"
  },
  rootEmbedded: {
    height: "100%"
  },
  viewport: {
    "--chat-transcript-top-fade": "calc(var(--shell-deck-header-height, 44px) + 56px)",
    width: "100%",
    height: "100%",
    minWidth: 0,
    minHeight: 0,
    overflowX: "hidden",
    overflowY: "auto",
    overscrollBehavior: "contain",
    maskImage:
      "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black calc(100% - var(--chat-transcript-bottom-fade, 128px)), transparent 100%)",
    WebkitMaskImage:
      "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black calc(100% - var(--chat-transcript-bottom-fade, 128px)), transparent 100%)"
  },
  viewportEmbedded: {
    "--chat-transcript-top-fade": "var(--spacing-8)",
    height: "100%",
    maxHeight: "none",
    maskImage: "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black 100%)",
    WebkitMaskImage: "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black 100%)"
  },
  content: {
    display: "flex",
    width: "var(--chat-column-width)",
    maxWidth: "100%",
    minWidth: 0,
    minHeight: "100%",
    boxSizing: "border-box",
    flexDirection: "column",
    marginInline: "auto",
    paddingTop: "var(--chat-transcript-top-fade)",
    paddingBottom: "max(80px, calc(var(--chat-composer-dock-height, 0px) + 16px))",
    paddingInline: 2
  },
  contentEmbedded: {
    width: "100%",
    maxWidth: "100%",
    minHeight: 0,
    marginInline: 0,
    paddingTop: "var(--spacing-8)",
    paddingBottom: "calc(var(--spacing-6) + var(--spacing-2) + var(--task-transcript-bottom-inset, 0px))",
    paddingInline: "var(--spacing-4)"
  },
  items: {
    width: "100%",
    marginTop: "auto"
  },
  loadBeforeStatus: {
    display: "flex",
    minHeight: 28,
    alignItems: "center",
    justifyContent: "center",
    color: "var(--noema-text-tertiary)",
    fontSize: 12,
    lineHeight: "16px"
  },
  loadBeforeButton: {
    borderWidth: 0,
    backgroundColor: "transparent",
    color: "var(--noema-text-secondary)",
    font: "inherit",
    paddingBlock: 4,
    paddingInline: 8,
    textDecorationLine: "underline",
    textUnderlineOffset: 3,
    ":hover": {
      color: "var(--noema-text-primary)"
    },
    ":focus-visible": {
      borderRadius: 6,
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)"
    }
  },
  item: {
    display: "flex",
    width: "100%",
    minWidth: 0,
    flexShrink: 0,
    marginTop: 12
  },
  itemEnd: {
    justifyContent: "flex-end"
  },
  compact: {
    marginTop: 4
  },
  scrollButton: {
    position: "absolute",
    left: "50%",
    bottom: "calc(var(--chat-composer-dock-height, 0px) + 10px)",
    zIndex: 3,
    display: "inline-flex",
    width: 32,
    height: 32,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 999,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-default)",
    backgroundColor: "var(--noema-surface-card)",
    color: "var(--noema-text-primary)",
    ":hover": {
      backgroundColor: "var(--noema-surface-sunken)"
    },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)"
    }
  },
  embeddedScrollButton: {
    bottom: "var(--spacing-4)"
  },
  srOnly: {
    position: "absolute",
    width: 1,
    height: 1,
    margin: -1,
    overflow: "hidden",
    clip: "rect(0 0 0 0)",
    whiteSpace: "nowrap",
    borderWidth: 0,
    padding: 0
  }
});

export function TranscriptScrollerProvider({ children }: TranscriptScrollerProviderProps) {
  const contentRef = React.useRef<HTMLDivElement | null>(null);
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const scrollToEnd = React.useCallback(({ behavior = "auto" }: ScrollToEndOptions = {}) => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    viewport.scrollTo({ top: viewport.scrollHeight, behavior });
  }, []);
  const value = React.useMemo(() => ({ contentRef, viewportRef, scrollToEnd }), [scrollToEnd]);

  return <TranscriptScrollerContext.Provider value={value}>{children}</TranscriptScrollerContext.Provider>;
}

export function useTranscriptScroller() {
  const context = React.useContext(TranscriptScrollerContext);
  if (!context) {
    throw new Error("useTranscriptScroller must be used within TranscriptScrollerProvider");
  }
  return context;
}

export function TranscriptScroller({
  entries,
  density = "full",
  hasMoreBefore,
  loadingBefore,
  loadBeforeError,
  renderEntry,
  onLoadBefore,
  onViewportScroll,
  "aria-label": ariaLabel
}: TranscriptScrollerProps) {
  const { contentRef, viewportRef, scrollToEnd } = useTranscriptScroller();
  const reduceMotion = useReducedMotion();
  const [stuckToBottom, setStuckToBottom] = React.useState(true);
  const oldestEntryKey = entries[0] ? renderedEntryMessageId(entries[0]) : null;
  const userScrolledTowardStartRef = React.useRef(false);
  const requestedOldestKeyRef = React.useRef<React.Key | null>(null);
  const autoFillOldestKeyRef = React.useRef<React.Key | null>(null);
  const previousOldestKeyRef = React.useRef(oldestEntryKey);
  const prependAnchorRef = React.useRef<PrependAnchor | null>(null);
  const touchStartYRef = React.useRef<number | null>(null);
  const userScrollAnimationRef = React.useRef<(() => void) | null>(null);
  const rememberPrependAnchor = React.useCallback(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (viewport && content) {
      prependAnchorRef.current = readPrependAnchor(viewport, content);
    }
  }, [contentRef, viewportRef]);
  const requestLoadBefore = React.useCallback(() => {
    rememberPrependAnchor();
    requestedOldestKeyRef.current = oldestEntryKey;
    onLoadBefore();
  }, [oldestEntryKey, onLoadBefore, rememberPrependAnchor]);
  const handleScroll = React.useCallback(
    (event: React.UIEvent<HTMLDivElement>) => {
      const viewport = event.currentTarget;
      const distance = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
      setStuckToBottom(distance < BOTTOM_SCROLL_THRESHOLD_PX);
      if (loadingBefore) {
        rememberPrependAnchor();
      } else if (
        viewport.scrollTop <= LOAD_BEFORE_THRESHOLD_PX &&
        userScrolledTowardStartRef.current &&
        hasMoreBefore &&
        !loadBeforeError &&
        oldestEntryKey !== null &&
        requestedOldestKeyRef.current !== oldestEntryKey
      ) {
        requestLoadBefore();
      }
      onViewportScroll?.(event);
    },
    [hasMoreBefore, loadBeforeError, loadingBefore, oldestEntryKey, onViewportScroll, rememberPrependAnchor, requestLoadBefore]
  );
  const markUserScrolledTowardStart = React.useCallback(() => {
    userScrolledTowardStartRef.current = true;
  }, []);
  const cancelUserScrollAnimation = React.useCallback(() => {
    userScrollAnimationRef.current?.();
    userScrollAnimationRef.current = null;
  }, []);
  const handleWheel = React.useCallback(
    (event: React.WheelEvent<HTMLDivElement>) => {
      cancelUserScrollAnimation();
      if (event.deltaY < 0) {
        markUserScrolledTowardStart();
      }
    },
    [cancelUserScrollAnimation, markUserScrolledTowardStart]
  );
  const handleKeyDown = React.useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      cancelUserScrollAnimation();
      if (event.key === "ArrowUp" || event.key === "PageUp" || event.key === "Home") {
        markUserScrolledTowardStart();
      }
    },
    [cancelUserScrollAnimation, markUserScrolledTowardStart]
  );
  const handleTouchStart = React.useCallback((event: React.TouchEvent<HTMLDivElement>) => {
    cancelUserScrollAnimation();
    touchStartYRef.current = event.touches[0]?.clientY ?? null;
  }, [cancelUserScrollAnimation]);
  const handleTouchMove = React.useCallback(
    (event: React.TouchEvent<HTMLDivElement>) => {
      const startY = touchStartYRef.current;
      const currentY = event.touches[0]?.clientY;
      if (startY !== null && currentY !== undefined && currentY > startY) {
        markUserScrolledTowardStart();
      }
    },
    [markUserScrolledTowardStart]
  );

  React.useEffect(() => cancelUserScrollAnimation, [cancelUserScrollAnimation]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || previousOldestKeyRef.current === oldestEntryKey) {
      return;
    }
    previousOldestKeyRef.current = oldestEntryKey;
    const anchor = prependAnchorRef.current;
    prependAnchorRef.current = null;
    if (anchor?.element.isConnected) {
      const nextTop = anchor.element.getBoundingClientRect().top - viewport.getBoundingClientRect().top;
      viewport.scrollTop += nextTop - anchor.top;
    }
  }, [oldestEntryKey, viewportRef]);

  React.useEffect(() => {
    if (!loadingBefore && loadBeforeError) {
      prependAnchorRef.current = null;
    }
  }, [loadBeforeError, loadingBefore]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || !hasMoreBefore || loadingBefore || loadBeforeError || oldestEntryKey === null) {
      return;
    }
    if (viewport.scrollHeight > viewport.clientHeight + 1 || autoFillOldestKeyRef.current === oldestEntryKey) {
      return;
    }
    autoFillOldestKeyRef.current = oldestEntryKey;
    requestLoadBefore();
  }, [entries.length, hasMoreBefore, loadBeforeError, loadingBefore, oldestEntryKey, requestLoadBefore, viewportRef]);

  return (
    <div {...stylex.props(styles.root, density === "embedded" && styles.rootEmbedded)}>
      <div
        ref={viewportRef}
        {...stylex.props(styles.viewport, density === "embedded" && styles.viewportEmbedded)}
        aria-label={ariaLabel}
        onKeyDown={handleKeyDown}
        onPointerDown={cancelUserScrollAnimation}
        onScroll={handleScroll}
        onTouchMove={handleTouchMove}
        onTouchStart={handleTouchStart}
        onWheel={handleWheel}
        role="region"
        tabIndex={0}
      >
        <div ref={contentRef} {...stylex.props(styles.content, density === "embedded" && styles.contentEmbedded)}>
          {hasMoreBefore || loadingBefore || loadBeforeError ? (
            <div {...stylex.props(styles.loadBeforeStatus)} role={loadBeforeError ? "alert" : "status"}>
              {loadBeforeError ? (
                <button type="button" {...stylex.props(styles.loadBeforeButton)} onClick={requestLoadBefore}>
                  Retry loading earlier messages
                </button>
              ) : loadingBefore ? (
                "Loading earlier messages"
              ) : (
                <button type="button" {...stylex.props(styles.loadBeforeButton)} onClick={requestLoadBefore}>
                  Load earlier messages
                </button>
              )}
            </div>
          ) : null}
          <div {...stylex.props(styles.items)}>
            {entries.map((entry, index) => renderEntry(entry, index))}
          </div>
        </div>
      </div>
      <AnimatePresence initial={false}>
        {!stuckToBottom ? (
          <m.button
            key="scroll-to-end"
            type="button"
            {...stylex.props(
              styles.scrollButton,
              density === "embedded" && styles.embeddedScrollButton
            )}
            data-active="true"
            initial={reduceMotion ? false : { opacity: 0, x: "-50%", y: 16, scale: 0.95 }}
            animate={{ opacity: 1, x: "-50%", y: 0, scale: 1 }}
            exit={{ opacity: 0, x: "-50%", y: 16, scale: 0.95 }}
            transition={reduceMotion ? { duration: 0 } : springs.micro}
            onClick={() => {
              cancelUserScrollAnimation();
              if (reduceMotion) {
                scrollToEnd({ behavior: "auto" });
                return;
              }
              const viewport = viewportRef.current;
              if (viewport) {
                userScrollAnimationRef.current = animateScrollToBottom(viewport, () => {
                  userScrollAnimationRef.current = null;
                });
              }
            }}
          >
            <ArrowDownIcon aria-hidden="true" size={16} />
            <span {...stylex.props(styles.srOnly)}>Scroll to end</span>
          </m.button>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

export function TranscriptScrollerItem({
  align = "start",
  compact = false,
  messageId,
  scrollAnchor = false,
  children,
  ...props
}: TranscriptScrollerItemProps) {
  return (
    <div
      {...props}
      {...stylex.props(styles.item, align === "end" && styles.itemEnd, compact && styles.compact)}
      data-message-id={messageId}
      data-scroll-anchor={scrollAnchor ? "true" : undefined}
      data-slot="message-scroller-item"
    >
      {children}
    </div>
  );
}

type PrependAnchor = {
  element: HTMLElement;
  top: number;
};

function readPrependAnchor(viewport: HTMLDivElement, content: HTMLDivElement): PrependAnchor | null {
  const viewportTop = viewport.getBoundingClientRect().top;
  const items = content.querySelectorAll<HTMLElement>('[data-slot="message-scroller-item"]');
  for (const element of items) {
    const rect = element.getBoundingClientRect();
    if (rect.bottom > viewportTop) {
      return { element, top: rect.top - viewportTop };
    }
  }
  return null;
}
