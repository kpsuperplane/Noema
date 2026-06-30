import React from "react";
import type { AppRoute } from "@/routes";

export type DeckNavigationState = {
  navOpen: boolean;
  sidebarCollapsed: boolean;
};

export type DeckNavigationAction =
  | { type: "openNav" }
  | { type: "closeNav" }
  | { type: "escape" }
  | { type: "navigate" }
  | { type: "syncWideViewport" }
  | { type: "toggleSidebarCollapsed" };

export const initialDeckNavigationState: DeckNavigationState = {
  navOpen: false,
  sidebarCollapsed: false
};

export const DECK_NAVIGATION_ROUTE_DEFER_MS = 300;

export function deckNavigationRouteTiming(state: DeckNavigationState) {
  return state.navOpen
    ? {
        deferRouteChange: true,
        delayMs: DECK_NAVIGATION_ROUTE_DEFER_MS
      }
    : {
        deferRouteChange: false,
        delayMs: 0
      };
}

export function deckNavigationReducer(
  state: DeckNavigationState,
  action: DeckNavigationAction
): DeckNavigationState {
  switch (action.type) {
    case "openNav":
      return { ...state, navOpen: true };
    case "closeNav":
    case "escape":
    case "navigate":
    case "syncWideViewport":
      return { ...state, navOpen: false };
    case "toggleSidebarCollapsed":
      return {
        navOpen: false,
        sidebarCollapsed: !state.sidebarCollapsed
      };
  }
}

export function deckNavigationControlLabels(state: DeckNavigationState) {
  return {
    menu: state.navOpen ? "Close navigation" : "Open navigation",
    collapse: state.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"
  };
}

export function useDeckNavigation(onNavigate: (route: AppRoute) => void) {
  const [state, dispatch] = React.useReducer(deckNavigationReducer, initialDeckNavigationState);
  const menuButtonRef = React.useRef<HTMLButtonElement>(null);
  const routeChangeTimeoutRef = React.useRef<number | null>(null);

  const openNav = React.useCallback(() => {
    dispatch({ type: "openNav" });
  }, []);

  const closeNav = React.useCallback(() => {
    dispatch({ type: "closeNav" });
    menuButtonRef.current?.focus();
  }, []);

  const toggleSidebarCollapsed = React.useCallback(() => {
    dispatch({ type: "toggleSidebarCollapsed" });
  }, []);

  const navigateFromShell = React.useCallback(
    (nextRoute: AppRoute) => {
      const routeTiming = deckNavigationRouteTiming(state);
      dispatch({ type: "navigate" });

      if (routeChangeTimeoutRef.current !== null) {
        window.clearTimeout(routeChangeTimeoutRef.current);
        routeChangeTimeoutRef.current = null;
      }

      if (!routeTiming.deferRouteChange) {
        onNavigate(nextRoute);
        return;
      }

      routeChangeTimeoutRef.current = window.setTimeout(() => {
        routeChangeTimeoutRef.current = null;
        onNavigate(nextRoute);
      }, routeTiming.delayMs);
    },
    [onNavigate, state]
  );

  React.useEffect(() => {
    return () => {
      if (routeChangeTimeoutRef.current !== null) {
        window.clearTimeout(routeChangeTimeoutRef.current);
      }
    };
  }, []);

  React.useEffect(() => {
    if (!state.navOpen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      dispatch({ type: "escape" });
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
    navigateFromShell
  };
}
