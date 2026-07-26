import * as React from "react";

const KEYBOARD_VIEWPORT_DELTA_PX = 160;

type ScrollToEndOptions = { behavior?: ScrollBehavior };

type TranscriptScrollerContextValue = {
  contentRef: React.RefObject<HTMLDivElement | null>;
  viewportRef: React.RefObject<HTMLDivElement | null>;
  getScrollElement: () => HTMLElement | null;
  scrollMode: TranscriptScrollMode;
  scrollToEnd: (options?: ScrollToEndOptions) => void;
};

type TranscriptScrollerProviderProps = { children: React.ReactNode; scrollMode: TranscriptScrollMode };

export type TranscriptScrollMode = "document" | "element";

const TranscriptScrollerContext = React.createContext<TranscriptScrollerContextValue | null>(null);

export function TranscriptScrollerProvider({ children, scrollMode }: TranscriptScrollerProviderProps) {
  const contentRef = React.useRef<HTMLDivElement | null>(null);
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const [activeScrollMode, setActiveScrollMode] = React.useState(scrollMode);
  const activeScrollModeRef = React.useRef(scrollMode);
  const restoreBottomDistanceRef = React.useRef<number | null>(null);
  const getScrollElement = React.useCallback(() => {
    if (activeScrollMode === "document") {
      return document.scrollingElement as HTMLElement | null;
    }
    return viewportRef.current;
  }, [activeScrollMode]);
  const scrollToEnd = React.useCallback(({ behavior = "auto" }: ScrollToEndOptions = {}) => {
    const viewport = getScrollElement();
    if (!viewport) {
      return;
    }
    if (activeScrollMode === "document") {
      window.scrollTo({ top: viewport.scrollHeight, behavior });
      return;
    }
    viewport.scrollTo({ top: viewport.scrollHeight, behavior });
  }, [activeScrollMode, getScrollElement]);

  React.useEffect(() => {
    const visualViewport = window.visualViewport;
    if (scrollMode !== "document" || !visualViewport) {
      return;
    }

    const syncScrollMode = () => {
      const activeElement = document.activeElement;
      const editableFocused = activeElement instanceof HTMLElement &&
        (activeElement.matches("input, textarea") || activeElement.isContentEditable);
      const keyboardOpen = editableFocused && visualViewport.scale === 1 &&
        document.documentElement.clientHeight - visualViewport.height > KEYBOARD_VIEWPORT_DELTA_PX;
      const nextMode: TranscriptScrollMode = keyboardOpen ? "element" : "document";
      if (nextMode === activeScrollModeRef.current) {
        return;
      }

      const currentViewport = activeScrollModeRef.current === "document"
        ? document.scrollingElement as HTMLElement | null
        : viewportRef.current;
      if (currentViewport) {
        restoreBottomDistanceRef.current = Math.max(
          0,
          currentViewport.scrollHeight - currentViewport.scrollTop - currentViewport.clientHeight
        );
      }
      activeScrollModeRef.current = nextMode;
      setActiveScrollMode(nextMode);
    };

    syncScrollMode();
    visualViewport.addEventListener("resize", syncScrollMode);
    return () => visualViewport.removeEventListener("resize", syncScrollMode);
  }, [scrollMode]);

  React.useLayoutEffect(() => {
    const bottomDistance = restoreBottomDistanceRef.current;
    const viewport = getScrollElement();
    if (bottomDistance === null || !viewport) {
      return;
    }
    restoreBottomDistanceRef.current = null;
    viewport.scrollTop = Math.max(0, viewport.scrollHeight - viewport.clientHeight - bottomDistance);
  }, [activeScrollMode, getScrollElement]);

  const value = React.useMemo(
    () => ({ contentRef, getScrollElement, scrollMode: activeScrollMode, scrollToEnd, viewportRef }),
    [activeScrollMode, getScrollElement, scrollToEnd]
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
