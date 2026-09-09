import * as React from "react";
import { useReducedMotion } from "motion/react";
import { animateScrollToBottom } from "@/motion/scroll";
import { useTranscriptScroller } from "./TranscriptScroller";

const INITIAL_SCROLL_SETTLE_DURATION_MS = 600;

type ScrollMetrics = {
  clientHeight: number;
  scrollHeight: number;
  scrollTop: number;
};

type ActiveScrollAnimation = {
  cancel: () => void;
  key: string;
  messageIds: readonly string[];
};

export function TranscriptBottomFollower({
  arrivalScrollKey,
  arrivalMessageIdsJson,
  followBottomRef,
  restoreInitialScroll = false,
  onArrivalSettled,
  sentMessageScrollRequest,
  scrollKey
}: {
  arrivalScrollKey: string;
  arrivalMessageIdsJson: string;
  followBottomRef: React.MutableRefObject<boolean>;
  restoreInitialScroll?: boolean;
  onArrivalSettled: (messageIds: readonly string[]) => void;
  sentMessageScrollRequest: number;
  scrollKey: string;
}) {
  const {
    contentRef,
    scrollToEnd,
    viewportRef
  } = useTranscriptScroller();
  const reduceMotion = useReducedMotion();
  const arrivalMessageIds = React.useMemo<readonly string[]>(
    () => JSON.parse(arrivalMessageIdsJson) as string[],
    [arrivalMessageIdsJson]
  );
  const previousMetricsRef = React.useRef<ScrollMetrics | null>(null);
  const completedArrivalKeyRef = React.useRef("");
  const completedSentMessageScrollRequestRef = React.useRef(sentMessageScrollRequest);
  const initialBottomLockActiveRef = React.useRef(false);
  const initialBottomLockCompletedRef = React.useRef(false);
  const initialBottomLockTimeoutRef = React.useRef<number | null>(null);
  const activeScrollAnimationRef = React.useRef<ActiveScrollAnimation | null>(null);
  const resizeSyncFrameRef = React.useRef<number | null>(null);
  const resizeSyncTimeoutRef = React.useRef<number | null>(null);
  const scheduleInitialBottomLockRelease = React.useCallback(() => {
    if (!initialBottomLockActiveRef.current) {
      return;
    }
    if (initialBottomLockTimeoutRef.current !== null) {
      window.clearTimeout(initialBottomLockTimeoutRef.current);
    }
    initialBottomLockTimeoutRef.current = window.setTimeout(() => {
      initialBottomLockActiveRef.current = false;
      initialBottomLockTimeoutRef.current = null;
    }, INITIAL_SCROLL_SETTLE_DURATION_MS);
  }, []);

  React.useLayoutEffect(() => {
    return () => {
      if (initialBottomLockTimeoutRef.current !== null) {
        window.clearTimeout(initialBottomLockTimeoutRef.current);
        initialBottomLockTimeoutRef.current = null;
      }
      if (resizeSyncFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeSyncFrameRef.current);
        resizeSyncFrameRef.current = null;
      }
      if (resizeSyncTimeoutRef.current !== null) {
        window.clearTimeout(resizeSyncTimeoutRef.current);
        resizeSyncTimeoutRef.current = null;
      }
    };
  }, []);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || !scrollKey || initialBottomLockCompletedRef.current) {
      return;
    }

    initialBottomLockCompletedRef.current = true;
    if (restoreInitialScroll && !followBottomRef.current) {
      previousMetricsRef.current = readScrollMetrics(viewport);
      return;
    }
    initialBottomLockActiveRef.current = true;
    scheduleInitialBottomLockRelease();
    followBottomRef.current = true;
    scrollToEnd({ behavior: "auto" });
    previousMetricsRef.current = readScrollMetrics(viewport);

    resizeSyncFrameRef.current = window.requestAnimationFrame(() => {
      resizeSyncFrameRef.current = null;
      if (!initialBottomLockActiveRef.current) {
        return;
      }

      followBottomRef.current = true;
      scrollToEnd({ behavior: "auto" });
      previousMetricsRef.current = readScrollMetrics(viewport);
    });

  }, [followBottomRef, restoreInitialScroll, scheduleInitialBottomLockRelease, scrollKey, scrollToEnd, viewportRef]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const previousMetrics = previousMetricsRef.current;
    if (!viewport || !arrivalScrollKey || !previousMetrics || arrivalScrollKey === completedArrivalKeyRef.current) {
      return;
    }

    if (!followBottomRef.current || reduceMotion) {
      if (followBottomRef.current) {
        scrollToEnd({ behavior: "auto" });
      }
      completedArrivalKeyRef.current = arrivalScrollKey;
      previousMetricsRef.current = readScrollMetrics(viewport);
      onArrivalSettled(arrivalMessageIds);
      return;
    }

    activeScrollAnimationRef.current?.cancel();
    viewport.scrollTop = Math.min(previousMetrics.scrollTop, scrollBottomTop(readScrollMetrics(viewport)));

    const cancel = animateScrollToBottom(viewport, {
      onComplete: () => {
        completedArrivalKeyRef.current = arrivalScrollKey;
        activeScrollAnimationRef.current = null;
        followBottomRef.current = true;
        previousMetricsRef.current = readScrollMetrics(viewport);
        onArrivalSettled(arrivalMessageIds);
      }
    });
    activeScrollAnimationRef.current = { cancel, key: arrivalScrollKey, messageIds: arrivalMessageIds };

    return () => {
      if (activeScrollAnimationRef.current?.key === arrivalScrollKey) {
        activeScrollAnimationRef.current.cancel();
        activeScrollAnimationRef.current = null;
        previousMetricsRef.current = readScrollMetrics(viewport);
      }
    };
  }, [
    arrivalMessageIds,
    arrivalScrollKey,
    followBottomRef,
    onArrivalSettled,
    reduceMotion,
    scrollToEnd,
    viewportRef
  ]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (
      !viewport ||
      (!followBottomRef.current && !initialBottomLockActiveRef.current) ||
      activeScrollAnimationRef.current
    ) {
      return;
    }

    scrollToEnd({ behavior: "auto" });
  }, [followBottomRef, scrollKey, scrollToEnd, viewportRef]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || sentMessageScrollRequest <= completedSentMessageScrollRequestRef.current) {
      return;
    }

    completedSentMessageScrollRequestRef.current = sentMessageScrollRequest;
    if (followBottomRef.current) {
      return;
    }

    activeScrollAnimationRef.current?.cancel();
    activeScrollAnimationRef.current = null;
    followBottomRef.current = true;
    scrollToEnd({ behavior: "auto" });
    previousMetricsRef.current = readScrollMetrics(viewport);
  }, [followBottomRef, scrollToEnd, sentMessageScrollRequest, viewportRef]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (!viewport || typeof ResizeObserver === "undefined") {
      return;
    }

    function cancelInitialBottomLock() {
      initialBottomLockActiveRef.current = false;
      if (initialBottomLockTimeoutRef.current !== null) {
        window.clearTimeout(initialBottomLockTimeoutRef.current);
        initialBottomLockTimeoutRef.current = null;
      }
    }

    function cancelAutomaticScroll() {
      cancelInitialBottomLock();
      const activeAnimation = activeScrollAnimationRef.current;
      if (!activeAnimation || !viewport) {
        return;
      }
      activeAnimation.cancel();
      activeScrollAnimationRef.current = null;
      followBottomRef.current = false;
      previousMetricsRef.current = readScrollMetrics(viewport);
      onArrivalSettled(activeAnimation.messageIds);
    }

    function syncToBottom() {
      if (
        !viewport ||
        (!followBottomRef.current && !initialBottomLockActiveRef.current) ||
        activeScrollAnimationRef.current
      ) {
        return;
      }

      scrollToEnd({ behavior: "auto" });
      if (initialBottomLockActiveRef.current) {
        followBottomRef.current = true;
      }
      previousMetricsRef.current = readScrollMetrics(viewport);
    }

    function scheduleSyncToBottom() {
      scheduleInitialBottomLockRelease();
      if (initialBottomLockActiveRef.current) {
        syncToBottom();
        return;
      }
      if (resizeSyncFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeSyncFrameRef.current);
      }
      if (resizeSyncTimeoutRef.current !== null) {
        window.clearTimeout(resizeSyncTimeoutRef.current);
      }

      resizeSyncFrameRef.current = window.requestAnimationFrame(() => {
        resizeSyncFrameRef.current = null;
        syncToBottom();
        resizeSyncTimeoutRef.current = window.setTimeout(() => {
          resizeSyncTimeoutRef.current = null;
          syncToBottom();
        }, 120);
      });
    }

    const observer = new ResizeObserver(scheduleSyncToBottom);
    observer.observe(viewport);
    if (content) {
      observer.observe(content);
    }
    viewport.addEventListener("pointerdown", cancelAutomaticScroll, { passive: true });
    viewport.addEventListener("wheel", cancelAutomaticScroll, { passive: true });
    viewport.addEventListener("touchmove", cancelAutomaticScroll, { passive: true });
    window.visualViewport?.addEventListener("resize", scheduleSyncToBottom);
    window.visualViewport?.addEventListener("scroll", scheduleSyncToBottom);

    return () => {
      observer.disconnect();
      viewport.removeEventListener("pointerdown", cancelAutomaticScroll);
      viewport.removeEventListener("wheel", cancelAutomaticScroll);
      viewport.removeEventListener("touchmove", cancelAutomaticScroll);
      window.visualViewport?.removeEventListener("resize", scheduleSyncToBottom);
      window.visualViewport?.removeEventListener("scroll", scheduleSyncToBottom);
      if (initialBottomLockTimeoutRef.current !== null) {
        window.clearTimeout(initialBottomLockTimeoutRef.current);
        initialBottomLockTimeoutRef.current = null;
      }
      if (resizeSyncFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeSyncFrameRef.current);
        resizeSyncFrameRef.current = null;
      }
      if (resizeSyncTimeoutRef.current !== null) {
        window.clearTimeout(resizeSyncTimeoutRef.current);
        resizeSyncTimeoutRef.current = null;
      }
    };
  }, [contentRef, followBottomRef, onArrivalSettled, scheduleInitialBottomLockRelease, scrollToEnd, viewportRef]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || activeScrollAnimationRef.current) {
      return;
    }

    previousMetricsRef.current = readScrollMetrics(viewport);
  });

  return null;
}

function readScrollMetrics(viewport: HTMLDivElement): ScrollMetrics {
  return {
    clientHeight: viewport.clientHeight,
    scrollHeight: viewport.scrollHeight,
    scrollTop: viewport.scrollTop
  };
}

function scrollBottomTop(metrics: ScrollMetrics) {
  return Math.max(0, metrics.scrollHeight - metrics.clientHeight);
}
