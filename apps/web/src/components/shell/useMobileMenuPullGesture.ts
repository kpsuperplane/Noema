import React from "react";
import { animate, type AnimationPlaybackControls } from "motion/react";
import type { AppRoute } from "@/app/routes";
import { springs } from "@/motion/springs";
import { shouldAnimateDeckNavigation } from "./deckNavigation";
import { mobileMenuRevealHeightProperty } from "./useMobileMenuRevealHeight";

const pullActivationPx = 8;
const verticalDominance = 1.2;
const horizontalDominance = 1.2;
const velocityThreshold = 0.45;
const velocityMaxAgeMs = 120;
type PullGesture = {
  mode: "pending" | "open" | "close" | "previous" | "next";
  startX: number;
  startY: number;
  lastX: number;
  lastY: number;
  lastTime: number;
  velocity: number;
  velocityTime: number;
  offsetPx: number;
  targetOffsetPx: number;
  verticalScrollOwner: HTMLElement | null;
  horizontalScrollOwner: HTMLElement | null;
  previousRoute: AppRoute | null;
  nextRoute: AppRoute | null;
  captured: boolean;
};

export function useMobileMenuPullGesture({
  deckRef,
  menuEnabled,
  closeNav,
  adjacentRoutes,
  navOpen,
  onNavigateTab,
  onNavigationSettled,
  openNav
}: {
  deckRef: React.RefObject<HTMLElement | null>;
  menuEnabled: boolean;
  closeNav: () => void;
  adjacentRoutes: { previous: AppRoute | null; next: AppRoute | null };
  navOpen: boolean;
  onNavigateTab: (route: AppRoute) => void;
  onNavigationSettled: () => void;
  openNav: () => void;
}) {
  const navOpenRef = React.useRef(navOpen);
  const menuEnabledRef = React.useRef(menuEnabled);
  const adjacentRoutesRef = React.useRef(adjacentRoutes);
  const onNavigateTabRef = React.useRef(onNavigateTab);
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
    menuEnabledRef.current = menuEnabled;
    adjacentRoutesRef.current = adjacentRoutes;
    onNavigateTabRef.current = onNavigateTab;
  }, [adjacentRoutes, menuEnabled, navOpen, onNavigateTab]);

  React.useEffect(() => {
    const deck = deckRef.current;
    if (!deck) return;

    const finish = (commit: boolean, velocityY = 0) => {
      const gesture = gestureRef.current;
      gestureRef.current = null;
      if (!gesture?.captured || (gesture.mode !== "open" && gesture.mode !== "close")) return;

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

      const menuMode = navOpenRef.current ? "close" : "open";
      const verticalScrollOwner = menuMode === "open"
        ? nearestScrollOwner(event.target, deck)
        : null;
      const routes = adjacentRoutesRef.current;
      if (
        !navOpenRef.current
        && !menuEnabledRef.current
        && !routes.previous
        && !routes.next
      ) return;
      const touch = event.touches[0];
      const now = performance.now();
      gestureRef.current = {
        mode: "pending",
        startX: touch.clientX,
        startY: touch.clientY,
        lastX: touch.clientX,
        lastY: touch.clientY,
        lastTime: now,
        velocity: 0,
        velocityTime: now,
        offsetPx: menuMode === "open" ? 0 : mobileRevealOffset(deck),
        targetOffsetPx: mobileRevealOffset(deck),
        verticalScrollOwner,
        horizontalScrollOwner: nearestHorizontalScrollOwner(event.target, deck),
        previousRoute: routes.previous,
        nextRoute: routes.next,
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
        if (
          !navOpenRef.current
          && Math.abs(deltaX) >= Math.abs(deltaY) * horizontalDominance
        ) {
          const direction = deltaX > 0 ? "previous" : "next";
          if (
            gesture.horizontalScrollOwner
            || !(direction === "previous" ? gesture.previousRoute : gesture.nextRoute)
          ) {
            gestureRef.current = null;
            return;
          }
          gesture.mode = direction;
          gesture.captured = true;
        } else {
          const mode = navOpenRef.current ? "close" : "open";
          const movingTowardMenuState = mode === "open" ? deltaY > 0 : deltaY < 0;
          if (
            !menuEnabledRef.current
            || !movingTowardMenuState
            || Math.abs(deltaY) < Math.abs(deltaX) * verticalDominance
            || (mode === "open" && (gesture.verticalScrollOwner?.scrollTop ?? 0) > 0.5)
          ) {
            gestureRef.current = null;
            return;
          }
          gesture.mode = mode;
          gesture.captured = true;
          deck.style.setProperty("transition", "none");
          deck.style.setProperty("transform", `translateY(${gesture.offsetPx}px)`);
          setSnapshot({ dragging: true, settling: false });
        }
      }

      event.preventDefault();
      const now = performance.now();
      const elapsed = Math.max(1, now - gesture.lastTime);
      gesture.velocity = gesture.mode === "open" || gesture.mode === "close"
        ? (touch.clientY - gesture.lastY) / elapsed
        : (touch.clientX - gesture.lastX) / elapsed;
      gesture.velocityTime = now;
      gesture.lastX = touch.clientX;
      gesture.lastY = touch.clientY;
      gesture.lastTime = now;
      if (gesture.mode === "previous" || gesture.mode === "next") return;
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
      if (gesture.mode === "previous" || gesture.mode === "next") {
        gestureRef.current = null;
        const velocityX = performance.now() - gesture.velocityTime <= velocityMaxAgeMs
          ? gesture.velocity
          : 0;
        const distance = Math.abs(gesture.lastX - gesture.startX);
        const distanceThreshold = Math.min(84, deck.clientWidth * 0.22);
        const velocityCommits = gesture.mode === "previous"
          ? velocityX >= velocityThreshold
          : velocityX <= -velocityThreshold;
        if (distance >= distanceThreshold || velocityCommits) {
          const target = gesture.mode === "previous" ? gesture.previousRoute : gesture.nextRoute;
          if (target) onNavigateTabRef.current(target);
        }
        return;
      }
      const recentVelocity = performance.now() - gesture.velocityTime <= velocityMaxAgeMs
        ? gesture.velocity
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

    const onTouchCancel = () => {
      if (gestureRef.current?.mode === "previous" || gestureRef.current?.mode === "next") {
        gestureRef.current = null;
        return;
      }
      finish(false);
    };
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
  }, [closeNav, deckRef, finishSettlement, openNav]);

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

function nearestHorizontalScrollOwner(target: EventTarget | null, boundary: HTMLElement) {
  let element = target instanceof Element ? target : boundary;
  while (element instanceof HTMLElement) {
    const overflowX = window.getComputedStyle(element).overflowX;
    if (/(auto|scroll)/.test(overflowX) && element.scrollWidth > element.clientWidth + 1) {
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
