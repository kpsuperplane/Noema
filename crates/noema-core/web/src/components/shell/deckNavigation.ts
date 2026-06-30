import React from "react";
import type { AppRoute } from "@/routes";
import type { ShellSurfaceVisibility } from "./ShellSurfaceContext";

export type DeckNavigationState = {
  navOpen: boolean;
  sidebarCollapsed: boolean;
  surfaceVisibility: ShellSurfaceVisibility;
};

export type DeckNavigationAction =
  | { type: "openNav"; animated?: boolean }
  | { type: "closeNav"; animated?: boolean }
  | { type: "escape"; animated?: boolean }
  | { type: "navigate"; animated?: boolean }
  | { type: "settleSurfaceVisibility" }
  | { type: "syncWideViewport" }
  | { type: "toggleSidebarCollapsed" };

export const initialDeckNavigationState: DeckNavigationState = {
  navOpen: false,
  sidebarCollapsed: false,
  surfaceVisibility: "visible"
};

function visibilityAfterOpening(animated: boolean | undefined): ShellSurfaceVisibility {
  return animated === false ? "hidden" : "hiding";
}

function visibilityAfterClosing(
  state: DeckNavigationState,
  animated: boolean | undefined
): ShellSurfaceVisibility {
  if (!state.navOpen) {
    return "visible";
  }
  return animated === false ? "visible" : "showing";
}

function settledSurfaceVisibility(visibility: ShellSurfaceVisibility): ShellSurfaceVisibility {
  if (visibility === "hiding") {
    return "hidden";
  }
  if (visibility === "showing") {
    return "visible";
  }
  return visibility;
}

export function deckTransitionPropertyCanSettleSurfaceVisibility(propertyName: string) {
  return propertyName === "left" || propertyName === "right" || propertyName === "transform";
}

export function deckNavigationReducer(
  state: DeckNavigationState,
  action: DeckNavigationAction
): DeckNavigationState {
  switch (action.type) {
    case "openNav":
      return {
        ...state,
        navOpen: true,
        surfaceVisibility: visibilityAfterOpening(action.animated)
      };
    case "closeNav":
    case "escape":
    case "navigate":
      return {
        ...state,
        navOpen: false,
        surfaceVisibility: visibilityAfterClosing(state, action.animated)
      };
    case "settleSurfaceVisibility":
      return {
        ...state,
        surfaceVisibility: settledSurfaceVisibility(state.surfaceVisibility)
      };
    case "syncWideViewport":
      return { ...state, navOpen: false, surfaceVisibility: "visible" };
    case "toggleSidebarCollapsed":
      return {
        navOpen: false,
        sidebarCollapsed: !state.sidebarCollapsed,
        surfaceVisibility: "visible"
      };
  }
}

export function deckNavigationControlLabels(state: DeckNavigationState) {
  return {
    menu: state.navOpen ? "Close navigation" : "Open navigation",
    collapse: state.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"
  };
}

export function shouldAnimateDeckNavigation() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return true;
  }
  return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function useDeckNavigation(onNavigate: (route: AppRoute) => void) {
  const [state, dispatch] = React.useReducer(deckNavigationReducer, initialDeckNavigationState);
  const menuButtonRef = React.useRef<HTMLButtonElement>(null);

  const openNav = React.useCallback(() => {
    dispatch({ type: "openNav", animated: shouldAnimateDeckNavigation() });
  }, []);

  const closeNav = React.useCallback(() => {
    dispatch({ type: "closeNav", animated: shouldAnimateDeckNavigation() });
    menuButtonRef.current?.focus();
  }, []);

  const toggleSidebarCollapsed = React.useCallback(() => {
    dispatch({ type: "toggleSidebarCollapsed" });
  }, []);

  const navigateFromShell = React.useCallback(
    (nextRoute: AppRoute) => {
      onNavigate(nextRoute);
      dispatch({ type: "navigate", animated: shouldAnimateDeckNavigation() });
    },
    [onNavigate]
  );

  const settleSurfaceVisibility = React.useCallback(() => {
    dispatch({ type: "settleSurfaceVisibility" });
  }, []);

  React.useEffect(() => {
    if (!state.navOpen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      dispatch({ type: "escape", animated: shouldAnimateDeckNavigation() });
      menuButtonRef.current?.focus();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [state.navOpen]);

  React.useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      return;
    }

    const wideViewportQuery = window.matchMedia("(min-width: 761px)");
    const syncWideViewport = (query: Pick<MediaQueryList, "matches">) => {
      if (query.matches) {
        dispatch({ type: "syncWideViewport" });
      }
    };

    syncWideViewport(wideViewportQuery);
    wideViewportQuery.addEventListener("change", syncWideViewport);
    return () => wideViewportQuery.removeEventListener("change", syncWideViewport);
  }, []);

  return {
    state,
    labels: deckNavigationControlLabels(state),
    menuButtonRef,
    openNav,
    closeNav,
    toggleSidebarCollapsed,
    navigateFromShell,
    settleSurfaceVisibility
  };
}
