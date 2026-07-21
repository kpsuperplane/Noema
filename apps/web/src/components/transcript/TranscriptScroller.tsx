import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDownIcon } from "lucide-react";
import { renderedEntryMessageId, type RenderTranscriptEntry } from "./renderModel";
import { BOTTOM_SCROLL_THRESHOLD_PX } from "./scrollModel";
import { shouldLoadBeforeFromVirtualItems, transcriptBottomAnchorOffset } from "./transcriptScrollerModel";

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

type PrependAnchor = {
  key: React.Key;
  offset: number;
  oldestKey: React.Key;
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
    overflowAnchor: "none",
    overflowX: "hidden",
    overflowY: "auto",
    overscrollBehavior: "contain",
    maskImage:
      "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black calc(100% - var(--chat-transcript-bottom-fade, 128px)), transparent 100%)",
    WebkitMaskImage:
      "linear-gradient(to bottom, transparent 0, black var(--chat-transcript-top-fade), black calc(100% - var(--chat-transcript-bottom-fade, 128px)), transparent 100%)"
  },
  viewportEmbedded: {
    height: "100%",
    maxHeight: "none",
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
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-default)",
    backgroundColor: "var(--noema-surface-card)",
    color: "var(--noema-text-primary)",
    transform: "translateX(-50%)",
    transitionDuration: "200ms",
    transitionProperty: "opacity, transform",
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
  hidden: {
    pointerEvents: "none",
    opacity: 0,
    transform: "translateX(-50%) translateY(100%) scale(0.95)"
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
  const [stuckToBottom, setStuckToBottom] = React.useState(true);
  const [userScrolledTowardStart, setUserScrolledTowardStart] = React.useState(false);
  const [availableHeight, setAvailableHeight] = React.useState(0);
  const pendingPrependAnchorRef = React.useRef<PrependAnchor | null>(null);
  const loadBeforeStatusRef = React.useRef<HTMLDivElement | null>(null);
  const nearTopLoadArmedRef = React.useRef(true);
  const requestedOldestKeyRef = React.useRef<React.Key | null>(null);
  const autoFillOldestKeyRef = React.useRef<React.Key | null>(null);
  const touchStartYRef = React.useRef<number | null>(null);
  // TanStack Virtual exposes imperative measurement functions that React Compiler cannot memoize.
  // eslint-disable-next-line react-hooks/incompatible-library
  const rowVirtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => 96,
    anchorTo: "end",
    followOnAppend: "auto",
    scrollEndThreshold: BOTTOM_SCROLL_THRESHOLD_PX,
    overscan: 8,
    getItemKey: (index) => renderedEntryMessageId(entries[index])
  });
  const virtualItems = rowVirtualizer.getVirtualItems();
  const totalSize = rowVirtualizer.getTotalSize();
  const bottomAnchorOffset = transcriptBottomAnchorOffset({ availableHeight, totalSize });
  const virtualSizerHeight = Math.max(totalSize, availableHeight);
  const oldestEntryKey = entries[0] ? renderedEntryMessageId(entries[0]) : null;
  const handleScroll = React.useCallback(
    (event: React.UIEvent<HTMLDivElement>) => {
      const viewport = event.currentTarget;
      const distance = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
      setStuckToBottom(distance < BOTTOM_SCROLL_THRESHOLD_PX);
      onViewportScroll?.(event);
    },
    [onViewportScroll]
  );
  const markUserScrolledTowardStart = React.useCallback(() => {
    setUserScrolledTowardStart(true);
  }, []);
  const handleWheel = React.useCallback(
    (event: React.WheelEvent<HTMLDivElement>) => {
      if (event.deltaY < 0) {
        markUserScrolledTowardStart();
      }
    },
    [markUserScrolledTowardStart]
  );
  const handleKeyDown = React.useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "ArrowUp" || event.key === "PageUp" || event.key === "Home") {
        markUserScrolledTowardStart();
      }
    },
    [markUserScrolledTowardStart]
  );
  const handleTouchStart = React.useCallback((event: React.TouchEvent<HTMLDivElement>) => {
    touchStartYRef.current = event.touches[0]?.clientY ?? null;
  }, []);
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
      const nextAvailableHeight = Math.max(0, viewport.clientHeight - paddingTop - paddingBottom - loadBeforeHeight);
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
  }, [contentRef, hasMoreBefore, loadBeforeError, loadingBefore, viewportRef]);

  React.useEffect(() => {
    const first = virtualItems[0];
    if (!first || !hasMoreBefore || loadingBefore || !oldestEntryKey) {
      return;
    }
    if (first.index > 3) {
      nearTopLoadArmedRef.current = true;
      return;
    }
    if (!nearTopLoadArmedRef.current) {
      return;
    }
    if (
      !shouldLoadBeforeFromVirtualItems({
        virtualItems,
        hasMoreBefore,
        loadingBefore,
        oldestEntryKey,
        requestedOldestKey: requestedOldestKeyRef.current?.toString() ?? null,
        nearTopLoadArmed: nearTopLoadArmedRef.current,
        userScrolledTowardStart
      })
    ) {
      return;
    }

    pendingPrependAnchorRef.current = capturePrependAnchor(viewportRef.current, virtualItems, oldestEntryKey);
    nearTopLoadArmedRef.current = false;
    requestedOldestKeyRef.current = oldestEntryKey;
    onLoadBefore();
  }, [hasMoreBefore, loadingBefore, oldestEntryKey, onLoadBefore, userScrolledTowardStart, viewportRef, virtualItems]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || !hasMoreBefore || loadingBefore || loadBeforeError || oldestEntryKey === null) {
      return;
    }
    if (viewport.scrollHeight > viewport.clientHeight + 1 || autoFillOldestKeyRef.current === oldestEntryKey) {
      return;
    }
    autoFillOldestKeyRef.current = oldestEntryKey;
    onLoadBefore();
  }, [availableHeight, entries.length, hasMoreBefore, loadBeforeError, loadingBefore, oldestEntryKey, onLoadBefore, viewportRef]);

  React.useLayoutEffect(() => {
    const anchor = pendingPrependAnchorRef.current;
    const viewport = viewportRef.current;
    if (!anchor || !viewport) {
      return;
    }
    if (oldestEntryKey === anchor.oldestKey) {
      return;
    }

    const anchorIndex = entries.findIndex((entry) => renderedEntryMessageId(entry) === anchor.key);
    if (anchorIndex === -1) {
      pendingPrependAnchorRef.current = null;
      return;
    }

    const offset = rowVirtualizer.getOffsetForIndex(anchorIndex, "start")?.[0];
    if (offset === undefined) {
      return;
    }

    pendingPrependAnchorRef.current = null;
    rowVirtualizer.scrollToOffset(Math.max(0, offset + anchor.offset), { align: "start" });
  }, [entries, oldestEntryKey, rowVirtualizer, viewportRef]);

  return (
    <div {...stylex.props(styles.root, density === "embedded" && styles.rootEmbedded)}>
      <div
        ref={viewportRef}
        {...stylex.props(styles.viewport, density === "embedded" && styles.viewportEmbedded)}
        aria-label={ariaLabel}
        onKeyDown={handleKeyDown}
        onScroll={handleScroll}
        onTouchMove={handleTouchMove}
        onTouchStart={handleTouchStart}
        onWheel={handleWheel}
        role="region"
        tabIndex={0}
      >
        <div ref={contentRef} {...stylex.props(styles.content, density === "embedded" && styles.contentEmbedded)}>
          {hasMoreBefore || loadingBefore || loadBeforeError ? (
            <div
              ref={loadBeforeStatusRef}
              {...stylex.props(styles.loadBeforeStatus)}
              role={loadBeforeError ? "alert" : "status"}
            >
              {loadBeforeError ? (
                <button type="button" {...stylex.props(styles.loadBeforeButton)} onClick={onLoadBefore}>
                  Retry loading earlier messages
                </button>
              ) : loadingBefore ? (
                "Loading earlier messages"
              ) : hasMoreBefore ? (
                <button type="button" {...stylex.props(styles.loadBeforeButton)} onClick={onLoadBefore}>
                  Load earlier messages
                </button>
              ) : null}
            </div>
          ) : null}
          <div {...stylex.props(styles.virtualSizer)} style={{ height: `${virtualSizerHeight}px` }}>
            {virtualItems.map((virtualItem) => {
              const entry = entries[virtualItem.index];
              return (
                <div
                  key={virtualItem.key}
                  ref={rowVirtualizer.measureElement}
                  data-index={virtualItem.index}
                  {...stylex.props(styles.virtualRow)}
                  style={{ transform: `translateY(${virtualItem.start + bottomAnchorOffset}px)` }}
                >
                  {renderEntry(entry, virtualItem.index)}
                </div>
              );
            })}
          </div>
        </div>
      </div>
      <button
        type="button"
        {...stylex.props(
          styles.scrollButton,
          density === "embedded" && styles.embeddedScrollButton,
          stuckToBottom && styles.hidden
        )}
        aria-hidden={stuckToBottom}
        data-active={stuckToBottom ? "false" : "true"}
        disabled={stuckToBottom}
        onClick={() => scrollToEnd({ behavior: "smooth" })}
        tabIndex={stuckToBottom ? -1 : 0}
      >
        <ArrowDownIcon aria-hidden="true" size={16} />
        <span {...stylex.props(styles.srOnly)}>Scroll to end</span>
      </button>
    </div>
  );
}

function cssPixels(value: string) {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function capturePrependAnchor(
  viewport: HTMLDivElement | null,
  virtualItems: ReturnType<ReturnType<typeof useVirtualizer>["getVirtualItems"]>,
  oldestKey: React.Key
): PrependAnchor | null {
  if (!viewport) {
    return null;
  }

  const scrollTop = viewport.scrollTop;
  const firstVisible = virtualItems.find((item) => item.end > scrollTop) ?? virtualItems[0];
  if (!firstVisible) {
    return null;
  }

  return {
    key: firstVisible.key,
    offset: scrollTop - firstVisible.start,
    oldestKey
  };
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
