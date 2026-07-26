import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import {
  useVirtualizer,
  useWindowVirtualizer,
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
const TRANSCRIPT_SCROLL_DEBUG_ENABLED =
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get(TRANSCRIPT_SCROLL_DEBUG_PARAM) === "1";

type ScrollToEndOptions = { behavior?: ScrollBehavior };

type TranscriptScrollerContextValue = {
  contentRef: React.RefObject<HTMLDivElement | null>;
  viewportRef: React.RefObject<HTMLDivElement | null>;
  getScrollElement: () => HTMLElement | null;
  scrollMode: TranscriptScrollMode;
  scrollToEnd: (options?: ScrollToEndOptions) => void;
};

type TranscriptScrollerProviderProps = {
  children: React.ReactNode;
  scrollMode: TranscriptScrollMode;
};

export type TranscriptScrollMode = "document" | "element";
type TranscriptVirtualizer = Virtualizer<Window | HTMLDivElement, HTMLDivElement>;

type TranscriptScrollerProps = {
  entries: RenderTranscriptEntry[];
  density?: "full" | "embedded";
  hasMoreBefore: boolean;
  loadingBefore: boolean;
  loadBeforeError: string | null;
  renderEntry: (entry: RenderTranscriptEntry, index: number) => React.ReactNode;
  onLoadBefore: () => void;
  "aria-label"?: string;
  onViewportScroll?: (viewport: HTMLElement) => void;
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
    minHeight: 0,
    flexDirection: "column"
  },
  rootEmbedded: {
    height: "100%",
    overflow: "hidden"
  },
  rootDocument: {
    minHeight: "calc(100svh - 52px)",
    height: "auto",
    overflow: "visible"
  },
  viewport: {
    "--chat-transcript-top-fade": "calc(var(--shell-deck-header-height, 44px) + 56px)",
    width: "100%",
    height: "100%",
    minWidth: 0,
    minHeight: 0,
    overflowAnchor: "none"
  },
  viewportEmbedded: {
    "--chat-transcript-top-fade": "var(--spacing-8)",
    height: "100%",
    maxHeight: "none",
    overflowX: "hidden",
    overflowY: "auto",
    overscrollBehavior: "contain",
    maskImage: "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black 100%)",
    WebkitMaskImage: "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black 100%)"
  },
  viewportDocument: {
    height: "auto",
    minHeight: "calc(100svh - 52px)",
    overflowX: "visible",
    overflowY: "visible",
    overscrollBehavior: "auto",
    maskImage: "none",
    WebkitMaskImage: "none"
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
  documentScrollButton: {
    position: "fixed"
  },
  documentTopFade: {
    position: "sticky",
    top: "calc(52px + var(--shell-chrome-viewport-top, 0px))",
    zIndex: 2,
    width: "100%",
    height: 56,
    flexShrink: 0,
    marginBottom: -56,
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
    padding: 0
  }
});

