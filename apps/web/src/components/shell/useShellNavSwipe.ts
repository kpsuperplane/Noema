import React from "react";
import {
  animate,
  useReducedMotion,
  type AnimationPlaybackControls,
  type MotionValue
} from "motion/react";
import { springs } from "@/motion/springs";

const shellNavSwipeAxisIntentPx = 10;
const shellNavSwipeHorizontalDominance = 1.35;
const shellNavSwipeVelocityThreshold = 0.45;
const shellNavSwipeVelocityMaxAgeMs = 120;
const shellNavSwipeMaxDeckOffsetPx = 252;
const shellNavSwipeDeckOffsetViewportRatio = 0.72;

type ShellNavSwipeMode = "open" | "close";

type ShellNavSwipeDrag = {
  pointerId: number;
  mode: ShellNavSwipeMode;
  startX: number;
  startY: number;
  lastX: number;
  lastTime: number;
  velocityX: number;
  velocityTime: number;
  offsetPx: number;
  originOffsetPx: number;
  captured: boolean;
};

type ShellNavSwipeSnapshot = {
  dragging: boolean;
  settling: boolean;
};

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

export function shellNavDeckOffsetPx() {
  if (typeof window === "undefined") {
    return shellNavSwipeMaxDeckOffsetPx;
  }

  return Math.min(
    shellNavSwipeMaxDeckOffsetPx,
    window.innerWidth * shellNavSwipeDeckOffsetViewportRatio
  );
}

function shellNavSwipeEnabled() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }

  return window.matchMedia("(max-width: 760px)").matches;
}

function targetAllowsShellNavSwipe(target: EventTarget | null) {
  if (!(target instanceof Element)) {
    return true;
  }

  return !target.closest("input, textarea, select, [contenteditable='true']");
}

function shouldStartShellNavSwipe(
  event: React.PointerEvent<HTMLElement>,
  enabled: boolean
) {
  if (!enabled || !event.isPrimary || event.button !== 0 || event.pointerType === "mouse") {
    return false;
  }

  if (!shellNavSwipeEnabled()) {
    return false;
  }

  return targetAllowsShellNavSwipe(event.target);
}

function shellNavSwipeOffset(offsetPx: number) {
  return clamp(offsetPx, 0, shellNavDeckOffsetPx());
}

function shouldCommitShellNavSwipe({
  mode,
  offsetPx,
  velocityX
}: {
  mode: ShellNavSwipeMode;
  offsetPx: number;
  velocityX: number;
}) {
  const maxOffset = shellNavDeckOffsetPx();
  const threshold = Math.min(84, maxOffset * 0.35);
  const distance = mode === "open" ? offsetPx : maxOffset - offsetPx;

  if (mode === "open") {
    return distance >= threshold || velocityX >= shellNavSwipeVelocityThreshold;
  }

  return distance >= threshold || velocityX <= -shellNavSwipeVelocityThreshold;
}

