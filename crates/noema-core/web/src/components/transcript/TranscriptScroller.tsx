import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { ArrowDownIcon } from "lucide-react";

type ScrollToEndOptions = {
  behavior?: ScrollBehavior;
};

type TranscriptScrollerContextValue = {
  viewportRef: React.RefObject<HTMLDivElement | null>;
  scrollToEnd: (options?: ScrollToEndOptions) => void;
};

type TranscriptScrollerProviderProps = {
  children: React.ReactNode;
};

type TranscriptScrollerProps = {
  children: React.ReactNode;
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
  viewport: {
    width: "100%",
    height: "100%",
    minWidth: 0,
    minHeight: 0,
    overflowAnchor: "none",
    overflowY: "auto",
    overscrollBehavior: "contain"
  },
  content: {
    display: "flex",
    width: "var(--chat-column-width)",
    minHeight: "100%",
    flexDirection: "column",
    justifyContent: "flex-end",
    gap: 12,
    marginInline: "auto",
    paddingBlock: 24,
    paddingInline: 2
  },
  item: {
    display: "flex",
    width: "100%",
    minWidth: 0,
    flexShrink: 0,
    contentVisibility: "auto",
    containIntrinsicSize: "auto 10rem"
  },
  itemEnd: {
    justifyContent: "flex-end"
  },
  compact: {
    marginTop: -8
  },
  scrollButton: {
    position: "absolute",
    left: "50%",
    bottom: 16,
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
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const scrollToEnd = React.useCallback(({ behavior = "auto" }: ScrollToEndOptions = {}) => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    viewport.scrollTo({ top: viewport.scrollHeight, behavior });
  }, []);
  const value = React.useMemo(() => ({ viewportRef, scrollToEnd }), [scrollToEnd]);

  return <TranscriptScrollerContext.Provider value={value}>{children}</TranscriptScrollerContext.Provider>;
}

export function useTranscriptScroller() {
  const context = React.useContext(TranscriptScrollerContext);
  if (!context) {
    throw new Error("useTranscriptScroller must be used within TranscriptScrollerProvider");
  }
  return context;
}

export function TranscriptScroller({ children, onViewportScroll, "aria-label": ariaLabel }: TranscriptScrollerProps) {
  const { viewportRef, scrollToEnd } = useTranscriptScroller();
  const [stuckToBottom, setStuckToBottom] = React.useState(true);
  const handleScroll = React.useCallback(
    (event: React.UIEvent<HTMLDivElement>) => {
      const viewport = event.currentTarget;
      const distance = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
      setStuckToBottom(distance < 80);
      onViewportScroll?.(event);
    },
    [onViewportScroll]
  );

  return (
    <div {...stylex.props(styles.root)}>
      <div
        ref={viewportRef}
        {...stylex.props(styles.viewport)}
        aria-atomic="false"
        aria-label={ariaLabel}
        aria-live="polite"
        aria-relevant="additions text"
        onScroll={handleScroll}
        role="log"
      >
        <div {...stylex.props(styles.content)}>{children}</div>
      </div>
      <button
        type="button"
        {...stylex.props(styles.scrollButton, stuckToBottom && styles.hidden)}
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
