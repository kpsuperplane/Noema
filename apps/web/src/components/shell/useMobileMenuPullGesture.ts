import React from "react";
import { animate, type AnimationPlaybackControls } from "motion/react";
import { springs } from "@/motion/springs";
import { shouldAnimateDeckNavigation } from "./deckNavigation";
import { mobileMenuRevealHeightProperty } from "./useMobileMenuRevealHeight";

const pullActivationPx = 8;
const verticalDominance = 1.2;
const velocityThreshold = 0.45;
const velocityMaxAgeMs = 120;
type PullGesture = {
  mode: "open" | "close";
  startX: number;
  startY: number;
  lastY: number;
  lastTime: number;
  velocityY: number;
  velocityTime: number;
  offsetPx: number;
  targetOffsetPx: number;
  scrollOwner: HTMLElement | null;
  captured: boolean;
};

export function useMobileMenuPullGesture({
  deckRef,
  enabled,
  closeNav,
  navOpen,
  onNavigationSettled,
  openNav
}: {
  deckRef: React.RefObject<HTMLElement | null>;
  enabled: boolean;
  closeNav: () => void;
  navOpen: boolean;
  onNavigationSettled: () => void;
  openNav: () => void;
}) {
  const navOpenRef = React.useRef(navOpen);
  const gestureRef = React.useRef<PullGesture | null>(null);
  const animationRef = React.useRef<AnimationPlaybackControls | null>(null);
  const settlingRef = React.useRef(false);
  const [snapshot, setSnapshot] = React.useState({ dragging: false, settling: false });

  const finishSettlement = React.useCallback((deck: HTMLElement, navigated: boolean) => {
    animationRef.current = null;
    settlingRef.current = false;
    setSnapshot({ dragging: false, settling: false });
    deck.style.removeProperty("transition");
    deck.style.removeProperty("transform");
    if (navigated) onNavigationSettled();
  }, [onNavigationSettled]);

  React.useEffect(() => {
    navOpenRef.current = navOpen;
  }, [navOpen]);

  React.useEffect(() => {
    const deck = deckRef.current;
    if (!enabled || !deck) return;

    const finish = (commit: boolean, velocityY = 0) => {
      const gesture = gestureRef.current;
      gestureRef.current = null;
      if (!gesture?.captured) return;

      settlingRef.current = true;
      setSnapshot({ dragging: false, settling: true });
      if (commit) {
        if (gesture.mode === "open") openNav();
        else closeNav();
      }

      const destination = gesture.mode === "open"
        ? commit ? gesture.targetOffsetPx : 0
        : commit ? 0 : gesture.targetOffsetPx;
      if (!shouldAnimateDeckNavigation()) {
        deck.style.setProperty("transform", `translateY(${destination}px)`);
        finishSettlement(deck, commit);
        return;
      }
      animationRef.current = animate(gesture.offsetPx, destination, {
        ...springs.surface,
        velocity: velocityY * 1_000,
        onUpdate: (value) => {
          deck.style.setProperty("transform", `translateY(${value}px)`);
        },
        onComplete: () => finishSettlement(deck, commit)
      });
    };

    const onTouchStart = (event: TouchEvent) => {
      if (
        settlingRef.current
        || event.touches.length !== 1
        || !mobileViewport()
        || (!navOpenRef.current && ignoresPullGesture(event.target))
      ) {
        return;
      }

      const mode = navOpenRef.current ? "close" : "open";
      const scrollOwner = mode === "open" ? nearestScrollOwner(event.target, deck) : null;
      if (mode === "open" && scrollOwner && scrollOwner.scrollTop > 0.5) return;
      const touch = event.touches[0];
      const now = performance.now();
      gestureRef.current = {
        mode,
        startX: touch.clientX,
        startY: touch.clientY,
        lastY: touch.clientY,
        lastTime: now,
        velocityY: 0,
        velocityTime: now,
        offsetPx: mode === "open" ? 0 : mobileRevealOffset(deck),
        targetOffsetPx: mobileRevealOffset(deck),
        scrollOwner,
        captured: false
      };
    };

    const onTouchMove = (event: TouchEvent) => {
      const gesture = gestureRef.current;
      if (!gesture || event.touches.length !== 1) return;
      const touch = event.touches[0];
      const deltaX = touch.clientX - gesture.startX;
      const deltaY = touch.clientY - gesture.startY;

      if (!gesture.captured) {
        if (Math.max(Math.abs(deltaX), Math.abs(deltaY)) < pullActivationPx) return;
        const movingTowardMenuState = gesture.mode === "open" ? deltaY > 0 : deltaY < 0;
        if (
          !movingTowardMenuState
          || Math.abs(deltaY) < Math.abs(deltaX) * verticalDominance
          || (gesture.mode === "open" && (gesture.scrollOwner?.scrollTop ?? 0) > 0.5)
        ) {
          gestureRef.current = null;
          return;
        }
        gesture.captured = true;
        deck.style.setProperty("transition", "none");
        deck.style.setProperty("transform", `translateY(${gesture.offsetPx}px)`);
        setSnapshot({ dragging: true, settling: false });
      }

      event.preventDefault();
      const now = performance.now();
      const elapsed = Math.max(1, now - gesture.lastTime);
      gesture.velocityY = (touch.clientY - gesture.lastY) / elapsed;
      gesture.velocityTime = now;
      gesture.lastY = touch.clientY;
      gesture.lastTime = now;
      gesture.offsetPx = gesture.mode === "open"
        ? Math.min(
            Math.max(0, gesture.targetOffsetPx - 1),
            Math.max(0, deltaY - pullActivationPx)
          )
        : Math.max(1, gesture.targetOffsetPx + Math.min(0, deltaY + pullActivationPx));
      deck.style.setProperty("transform", `translateY(${gesture.offsetPx}px)`);
    };

    const onTouchEnd = () => {
      const gesture = gestureRef.current;
      if (!gesture) return;
      const recentVelocity = performance.now() - gesture.velocityTime <= velocityMaxAgeMs
        ? gesture.velocityY
        : 0;
      const distance = gesture.mode === "open"
        ? gesture.offsetPx
        : gesture.targetOffsetPx - gesture.offsetPx;
      const distanceThreshold = Math.min(84, gesture.targetOffsetPx * 0.35);
      const velocityCommits = gesture.mode === "open"
        ? recentVelocity >= velocityThreshold
        : recentVelocity <= -velocityThreshold;
      finish(
        distance >= distanceThreshold || velocityCommits,
        recentVelocity
      );
    };

    const onTouchCancel = () => finish(false);
    deck.addEventListener("touchstart", onTouchStart, { passive: true });
    deck.addEventListener("touchmove", onTouchMove, { passive: false });
    deck.addEventListener("touchend", onTouchEnd, { passive: true });
    deck.addEventListener("touchcancel", onTouchCancel, { passive: true });
    return () => {
      animationRef.current?.stop();
      animationRef.current = null;
      gestureRef.current = null;
      settlingRef.current = false;
      deck.style.removeProperty("transition");
      deck.style.removeProperty("transform");
      deck.removeEventListener("touchstart", onTouchStart);
      deck.removeEventListener("touchmove", onTouchMove);
      deck.removeEventListener("touchend", onTouchEnd);
      deck.removeEventListener("touchcancel", onTouchCancel);
    };
  }, [closeNav, deckRef, enabled, finishSettlement, openNav]);

  return {
    dragging: snapshot.dragging,
    visible: snapshot.dragging || snapshot.settling
  };
}