export function TranscriptScrollerProvider({ children, scrollMode }: TranscriptScrollerProviderProps) {
  const contentRef = React.useRef<HTMLDivElement | null>(null);
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const getScrollElement = React.useCallback(() => {
    if (scrollMode === "document") {
      return document.scrollingElement as HTMLElement | null;
    }
    return viewportRef.current;
  }, [scrollMode]);
  const scrollToEnd = React.useCallback(({ behavior = "auto" }: ScrollToEndOptions = {}) => {
    const viewport = getScrollElement();
    if (!viewport) {
      return;
    }
    if (scrollMode === "document") {
      window.scrollTo({ top: viewport.scrollHeight, behavior });
      return;
    }
    viewport.scrollTo({ top: viewport.scrollHeight, behavior });
  }, [getScrollElement, scrollMode]);
  const value = React.useMemo(
    () => ({ contentRef, getScrollElement, scrollMode, scrollToEnd, viewportRef }),
    [getScrollElement, scrollMode, scrollToEnd]
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
  loadingBefore,
  loadBeforeError,
  renderEntry,
  onLoadBefore,
  onViewportScroll,
  "aria-label": ariaLabel
}: TranscriptScrollerProps) {
  const { contentRef, getScrollElement, scrollMode, scrollToEnd, viewportRef } = useTranscriptScroller();
  const documentMode = scrollMode === "document";
  const reduceMotion = useReducedMotion();
  const [stuckToBottom, setStuckToBottom] = React.useState(true);
  const [userScrolledTowardStart, setUserScrolledTowardStart] = React.useState(false);
  const [availableHeight, setAvailableHeight] = React.useState(0);
  const [scrollMargin, setScrollMargin] = React.useState(0);
  const previousToolGroupKeysRef = React.useRef<ReadonlyMap<string, React.Key>>(new Map());
  const virtualItemKeysRef = React.useRef<readonly React.Key[]>([]);
  const virtualSizerRef = React.useRef<HTMLDivElement | null>(null);
  const virtualBottomOffsetRef = React.useRef(0);
  const loadBeforeStatusRef = React.useRef<HTMLDivElement | null>(null);
  const nearTopLoadArmedRef = React.useRef(true);
  const requestedOldestKeyRef = React.useRef<React.Key | null>(null);
  const autoFillOldestKeyRef = React.useRef<React.Key | null>(null);
  const debugScrollMetricsRef = React.useRef<{ scrollHeight: number; scrollTop: number } | null>(null);
  const touchStartYRef = React.useRef<number | null>(null);
  const userScrollAnimationRef = React.useRef<(() => void) | null>(null);
  const virtualItemKeys = reconcileVirtualItemKeys(entries, previousToolGroupKeysRef.current);
  virtualItemKeysRef.current = virtualItemKeys.keys;
  const getItemKey = React.useCallback(
    (index: number) => virtualItemKeysRef.current[index],
    []
  );
  const syncVirtualLayout = React.useCallback(
    (instance: TranscriptVirtualizer) => {
      const totalSize = instance.getTotalSize();
      const bottomOffset = Math.max(0, availableHeight - totalSize);
      virtualBottomOffsetRef.current = bottomOffset;
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
    [availableHeight, scrollMargin]
  );
  // TanStack Virtual exposes imperative measurement functions that React Compiler cannot memoize.
  // eslint-disable-next-line react-hooks/incompatible-library
  const elementVirtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    enabled: !documentMode,
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
    useAnimationFrameWithResizeObserver: true,
    onChange: (instance) => syncVirtualLayout(instance as TranscriptVirtualizer),
    getItemKey
  });
  const documentVirtualizer = useWindowVirtualizer<HTMLDivElement>({
    enabled: documentMode,
    count: entries.length,
    directDomUpdates: true,
    estimateSize: () => 96,
    anchorTo: "end",
    followOnAppend: false,
    scrollEndThreshold: -1,
    scrollMargin,
    overscan: 12,
    useAnimationFrameWithResizeObserver: true,
    onChange: (instance) => syncVirtualLayout(instance as TranscriptVirtualizer),
    getItemKey
  });
  const rowVirtualizer = (
    documentMode ? documentVirtualizer : elementVirtualizer
  ) as TranscriptVirtualizer;
  rowVirtualizer.shouldAdjustScrollPositionOnItemSizeChange = preserveVisibleAnchorOnRowResize;
  const virtualItems = rowVirtualizer.getVirtualItems();
  const totalSize = rowVirtualizer.getTotalSize();
  const firstVirtualIndex = virtualItems[0]?.index ?? null;
  const lastVirtualIndex = virtualItems.at(-1)?.index ?? null;
  const setVirtualSizer = React.useCallback(
    (node: HTMLDivElement | null) => {
      virtualSizerRef.current = node;
      if (!documentMode) {
        elementVirtualizer.containerRef(node);
      }
    },
    [documentMode, elementVirtualizer]
  );
  const oldestEntryKey = entries[0] ? renderedEntryMessageId(entries[0]) : null;
  const requestLoadBefore = React.useCallback(() => {
    logTranscriptScroll("load-before", {});
    requestedOldestKeyRef.current = oldestEntryKey;
    onLoadBefore();
  }, [oldestEntryKey, onLoadBefore]);

  React.useLayoutEffect(() => {
    previousToolGroupKeysRef.current = virtualItemKeys.toolGroupKeys;
  }, [virtualItemKeys.toolGroupKeys]);
  const syncScrollState = React.useCallback(
    (viewport: HTMLElement, trusted: boolean) => {
      if (TRANSCRIPT_SCROLL_DEBUG_ENABLED) {
        const previousMetrics = debugScrollMetricsRef.current;
        logTranscriptScroll("scroll", {
          scrollHeight: viewport.scrollHeight,
          scrollHeightDelta: previousMetrics ? viewport.scrollHeight - previousMetrics.scrollHeight : 0,
          scrollTop: roundScrollMetric(viewport.scrollTop),
          scrollTopDelta: previousMetrics ? roundScrollMetric(viewport.scrollTop - previousMetrics.scrollTop) : 0,
          trusted
        });
        debugScrollMetricsRef.current = {
          scrollHeight: viewport.scrollHeight,
          scrollTop: viewport.scrollTop
        };
      }
      const distance = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
      setStuckToBottom(distance < BOTTOM_SCROLL_THRESHOLD_PX);
      onViewportScroll?.(viewport);
    },
    [onViewportScroll]
  );
  const handleElementScroll = React.useCallback(
    (event: React.UIEvent<HTMLDivElement>) => {
      syncScrollState(event.currentTarget, event.nativeEvent.isTrusted);
    },
    [syncScrollState]
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
      const viewport = getScrollElement();
      if (TRANSCRIPT_SCROLL_DEBUG_ENABLED) {
        logTranscriptScroll("wheel", {
          deltaY: roundScrollMetric(event.deltaY),
          scrollHeight: viewport?.scrollHeight,
          scrollTop: roundScrollMetric(viewport?.scrollTop ?? 0),
          trusted: event.nativeEvent.isTrusted
        });
      }
      if (event.deltaY < 0) {
        markUserScrolledTowardStart();
      }
    },
    [cancelUserScrollAnimation, getScrollElement, markUserScrolledTowardStart]
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

  React.useEffect(() => {
    if (!documentMode) {
      return;
    }
    const handleDocumentScroll = (event: Event) => {
      const viewport = getScrollElement();
      if (viewport) {
        syncScrollState(viewport, event.isTrusted);
      }
    };
    window.addEventListener("scroll", handleDocumentScroll, { passive: true });
    return () => window.removeEventListener("scroll", handleDocumentScroll);
  }, [documentMode, getScrollElement, syncScrollState]);

  React.useEffect(() => {
    logTranscriptScroll("virtual-range", {
      firstIndex: firstVirtualIndex,
      lastIndex: lastVirtualIndex,
      renderedRows: virtualItems.length,
      totalSize: roundScrollMetric(totalSize)
    });
  }, [firstVirtualIndex, lastVirtualIndex, totalSize, virtualItems.length]);

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
      const nextScrollMargin = documentMode
        ? content.getBoundingClientRect().top + window.scrollY + paddingTop + loadBeforeHeight
        : paddingTop + loadBeforeHeight;
      const viewportHeight = documentMode
        ? document.documentElement.clientHeight - 52
        : viewport.clientHeight;
      const nextAvailableHeight = Math.max(0, viewportHeight - paddingTop - paddingBottom - loadBeforeHeight);
      setScrollMargin((currentMargin) => currentMargin === nextScrollMargin ? currentMargin : nextScrollMargin);
      setAvailableHeight((currentHeight) =>
        currentHeight === nextAvailableHeight ? currentHeight : nextAvailableHeight
      );
    };

    syncAvailableHeight();

    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", syncAvailableHeight);
      return () => {
        window.removeEventListener("resize", syncAvailableHeight);
      };
    }

    const observer = new ResizeObserver(syncAvailableHeight);
    observer.observe(viewport);
    observer.observe(content);
    if (loadBeforeStatusRef.current) {
      observer.observe(loadBeforeStatusRef.current);
    }
    return () => {
      observer.disconnect();
    };
  }, [contentRef, documentMode, viewportRef]);

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
    const viewport = getScrollElement();
    if (!viewport || !hasMoreBefore || loadingBefore || loadBeforeError || oldestEntryKey === null) {
      return;
    }
    if (viewport.scrollHeight > viewport.clientHeight + 1 || autoFillOldestKeyRef.current === oldestEntryKey) {
      return;
    }
    autoFillOldestKeyRef.current = oldestEntryKey;
    requestLoadBefore();
  }, [availableHeight, entries.length, getScrollElement, hasMoreBefore, loadBeforeError, loadingBefore, oldestEntryKey, requestLoadBefore]);

  return (
    <div {...stylex.props(styles.root, documentMode ? styles.rootDocument : styles.rootEmbedded)}>
      {documentMode ? <div aria-hidden="true" {...stylex.props(styles.documentTopFade)} /> : null}
      <div
        ref={viewportRef}
        {...stylex.props(styles.viewport, documentMode ? styles.viewportDocument : styles.viewportEmbedded)}
        aria-label={ariaLabel}
        onKeyDown={handleKeyDown}
        onPointerDown={cancelUserScrollAnimation}
        onScroll={documentMode ? undefined : handleElementScroll}
        onTouchMove={handleTouchMove}
        onTouchStart={handleTouchStart}
        onWheel={handleWheel}
        role="region"
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
      <AnimatePresence initial={false}>
        {!stuckToBottom ? (
          <m.button
            key="scroll-to-end"
            type="button"
            {...stylex.props(
              styles.scrollButton,
              documentMode ? styles.documentScrollButton : styles.embeddedScrollButton
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
              const viewport = getScrollElement();
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
  instance: TranscriptVirtualizer
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
