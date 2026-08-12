import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import {
  defaultRangeExtractor,
  useVirtualizer,
  type Range,
  type VirtualItem,
  type Virtualizer
} from "@tanstack/react-virtual";
import { ArrowDownIcon } from "lucide-react";
import { AnimatePresence, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { animateScrollToBottom } from "@/motion/scroll";
import { springs } from "@/motion/springs";
import { renderedEntryMessageId, type RenderTranscriptEntry } from "./renderModel";
import { BOTTOM_SCROLL_THRESHOLD_PX } from "./scrollModel";

const TRANSCRIPT_SCROLL_DEBUG_PARAM = "debugTranscriptScroll";
const TRANSCRIPT_SCROLL_LOG_PREFIX = "[transcript-scroll]";
const TRANSCRIPT_SCROLL_SETTLE_MS = 180;
const TRANSCRIPT_SCROLL_DEBUG_ENABLED =
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get(TRANSCRIPT_SCROLL_DEBUG_PARAM) === "1";

type ScrollToEndOptions = {
  behavior?: ScrollBehavior;
};

type TranscriptScrollerContextValue = {
  bottomOffsetRef: React.MutableRefObject<number>;
  contentRef: React.RefObject<HTMLDivElement | null>;
  viewportRef: React.RefObject<HTMLDivElement | null>;
  virtualSizerRef: React.RefObject<HTMLDivElement | null>;
  scrollToEnd: (options?: ScrollToEndOptions) => void;
};

type TranscriptScrollerProviderProps = {
  children: React.ReactNode;
};

type TranscriptScrollerProps = {
  entries: RenderTranscriptEntry[];
  density?: "full" | "embedded";
  hasMoreBefore: boolean;
  historyPrependDeferred?: boolean;
  loadingBefore: boolean;
  loadBeforeError: string | null;
  busy?: boolean;
  completionAnnouncementKey?: number;
  renderEntry: (entry: RenderTranscriptEntry, index: number) => React.ReactNode;
  onLoadBefore: () => void;
  onScrollActivityChange?: (active: boolean) => void;
  "aria-label"?: string;
  onViewportScroll?: React.UIEventHandler<HTMLDivElement>;
};

type TranscriptScrollerItemProps = React.HTMLAttributes<HTMLDivElement> & {
  align?: "start" | "end";
  compact?: boolean;
  "data-arrival"?: "true";
  messageId: string;
  scrollAnchor?: boolean;
};

const TranscriptScrollerContext = React.createContext<TranscriptScrollerContextValue | null>(null);

const styles = stylex.create({
  root: {
    "--chat-transcript-top-fade": "calc(var(--shell-deck-header-height, 44px) + 56px)",
    position: "relative",
    display: "flex",
    width: "100%",
    height: "100%",
    minHeight: 0,
    flexDirection: "column",
    overflow: "hidden"
  },
  rootEmbedded: {
    "--chat-transcript-top-fade": "var(--spacing-8)",
    height: "100%"
  },
  viewport: {
    width: "100%",
    height: "100%",
    minWidth: 0,
    minHeight: 0,
    overflowAnchor: "none",
    overflowX: "hidden",
    overflowY: "auto",
    overscrollBehavior: "contain"
  },
  viewportEmbedded: {
    height: "100%",
    maxHeight: "none"
  },
  content: {
    width: "var(--chat-column-width)",
    maxWidth: "100%",
    minWidth: 0,
    minHeight: "100%",
    marginInline: "auto",
    paddingTop: "var(--chat-transcript-top-fade)",
    paddingBottom:
      "max(80px, calc(var(--chat-composer-dock-height, 0px) + var(--chat-composer-scrim-height, 48px)))",
    paddingInline: "var(--spacing-0-5)"
  },
  contentEmbedded: {
    width: "100%",
    maxWidth: "100%",
    minHeight: 0,
    marginInline: "var(--spacing-0)",
    paddingTop: "var(--spacing-8)",
    paddingBottom: "calc(var(--spacing-6) + var(--spacing-2) + var(--task-transcript-bottom-inset, 0px))",
    paddingInline: "var(--spacing-4)"
  },
  virtualSizer: {
    position: "relative",
    width: "100%",
    minHeight: 0
  },
  virtualRow: {
    position: "absolute",
    top: 0,
    left: 0,
    width: "100%"
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
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)",
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
    marginTop: "var(--spacing-3)"
  },
  itemEnd: {
    justifyContent: "flex-end"
  },
  compact: {
    marginTop: "var(--spacing-1)"
  },
  arriving: {
    contentVisibility: "visible",
    position: "relative",
    zIndex: 1
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
    cornerShape: "var(--corner-shape-full)",
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
  topFade: {
    position: "absolute",
    top: 0,
    right: "var(--spacing-4)",
    left: 0,
    zIndex: 2,
    height: "var(--chat-transcript-top-fade)",
    pointerEvents: "none",
    backgroundImage: "linear-gradient(to bottom, var(--background), rgb(255 255 255 / 0))"
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
    padding: "var(--spacing-0)"
  }
});

export function TranscriptScrollerProvider({ children }: TranscriptScrollerProviderProps) {
  const bottomOffsetRef = React.useRef(0);
  const contentRef = React.useRef<HTMLDivElement | null>(null);
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const virtualSizerRef = React.useRef<HTMLDivElement | null>(null);
  const scrollToEnd = React.useCallback(({ behavior = "auto" }: ScrollToEndOptions = {}) => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    viewport.scrollTo({ top: viewport.scrollHeight, behavior });
  }, []);
  const value = React.useMemo(
    () => ({ bottomOffsetRef, contentRef, viewportRef, virtualSizerRef, scrollToEnd }),
    [scrollToEnd]
  );

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
  historyPrependDeferred = false,
  loadingBefore,
  loadBeforeError,
  busy = false,
  completionAnnouncementKey = 0,
  renderEntry,
  onLoadBefore,
  onScrollActivityChange,
  onViewportScroll,
  "aria-label": ariaLabel
}: TranscriptScrollerProps) {
  const { bottomOffsetRef, contentRef, viewportRef, virtualSizerRef, scrollToEnd } = useTranscriptScroller();
  const reduceMotion = useReducedMotion();
  const [stuckToBottom, setStuckToBottom] = React.useState(true);
  const [userScrolledTowardStart, setUserScrolledTowardStart] = React.useState(false);
  const [availableHeight, setAvailableHeight] = React.useState(0);
  const [scrollMargin, setScrollMargin] = React.useState(0);
  const [measuringInitialPage, setMeasuringInitialPage] = React.useState(true);
  const [settlingPrepend, setSettlingPrepend] = React.useState(false);
  const previousToolGroupKeysRef = React.useRef<ReadonlyMap<string, React.Key>>(new Map());
  const virtualItemKeysRef = React.useRef<readonly React.Key[]>([]);
  const loadBeforeStatusRef = React.useRef<HTMLDivElement | null>(null);
  const nearTopLoadArmedRef = React.useRef(true);
  const requestedOldestKeyRef = React.useRef<React.Key | null>(null);
  const loadBeforeStartedRef = React.useRef(false);
  const autoFillOldestKeyRef = React.useRef<React.Key | null>(null);
  const debugScrollMetricsRef = React.useRef<{ scrollHeight: number; scrollTop: number } | null>(null);
  const touchStartYRef = React.useRef<number | null>(null);
  const touchActiveRef = React.useRef(false);
  const scrollActiveRef = React.useRef(false);
  const scrollSettleTimerRef = React.useRef<number | null>(null);
  const userScrollAnimationRef = React.useRef<(() => void) | null>(null);
  const virtualItemKeys = reconcileVirtualItemKeys(entries, previousToolGroupKeysRef.current);
  virtualItemKeysRef.current = virtualItemKeys.keys;
  const oldestEntryKey = entries[0] ? renderedEntryMessageId(entries[0]) : null;
  const measuringPrependedPage =
    settlingPrepend &&
    requestedOldestKeyRef.current !== null &&
    requestedOldestKeyRef.current !== oldestEntryKey;
  const measuringLoadedPage = measuringInitialPage || measuringPrependedPage;
  const getItemKey = React.useCallback(
    (index: number) => virtualItemKeysRef.current[index],
    []
  );
  const extractVirtualRange = React.useCallback((range: Range) => {
    const indexes = defaultRangeExtractor(range);
    if (!measuringLoadedPage) {
      return indexes;
    }
    const lastIndex = indexes.at(-1) ?? range.endIndex;
    return Array.from({ length: lastIndex + 1 }, (_, index) => index);
  }, [measuringLoadedPage]);
  const syncVirtualLayout = React.useCallback(
    (instance: Virtualizer<HTMLDivElement, HTMLDivElement>) => {
      const totalSize = instance.getTotalSize();
      const bottomOffset = Math.max(0, availableHeight - totalSize);
      const previousBottomOffset = bottomOffsetRef.current;
      bottomOffsetRef.current = bottomOffset;
      if (bottomOffset === 0 && previousBottomOffset === 0) {
        return;
      }
      const sizer = virtualSizerRef.current;
      if (sizer) {
        sizer.style.height = `${Math.max(totalSize, availableHeight)}px`;
      }
      for (const item of instance.getVirtualItems()) {
        const element = instance.elementsCache.get(item.key);
        if (element) {
          element.style.transform = `translate3d(0, ${item.start - scrollMargin + bottomOffset}px, 0)`;
        }
      }
    },
    [availableHeight, bottomOffsetRef, scrollMargin, virtualSizerRef]
  );
  // TanStack Virtual exposes imperative measurement functions that React Compiler cannot memoize.
  // eslint-disable-next-line react-hooks/incompatible-library
  const rowVirtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: entries.length,
    directDomUpdates: true,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => 96,
    anchorTo: "end",
    followOnAppend: false,
    // TranscriptBottomFollower owns actual-DOM bottom following; disable the virtualizer's inset-blind resize pin.
    scrollEndThreshold: -1,
    scrollMargin,
    overscan: 12,
    rangeExtractor: extractVirtualRange,
    useAnimationFrameWithResizeObserver: false,
    onChange: syncVirtualLayout,
    getItemKey
  });
  rowVirtualizer.shouldAdjustScrollPositionOnItemSizeChange = preserveVisibleAnchorOnRowResize;
  const virtualItems = rowVirtualizer.getVirtualItems();
  const totalSize = rowVirtualizer.getTotalSize();
  const firstVirtualIndex = virtualItems[0]?.index ?? null;
  const lastVirtualIndex = virtualItems.at(-1)?.index ?? null;
  const setVirtualSizer = React.useCallback(
    (node: HTMLDivElement | null) => {
      virtualSizerRef.current = node;
      rowVirtualizer.containerRef(node);
    },
    [rowVirtualizer, virtualSizerRef]
  );
  const requestLoadBefore = React.useCallback(() => {
    logTranscriptScroll("load-before", {});
    requestedOldestKeyRef.current = oldestEntryKey;
    loadBeforeStartedRef.current = false;
    setSettlingPrepend(true);
    onLoadBefore();
  }, [oldestEntryKey, onLoadBefore]);

  React.useLayoutEffect(() => {
    previousToolGroupKeysRef.current = virtualItemKeys.toolGroupKeys;
  }, [virtualItemKeys.toolGroupKeys]);
  const setScrollActivity = React.useCallback((active: boolean) => {
    if (scrollActiveRef.current === active) {
      return;
    }
    scrollActiveRef.current = active;
    logTranscriptScroll(active ? "scroll-active" : "scroll-settled", {});
    onScrollActivityChange?.(active);
  }, [onScrollActivityChange]);
  const clearScrollSettleTimer = React.useCallback(() => {
    if (scrollSettleTimerRef.current !== null) {
      window.clearTimeout(scrollSettleTimerRef.current);
      scrollSettleTimerRef.current = null;
    }
  }, []);
  const scheduleScrollSettled = React.useCallback(() => {
    clearScrollSettleTimer();
    scrollSettleTimerRef.current = window.setTimeout(() => {
      scrollSettleTimerRef.current = null;
      if (!touchActiveRef.current) {
        setScrollActivity(false);
      }
    }, TRANSCRIPT_SCROLL_SETTLE_MS);
  }, [clearScrollSettleTimer, setScrollActivity]);
  const markScrollActive = React.useCallback(() => {
    setScrollActivity(true);
    scheduleScrollSettled();
  }, [scheduleScrollSettled, setScrollActivity]);
  const handleScroll = React.useCallback(
    (event: React.UIEvent<HTMLDivElement>) => {
      const viewport = event.currentTarget;
      markScrollActive();
      if (TRANSCRIPT_SCROLL_DEBUG_ENABLED) {
        const previousMetrics = debugScrollMetricsRef.current;
        logTranscriptScroll("scroll", {
          scrollHeight: viewport.scrollHeight,
          scrollHeightDelta: previousMetrics ? viewport.scrollHeight - previousMetrics.scrollHeight : 0,
          scrollTop: roundScrollMetric(viewport.scrollTop),
          scrollTopDelta: previousMetrics ? roundScrollMetric(viewport.scrollTop - previousMetrics.scrollTop) : 0,
          trusted: event.nativeEvent.isTrusted
        });
        debugScrollMetricsRef.current = {
          scrollHeight: viewport.scrollHeight,
          scrollTop: viewport.scrollTop
        };
      }
      const distance = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
      setStuckToBottom(distance < BOTTOM_SCROLL_THRESHOLD_PX);
      onViewportScroll?.(event);
    },
    [markScrollActive, onViewportScroll]
  );
  const markUserScrolledTowardStart = React.useCallback(() => {
    setUserScrolledTowardStart(true);
  }, []);
  const cancelUserScrollAnimation = React.useCallback(() => {
    userScrollAnimationRef.current?.();
    userScrollAnimationRef.current = null;
  }, []);
  const handleWheel = React.useCallback(
    (event: React.WheelEvent<HTMLDivElement>) => {
      cancelUserScrollAnimation();
      markScrollActive();
      if (TRANSCRIPT_SCROLL_DEBUG_ENABLED) {
        logTranscriptScroll("wheel", {
          deltaY: roundScrollMetric(event.deltaY),
          scrollHeight: event.currentTarget.scrollHeight,
          scrollTop: roundScrollMetric(event.currentTarget.scrollTop),
          trusted: event.nativeEvent.isTrusted
        });
      }
      if (event.deltaY < 0) {
        markUserScrolledTowardStart();
      }
    },
    [cancelUserScrollAnimation, markScrollActive, markUserScrolledTowardStart]
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
    clearScrollSettleTimer();
    touchActiveRef.current = true;
    setScrollActivity(true);
    touchStartYRef.current = event.touches[0]?.clientY ?? null;
  }, [cancelUserScrollAnimation, clearScrollSettleTimer, setScrollActivity]);
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
  const handleTouchEnd = React.useCallback(() => {
    touchActiveRef.current = false;
    touchStartYRef.current = null;
    scheduleScrollSettled();
  }, [scheduleScrollSettled]);

  React.useEffect(() => () => {
    cancelUserScrollAnimation();
    clearScrollSettleTimer();
  }, [cancelUserScrollAnimation, clearScrollSettleTimer]);

  React.useEffect(() => {
    if (historyPrependDeferred) {
      logTranscriptScroll("prepend-deferred", {});
    }
  }, [historyPrependDeferred]);

  React.useEffect(() => {
    logTranscriptScroll("virtual-range", {
      firstIndex: firstVirtualIndex,
      lastIndex: lastVirtualIndex,
      renderedRows: virtualItems.length,
      totalSize: roundScrollMetric(totalSize)
    });
  }, [firstVirtualIndex, lastVirtualIndex, totalSize, virtualItems.length]);

  React.useEffect(() => {
    if (!measuringInitialPage || entries.length === 0) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      setMeasuringInitialPage(false);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [entries.length, measuringInitialPage]);

  React.useEffect(() => {
    if (!settlingPrepend) {
      return;
    }
    if (loadingBefore) {
      loadBeforeStartedRef.current = true;
    }
    const requestedOldestKey = requestedOldestKeyRef.current;
    const pageSettled = requestedOldestKey !== null && requestedOldestKey !== oldestEntryKey;
    const pageEnded = loadBeforeStartedRef.current && !loadingBefore && !historyPrependDeferred;
    if (!pageSettled && !pageEnded) {
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      loadBeforeStartedRef.current = false;
      setSettlingPrepend(false);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [historyPrependDeferred, loadingBefore, oldestEntryKey, settlingPrepend]);

  React.useLayoutEffect(() => {
    syncVirtualLayout(rowVirtualizer);
  }, [rowVirtualizer, syncVirtualLayout]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (!viewport || !content) {
      return;
    }

    const syncAvailableHeight = () => {
      const computedStyle = window.getComputedStyle(content);
      const paddingTop = cssPixels(computedStyle.paddingTop);
      const paddingBottom = cssPixels(computedStyle.paddingBottom);
      const loadBeforeHeight = loadBeforeStatusRef.current?.getBoundingClientRect().height ?? 0;
      const nextScrollMargin = paddingTop + loadBeforeHeight;
      const nextAvailableHeight = Math.max(0, viewport.clientHeight - paddingTop - paddingBottom - loadBeforeHeight);
      setScrollMargin((currentMargin) => currentMargin === nextScrollMargin ? currentMargin : nextScrollMargin);
      setAvailableHeight((currentHeight) =>
        currentHeight === nextAvailableHeight ? currentHeight : nextAvailableHeight
      );
    };

    syncAvailableHeight();

    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", syncAvailableHeight);
      return () => window.removeEventListener("resize", syncAvailableHeight);
    }

    const observer = new ResizeObserver(syncAvailableHeight);
    observer.observe(viewport);
    observer.observe(content);
    if (loadBeforeStatusRef.current) {
      observer.observe(loadBeforeStatusRef.current);
    }
    return () => observer.disconnect();
  }, [contentRef, viewportRef]);

  React.useEffect(() => {
    const first = virtualItems[0];
    if (!first || !hasMoreBefore || loadingBefore || loadBeforeError || !oldestEntryKey) {
      return;
    }
    if (first.index > 3) {
      nearTopLoadArmedRef.current = true;
      return;
    }
    if (
      !nearTopLoadArmedRef.current ||
      !userScrolledTowardStart ||
      requestedOldestKeyRef.current === oldestEntryKey
    ) {
      return;
    }
    nearTopLoadArmedRef.current = false;
    requestedOldestKeyRef.current = oldestEntryKey;
    requestLoadBefore();
  }, [hasMoreBefore, loadBeforeError, loadingBefore, oldestEntryKey, requestLoadBefore, userScrolledTowardStart, virtualItems]);

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
  }, [availableHeight, entries.length, hasMoreBefore, loadBeforeError, loadingBefore, oldestEntryKey, requestLoadBefore, viewportRef]);

  return (
    <div {...stylex.props(styles.root, density === "embedded" && styles.rootEmbedded)}>
      <div aria-hidden="true" {...stylex.props(styles.topFade)} />
      <div
        ref={viewportRef}
        {...stylex.props(styles.viewport, density === "embedded" && styles.viewportEmbedded)}
        aria-label={ariaLabel}
        onKeyDown={handleKeyDown}
        onPointerDown={cancelUserScrollAnimation}
        onScroll={handleScroll}
        onTouchCancel={handleTouchEnd}
        onTouchEnd={handleTouchEnd}
        onTouchMove={handleTouchMove}
        onTouchStart={handleTouchStart}
        onWheel={handleWheel}
        role="log"
        aria-live="off"
        aria-busy={busy}
        aria-relevant="additions"
        tabIndex={0}
      >
        <div ref={contentRef} {...stylex.props(styles.content, density === "embedded" && styles.contentEmbedded)}>
          <div
            ref={loadBeforeStatusRef}
            {...stylex.props(styles.loadBeforeStatus)}
            role={loadBeforeError ? "alert" : hasMoreBefore || loadingBefore ? "status" : undefined}
          >
            {loadBeforeError ? (
              <button type="button" {...stylex.props(styles.loadBeforeButton)} onClick={requestLoadBefore}>
                Retry loading earlier messages
              </button>
            ) : loadingBefore ? (
              "Loading earlier messages"
            ) : hasMoreBefore ? (
              <button type="button" {...stylex.props(styles.loadBeforeButton)} onClick={requestLoadBefore}>
                Load earlier messages
              </button>
            ) : null}
          </div>
          <div
            ref={setVirtualSizer}
            {...stylex.props(styles.virtualSizer)}
          >
            {virtualItems.map((virtualItem) => {
              const entry = entries[virtualItem.index];
              return (
                <div
                  key={virtualItem.key}
                  ref={rowVirtualizer.measureElement}
                  data-index={virtualItem.index}
                  {...stylex.props(styles.virtualRow)}
                >
                  {renderEntry(entry, virtualItem.index)}
                </div>
              );
            })}
          </div>
        </div>
      </div>
      <span aria-live="polite" aria-atomic="true" {...stylex.props(styles.srOnly)}>
        <span key={completionAnnouncementKey}>
          {completionAnnouncementKey > 0 ? "Noema replied" : ""}
        </span>
      </span>
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
                userScrollAnimationRef.current = animateScrollToBottom(viewport, {
                  onComplete: () => {
                    userScrollAnimationRef.current = null;
                  }
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

function reconcileVirtualItemKeys(
  entries: readonly RenderTranscriptEntry[],
  previousToolGroupKeys: ReadonlyMap<string, React.Key>
) {
  const toolGroupKeys = new Map<string, React.Key>();
  const usedKeys = new Set<React.Key>();
  const keys = entries.map((entry) => {
    if (entry.kind !== "tool_marker_group") {
      return renderedEntryMessageId(entry);
    }

    const previousKey = entry.markers
      .map((marker) => previousToolGroupKeys.get(marker.id))
      .find((key) => key !== undefined && !usedKeys.has(key));
    const key = previousKey ?? `tool-marker-group:${entry.markers[0]?.id ?? entry.id}`;
    usedKeys.add(key);
    for (const marker of entry.markers) {
      toolGroupKeys.set(marker.id, key);
    }
    return key;
  });

  return { keys, toolGroupKeys };
}

function preserveVisibleAnchorOnRowResize(
  item: VirtualItem,
  delta: number,
  instance: Virtualizer<HTMLDivElement, HTMLDivElement>
) {
  // Keep backward scrolling anchored while direct DOM updates settle row positions in the same frame.
  const scrollOffset = instance.scrollOffset ?? 0;
  const shouldAdjust = item.start < scrollOffset;
  logTranscriptScroll("row-resize", {
    delta: roundScrollMetric(delta),
    direction: instance.scrollDirection,
    index: item.index,
    itemEnd: roundScrollMetric(item.end),
    itemStart: roundScrollMetric(item.start),
    scrollOffset: roundScrollMetric(scrollOffset),
    shouldAdjust
  });
  return shouldAdjust;
}

function logTranscriptScroll(event: string, details: Record<string, unknown>) {
  if (!TRANSCRIPT_SCROLL_DEBUG_ENABLED) {
    return;
  }
  console.info(
    `${TRANSCRIPT_SCROLL_LOG_PREFIX} ${JSON.stringify({
      event,
      time: roundScrollMetric(window.performance.now()),
      ...details
    })}`
  );
}

function roundScrollMetric(value: number) {
  return Math.round(value * 100) / 100;
}

function cssPixels(value: string) {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function TranscriptScrollerItem({
  align = "start",
  compact = false,
  "data-arrival": arrival,
  messageId,
  scrollAnchor = false,
  children,
  ...props
}: TranscriptScrollerItemProps) {
  return (
    <div
      {...props}
      {...stylex.props(
        styles.item,
        align === "end" && styles.itemEnd,
        compact && styles.compact,
        arrival === "true" && styles.arriving
      )}
      data-arrival={arrival}
      data-message-id={messageId}
      data-scroll-anchor={scrollAnchor ? "true" : undefined}
      data-slot="message-scroller-item"
    >
      {children}
    </div>
  );
}