function mobileViewport() {
  return window.matchMedia("(max-width: 760px)").matches;
}

function ignoresPullGesture(target: EventTarget | null) {
  return target instanceof Element
    && Boolean(target.closest("input, textarea, select, [contenteditable='true'], [role='slider']"));
}

function nearestScrollOwner(target: EventTarget | null, boundary: HTMLElement) {
  let element = target instanceof Element ? target : boundary;
  while (element instanceof HTMLElement) {
    const overflowY = window.getComputedStyle(element).overflowY;
    if (/(auto|scroll)/.test(overflowY) && element.scrollHeight > element.clientHeight + 1) {
      return element;
    }
    if (element === boundary) break;
    element = element.parentElement ?? boundary;
  }
  return null;
}

function mobileRevealOffset(deck: HTMLElement) {
  const root = deck.closest<HTMLElement>("[data-slot='shell-root']");
  const style = window.getComputedStyle(deck);
  const deckSliver = Number.parseFloat(style.getPropertyValue("--spacing-12")) || 48;
  const maximum = Math.max(0, deck.clientHeight - deckSliver);
  const measured = root
    ? Number.parseFloat(root.style.getPropertyValue(mobileMenuRevealHeightProperty))
    : Number.NaN;
  return Number.isFinite(measured) && measured > 0
    ? Math.min(measured, maximum)
    : maximum;
}
