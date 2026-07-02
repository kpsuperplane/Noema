import * as React from "react";
import { useTranscriptScroller } from "./TranscriptScroller";

export const ARRIVAL_SCROLL_SETTLE_DURATION_MS = 260;
const SCROLL_REVEAL_DURATION_MS = 240;

type ScrollMetrics = {
  clientHeight: number;
  scrollHeight: number;
  scrollTop: number;
};

type ActiveScrollAnimation = {
  cancel: () => void;
  key: string;
};

export function TranscriptBottomFollower({
  arrivalScrollKey,
  followBottomRef,
  scrollKey
}: {
  arrivalScrollKey: string;
  followBottomRef: React.MutableRefObject<boolean>;
  scrollKey: string;
}) {
  const { scrollToEnd, viewportRef } = useTranscriptScroller();
  const previousMetricsRef = React.useRef<ScrollMetrics | null>(null);
  const completedArrivalKeyRef = React.useRef("");
  const activeScrollAnimationRef = React.useRef<ActiveScrollAnimation | null>(null);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const previousMetrics = previousMetricsRef.current;
    if (
      !viewport ||
      !arrivalScrollKey ||
      !previousMetrics ||
      arrivalScrollKey === completedArrivalKeyRef.current ||
      !followBottomRef.current ||
      prefersReducedMotion()
    ) {
      return;
    }

    activeScrollAnimationRef.current?.cancel();
    viewport.scrollTop = Math.min(previousMetrics.scrollTop, scrollBottomTop(readScrollMetrics(viewport)));

    const cancel = animateScrollToBottom(viewport, viewport.scrollTop, () => {
      completedArrivalKeyRef.current = arrivalScrollKey;
      activeScrollAnimationRef.current = null;
      previousMetricsRef.current = readScrollMetrics(viewport);
    });
    activeScrollAnimationRef.current = { cancel, key: arrivalScrollKey };

    return () => {
      if (activeScrollAnimationRef.current?.key === arrivalScrollKey) {
        activeScrollAnimationRef.current.cancel();
        activeScrollAnimationRef.current = null;
        previousMetricsRef.current = readScrollMetrics(viewport);
      }
    };
  }, [arrivalScrollKey, followBottomRef, viewportRef]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || !followBottomRef.current || activeScrollAnimationRef.current) {
      return;
    }

    scrollToEnd({ behavior: "auto" });
  }, [followBottomRef, scrollKey, scrollToEnd, viewportRef]);

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || activeScrollAnimationRef.current) {
      return;
    }

    previousMetricsRef.current = readScrollMetrics(viewport);
  });

  return null;
}

function animateScrollToBottom(viewport: HTMLDivElement, startScrollTop: number, onComplete: () => void) {
  let animationFrame: number | null = null;
  let startedAt: number | null = null;

  const step = (timestamp: number) => {
    if (startedAt === null) {
      startedAt = timestamp;
    }

    const progress = Math.min(1, (timestamp - startedAt) / SCROLL_REVEAL_DURATION_MS);
    const targetScrollTop = scrollBottomTop(readScrollMetrics(viewport));
    const distance = targetScrollTop - startScrollTop;
    viewport.scrollTop = startScrollTop + distance * easeOutCubic(progress);

    if (progress < 1) {
      animationFrame = window.requestAnimationFrame(step);
      return;
    }

    viewport.scrollTop = targetScrollTop;
    onComplete();
  };

  animationFrame = window.requestAnimationFrame(step);

  return () => {
    if (animationFrame !== null) {
      window.cancelAnimationFrame(animationFrame);
    }
  };
}

function easeOutCubic(progress: number) {
  return 1 - Math.pow(1 - progress, 3);
}

function prefersReducedMotion() {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
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
