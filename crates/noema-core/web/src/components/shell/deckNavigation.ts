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
  | { type: "toggleSidebarCollapsed" };

export const initialDeckNavigationState: DeckNavigationState = {
  navOpen: false,
  sidebarCollapsed: false
};

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
      onNavigate(nextRoute);
      dispatch({ type: "navigate" });
    },
    [onNavigate]
  );

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