export function useShellNavSwipe({
  enabled,
  navOpen,
  openNav,
  closeNav,
  deckX,
  onSettled
}: {
  enabled: boolean;
  navOpen: boolean;
  openNav: () => void;
  closeNav: () => void;
  deckX: MotionValue<number>;
  onSettled: () => void;
}) {
  const dragRef = React.useRef<ShellNavSwipeDrag | null>(null);
  const suppressClickRef = React.useRef(false);
  const animationRef = React.useRef<AnimationPlaybackControls | null>(null);
  const gestureCommitRef = React.useRef(false);
  const previousNavOpenRef = React.useRef(navOpen);
  const reduceMotion = useReducedMotion();
  const [snapshot, setSnapshot] = React.useState<ShellNavSwipeSnapshot>({
    dragging: false,
    settling: false
  });

  const clearDrag = React.useCallback(() => {
    dragRef.current = null;
    setSnapshot((current) => ({ ...current, dragging: false }));
  }, []);

  const finishSettlement = React.useCallback(() => {
    animationRef.current = null;
    setSnapshot((current) => ({ ...current, settling: false }));
    onSettled();
  }, [onSettled]);

  const settleDeck = React.useCallback(
    (target: number, velocityX = 0) => {
      animationRef.current?.stop();
      setSnapshot((current) => ({ ...current, settling: true }));
      if (reduceMotion) {
        deckX.set(target);
        finishSettlement();
        return;
      }
      animationRef.current = animate(deckX, target, {
        ...springs.surface,
        velocity: velocityX * 1_000,
        onComplete: finishSettlement
      });
    },
    [deckX, finishSettlement, reduceMotion]
  );

  React.useLayoutEffect(() => {
    if (previousNavOpenRef.current === navOpen) {
      return;
    }
    previousNavOpenRef.current = navOpen;
    if (gestureCommitRef.current) {
      gestureCommitRef.current = false;
      return;
    }
    settleDeck(navOpen ? shellNavDeckOffsetPx() : 0);
  }, [navOpen, settleDeck]);

  React.useEffect(() => () => animationRef.current?.stop(), []);

  const onPointerDown = React.useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      if (!shouldStartShellNavSwipe(event, enabled)) {
        return;
      }

      const now = typeof performance === "undefined" ? Date.now() : performance.now();
      dragRef.current = {
        pointerId: event.pointerId,
        mode: navOpen ? "close" : "open",
        startX: event.clientX,
        startY: event.clientY,
        lastX: event.clientX,
        lastTime: now,
        velocityX: 0,
        velocityTime: now,
        offsetPx: 0,
        originOffsetPx: navOpen ? shellNavDeckOffsetPx() : 0,
        captured: false
      };
    },
    [enabled, navOpen]
  );

  const onPointerMove = React.useCallback((event: React.PointerEvent<HTMLElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) {
      return;
    }

    const deltaX = event.clientX - drag.startX;
    const deltaY = event.clientY - drag.startY;
    const absDeltaX = Math.abs(deltaX);
    const absDeltaY = Math.abs(deltaY);

    if (!drag.captured) {
      if (Math.max(absDeltaX, absDeltaY) < shellNavSwipeAxisIntentPx) {
        return;
      }

      const movingTowardMenu = drag.mode === "open" ? deltaX > 0 : deltaX < 0;
      const clearlyHorizontal = absDeltaX >= absDeltaY * shellNavSwipeHorizontalDominance;
      if (!movingTowardMenu || !clearlyHorizontal) {
        clearDrag();
        return;
      }

      event.currentTarget.setPointerCapture(event.pointerId);
      drag.captured = true;
      animationRef.current?.stop();
      drag.originOffsetPx = deckX.get();
      drag.startX = event.clientX;
      drag.lastX = event.clientX;
      drag.lastTime = typeof performance === "undefined" ? Date.now() : performance.now();
      drag.offsetPx = drag.originOffsetPx;
      setSnapshot({ dragging: true, settling: false });
      return;
    }

    event.preventDefault();

    const now = typeof performance === "undefined" ? Date.now() : performance.now();
    const elapsed = Math.max(1, now - drag.lastTime);
    drag.velocityX = (event.clientX - drag.lastX) / elapsed;
    drag.velocityTime = now;
    drag.lastX = event.clientX;
    drag.lastTime = now;
    drag.offsetPx = shellNavSwipeOffset(drag.originOffsetPx + deltaX);
    deckX.set(drag.offsetPx);
  }, [clearDrag, deckX]);

  const finishDrag = React.useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) {
        return;
      }

      if (!drag.captured) {
        dragRef.current = null;
        return;
      }

      const now = typeof performance === "undefined" ? Date.now() : performance.now();
      const elapsed = Math.max(1, now - drag.lastTime);
      const releaseVelocityX = (event.clientX - drag.lastX) / elapsed;
      const velocityX =
        now - drag.velocityTime <= shellNavSwipeVelocityMaxAgeMs
          ? drag.velocityX
          : releaseVelocityX;
      const commit = shouldCommitShellNavSwipe({
        mode: drag.mode,
        offsetPx: drag.offsetPx,
        velocityX
      });

      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }

      dragRef.current = null;
      setSnapshot((current) => ({ ...current, dragging: false }));

      suppressClickRef.current = true;
      window.setTimeout(() => {
        suppressClickRef.current = false;
      }, 0);

      const nextNavOpen = commit ? drag.mode === "open" : drag.mode === "close";
      const target = nextNavOpen ? shellNavDeckOffsetPx() : 0;
      settleDeck(target, velocityX);

      if (nextNavOpen === navOpen) {
        return;
      }

      gestureCommitRef.current = true;
      if (nextNavOpen) {
        openNav();
      } else {
        closeNav();
      }
    },
    [closeNav, navOpen, openNav, settleDeck]
  );

  const onClickCapture = React.useCallback((event: React.MouseEvent<HTMLElement>) => {
    if (!suppressClickRef.current) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    suppressClickRef.current = false;
  }, []);

  const onPointerCancel = React.useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      const drag = dragRef.current;
      if (drag?.captured && drag.pointerId === event.pointerId && event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
      if (drag?.captured) {
        settleDeck(navOpen ? shellNavDeckOffsetPx() : 0);
      }
      clearDrag();
    },
    [clearDrag, navOpen, settleDeck]
  );

  return {
    active: snapshot.dragging || snapshot.settling,
    dragging: snapshot.dragging,
    pointerHandlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp: finishDrag,
      onPointerCancel,
      onClickCapture
    }
  };
}
