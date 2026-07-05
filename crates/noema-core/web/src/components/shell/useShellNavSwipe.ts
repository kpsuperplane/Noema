import React from "react";

const shellNavSwipeIntentPx = 8;
const shellNavSwipeVelocityThreshold = 0.45;
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
  offsetPx: number;
  captured: boolean;
};

type ShellNavSwipeSnapshot = {
  dragging: boolean;
  offsetPx: number;
};

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function shellNavSwipeDeckOffsetPx() {
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
  navOpen: boolean
) {
  if (!event.isPrimary || event.button !== 0 || event.pointerType === "mouse") {
    return false;
  }

  if (!shellNavSwipeEnabled()) {
    return false;
  }

  return navOpen || targetAllowsShellNavSwipe(event.target);
}

function shellNavSwipeOffset(mode: ShellNavSwipeMode, deltaX: number) {
  const maxOffset = shellNavSwipeDeckOffsetPx();
  return mode === "open"
    ? clamp(deltaX, 0, maxOffset)
    : clamp(deltaX, -maxOffset, 0);
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
  const threshold = Math.min(84, shellNavSwipeDeckOffsetPx() * 0.35);

  if (mode === "open") {
    return offsetPx >= threshold || velocityX >= shellNavSwipeVelocityThreshold;
  }

  return Math.abs(offsetPx) >= threshold || velocityX <= -shellNavSwipeVelocityThreshold;
}

export function useShellNavSwipe({
  navOpen,
  openNav,
  closeNav
}: {
  navOpen: boolean;
  openNav: () => void;
  closeNav: () => void;
}) {
  const dragRef = React.useRef<ShellNavSwipeDrag | null>(null);
  const suppressClickRef = React.useRef(false);
  const [snapshot, setSnapshot] = React.useState<ShellNavSwipeSnapshot>({
    dragging: false,
    offsetPx: 0
  });

  const clearDrag = React.useCallback(() => {
    dragRef.current = null;
    setSnapshot({ dragging: false, offsetPx: 0 });
  }, []);

  const onPointerDown = React.useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      if (!shouldStartShellNavSwipe(event, navOpen)) {
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
        offsetPx: 0,
        captured: false
      };
    },
    [navOpen]
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
      if (absDeltaY > absDeltaX + shellNavSwipeIntentPx) {
        clearDrag();
        return;
      }

      const movingTowardMenu = drag.mode === "open" ? deltaX > 0 : deltaX < 0;
      if (movingTowardMenu && absDeltaX > absDeltaY + shellNavSwipeIntentPx) {
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.captured = true;
      } else {
        return;
      }
    }

    event.preventDefault();

    const now = typeof performance === "undefined" ? Date.now() : performance.now();
    drag.lastX = event.clientX;
    drag.lastTime = now;
    drag.offsetPx = shellNavSwipeOffset(drag.mode, deltaX);
    setSnapshot({ dragging: true, offsetPx: drag.offsetPx });
  }, [clearDrag]);

  const finishDrag = React.useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) {
        return;
      }

      const now = typeof performance === "undefined" ? Date.now() : performance.now();
      const elapsed = Math.max(1, now - drag.lastTime);
      const velocityX = (event.clientX - drag.lastX) / elapsed;
      const captured = drag.captured;
      const commit = shouldCommitShellNavSwipe({
        mode: drag.mode,
        offsetPx: drag.offsetPx,
        velocityX
      });

      if (captured && event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }

      dragRef.current = null;
      setSnapshot({ dragging: false, offsetPx: 0 });

      if (captured) {
        suppressClickRef.current = true;
        window.setTimeout(() => {
          suppressClickRef.current = false;
        }, 0);
      }

      if (!commit) {
        return;
      }

      if (drag.mode === "open") {
        openNav();
      } else {
        closeNav();
      }
    },
    [closeNav, openNav]
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
      clearDrag();
    },
    [clearDrag]
  );

  return {
    dragging: snapshot.dragging,
    offsetPx: snapshot.offsetPx,
    pointerHandlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp: finishDrag,
      onPointerCancel,
      onClickCapture
    }
  };
}
