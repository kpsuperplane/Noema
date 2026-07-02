import * as React from "react";
import { useTranscriptScroller } from "./TranscriptScroller";

export const ARRIVAL_SCROLL_SETTLE_DURATION_MS = 260;
const SCROLL_REVEAL_DURATION_MS = 180;

type ScrollMetrics = {
  clientHeight: number;
  scrollHeight: number;
  scrollTop: number;
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
  const animatedArrivalKeyRef = React.useRef("");

  React.useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }

    const previousMetrics = previousMetricsRef.current;
    const currentMetrics = readScrollMetrics(viewport);
    const targetScrollTop = scrollBottomTop(currentMetrics);
    const shouldAnimateArrival =
      !!arrivalScrollKey &&
      arrivalScrollKey !== animatedArrivalKeyRef.current &&
      followBottomRef.current &&
      !prefersReducedMotion();

    if (shouldAnimateArrival && previousMetrics) {
      animatedArrivalKeyRef.current = arrivalScrollKey;
      viewport.scrollTop = Math.min(previousMetrics.scrollTop, targetScrollTop);
      const cancelScrollAnimation = animateScrollTop(viewport, targetScrollTop, () => {
        previousMetricsRef.current = readScrollMetrics(viewport);
      });
      return () => {
        cancelScrollAnimation?.();
        previousMetricsRef.current = readScrollMetrics(viewport);
      };
    }

    if (followBottomRef.current) {
      scrollToEnd({ behavior: "auto" });
    }

    previousMetricsRef.current = readScrollMetrics(viewport);
  }, [arrivalScrollKey, followBottomRef, scrollKey, scrollToEnd, viewportRef]);

  return null;
}

function animateScrollTop(viewport: HTMLDivElement, targetScrollTop: number, onComplete: () => void) {
  const startScrollTop = viewport.scrollTop;
  const distance = targetScrollTop - startScrollTop;
  if (Math.abs(distance) < 1) {
    viewport.scrollTop = targetScrollTop;
    onComplete();
    return;
  }

  let animationFrame: number | null = null;
  let startedAt: number | null = null;

  const step = (timestamp: number) => {
    if (startedAt === null) {
      startedAt = timestamp;
    }

    const progress = Math.min(1, (timestamp - startedAt) / SCROLL_REVEAL_DURATION_MS);
    viewport.scrollTop = startScrollTop + distance * easeOutCubic(progress);

    if (progress < 1) {
      animationFrame = window.requestAnimationFrame(step);
      return;
    }

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
