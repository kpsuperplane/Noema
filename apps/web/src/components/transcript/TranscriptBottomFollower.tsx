import * as React from "react";
import { useReducedMotion } from "motion/react";
import { animateScrollToBottom } from "@/motion/scroll";
import { useTranscriptScroller } from "./TranscriptScroller";

const INITIAL_SCROLL_SETTLE_DURATION_MS = 600;

type ActiveScrollAnimation = {
  cancel: () => void;
  key: string;
  messageIds: readonly string[];
};

export function TranscriptBottomFollower({
  arrivalScrollKey,
  arrivalMessageIdsJson,
  followBottomRef,
  onArrivalSettled,
  sentMessageScrollRequest,
  scrollKey
}: {
  arrivalScrollKey: string;
  arrivalMessageIdsJson: string;
  followBottomRef: React.MutableRefObject<boolean>;
  onArrivalSettled: (messageIds: readonly string[]) => void;
  sentMessageScrollRequest: number;
  scrollKey: string;
}) {
  const { contentRef, getScrollElement, scrollMode, scrollToEnd, viewportRef } = useTranscriptScroller();
  const reduceMotion = useReducedMotion();
  const arrivalMessageIds = React.useMemo<readonly string[]>(
    () => JSON.parse(arrivalMessageIdsJson) as string[],
    [arrivalMessageIdsJson]
  );
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
    const viewport = getScrollElement();
    if (!viewport || !scrollKey || initialBottomLockCompletedRef.current) {
      return;
    }

    initialBottomLockCompletedRef.current = true;
    initialBottomLockActiveRef.current = true;
    scheduleInitialBottomLockRelease();
    followBottomRef.current = true;
    scrollToEnd({ behavior: "auto" });

    resizeSyncFrameRef.current = window.requestAnimationFrame(() => {
      resizeSyncFrameRef.current = null;
      if (!initialBottomLockActiveRef.current) {
        return;
      }

      followBottomRef.current = true;
      scrollToEnd({ behavior: "auto" });
    });

  }, [followBottomRef, getScrollElement, scheduleInitialBottomLockRelease, scrollKey, scrollToEnd]);

  React.useLayoutEffect(() => {
    const viewport = getScrollElement();
    if (!viewport || !arrivalScrollKey || arrivalScrollKey === completedArrivalKeyRef.current) {
      return;
    }

    if (!followBottomRef.current || reduceMotion) {
      if (followBottomRef.current) {
        scrollToEnd({ behavior: "auto" });
      }
      completedArrivalKeyRef.current = arrivalScrollKey;
      onArrivalSettled(arrivalMessageIds);
      return;
    }

    activeScrollAnimationRef.current?.cancel();

    const cancel = animateScrollToBottom(viewport, () => {
      completedArrivalKeyRef.current = arrivalScrollKey;
      activeScrollAnimationRef.current = null;
      followBottomRef.current = true;
      onArrivalSettled(arrivalMessageIds);
    });
    activeScrollAnimationRef.current = { cancel, key: arrivalScrollKey, messageIds: arrivalMessageIds };

    return () => {
      if (activeScrollAnimationRef.current?.key === arrivalScrollKey) {
        activeScrollAnimationRef.current.cancel();
        activeScrollAnimationRef.current = null;
      }
    };
  }, [arrivalMessageIds, arrivalScrollKey, followBottomRef, getScrollElement, onArrivalSettled, reduceMotion, scrollToEnd]);

  React.useLayoutEffect(() => {
    const viewport = getScrollElement();
    if (
      !viewport ||
      (!followBottomRef.current && !initialBottomLockActiveRef.current) ||
      activeScrollAnimationRef.current
    ) {
      return;
    }

    scrollToEnd({ behavior: "auto" });
  }, [followBottomRef, getScrollElement, scrollKey, scrollToEnd]);

  React.useLayoutEffect(() => {
    const viewport = getScrollElement();
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
  }, [followBottomRef, getScrollElement, scrollToEnd, sentMessageScrollRequest]);

  React.useLayoutEffect(() => {
    const viewport = getScrollElement();
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
      if (!viewport) {
        return;
      }
      cancelInitialBottomLock();
      const activeAnimation = activeScrollAnimationRef.current;
      followBottomRef.current = false;
      if (!activeAnimation) {
        return;
      }
      activeAnimation.cancel();
      activeScrollAnimationRef.current = null;
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
    if (scrollMode === "element" && viewportRef.current) {
      observer.observe(viewportRef.current);
    }
    if (content) {
      observer.observe(content);
    }
    const eventTarget = scrollMode === "document" ? window : viewport;
    eventTarget.addEventListener("pointerdown", cancelAutomaticScroll, { passive: true });
    eventTarget.addEventListener("wheel", cancelAutomaticScroll, { passive: true });
    eventTarget.addEventListener("touchmove", cancelAutomaticScroll, { passive: true });
    const visualViewport = scrollMode === "element" ? window.visualViewport : null;
    visualViewport?.addEventListener("resize", scheduleSyncToBottom);
    visualViewport?.addEventListener("scroll", scheduleSyncToBottom);

    return () => {
      observer.disconnect();
      eventTarget.removeEventListener("pointerdown", cancelAutomaticScroll);
      eventTarget.removeEventListener("wheel", cancelAutomaticScroll);
      eventTarget.removeEventListener("touchmove", cancelAutomaticScroll);
      visualViewport?.removeEventListener("resize", scheduleSyncToBottom);
      visualViewport?.removeEventListener("scroll", scheduleSyncToBottom);
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
  }, [contentRef, followBottomRef, getScrollElement, onArrivalSettled, scheduleInitialBottomLockRelease, scrollMode, scrollToEnd, viewportRef]);

  return null;
}
