import React from "react";

export type SettingsSection =
  | "agents"
  | "tools-web"
  | "tools-mcps"
  | "safety-usage"
  | "safety-approvals"
  | "safety-identities"
  | "system-providers";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "memory_home" }
  | { kind: "memory_graph" }
  | { kind: "settings"; section: SettingsSection };

export type NonSettingsAppRoute = Exclude<AppRoute, { kind: "settings" }>;

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/memory") {
    return { kind: "memory_home" };
  }
  if (pathname === "/memory/graph") {
    return { kind: "memory_graph" };
  }
  if (pathname === "/settings" || pathname === "/settings/agents") {
    return { kind: "settings", section: "agents" };
  }
  if (pathname === "/settings/tools/web") {
    return { kind: "settings", section: "tools-web" };
  }
  if (pathname === "/settings/tools/mcps") {
    return { kind: "settings", section: "tools-mcps" };
  }
  if (pathname === "/settings/safety/usage") {
    return { kind: "settings", section: "safety-usage" };
  }
  if (pathname === "/settings/safety/approvals") {
    return { kind: "settings", section: "safety-approvals" };
  }
  if (pathname === "/settings/safety/identities") {
    return { kind: "settings", section: "safety-identities" };
  }
  if (pathname === "/settings/system/providers") {
    return { kind: "settings", section: "system-providers" };
  }
  return { kind: "chat" };
}

export function pathForRoute(route: AppRoute): string {
  if (route.kind === "memory_home") {
    return "/memory";
  }
  if (route.kind === "memory_graph") {
    return "/memory/graph";
  }
  if (route.kind === "settings") {
    switch (route.section) {
      case "agents":
        return "/settings/agents";
      case "tools-web":
        return "/settings/tools/web";
      case "tools-mcps":
        return "/settings/tools/mcps";
      case "safety-usage":
        return "/settings/safety/usage";
      case "safety-approvals":
        return "/settings/safety/approvals";
      case "safety-identities":
        return "/settings/safety/identities";
      case "system-providers":
        return "/settings/system/providers";
    }
  }
  return "/";
}

export function shouldRememberAsPreviousAppRoute(
  route: AppRoute
): route is NonSettingsAppRoute {
  return route.kind !== "settings";
}

export type SettingsBackNavigation =
  | { kind: "history-back" }
  | { kind: "navigate"; route: NonSettingsAppRoute };

export function settingsBackNavigation(canUseBrowserHistory: boolean): SettingsBackNavigation {
  if (canUseBrowserHistory) {
    return { kind: "history-back" };
  }
  return { kind: "navigate", route: { kind: "chat" } };
}

export function shouldReplaceHistoryEntryForNavigation(
  currentRoute: AppRoute,
  nextRoute: AppRoute
) {
  return currentRoute.kind === "settings" && nextRoute.kind === "settings";
}

export function useBrowserRoute() {
  const [route, setRoute] = React.useState(() => routeFromPathname(window.location.pathname));
  const previousAppRouteRef = React.useRef<NonSettingsAppRoute>(
    shouldRememberAsPreviousAppRoute(route) ? route : { kind: "chat" }
  );
  const canGoBackFromSettingsRef = React.useRef(false);

  React.useEffect(() => {
    const onPopState = () => {
      const nextRoute = routeFromPathname(window.location.pathname);
      if (shouldRememberAsPreviousAppRoute(nextRoute)) {
        previousAppRouteRef.current = nextRoute;
        canGoBackFromSettingsRef.current = false;
      }
      setRoute(nextRoute);
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  const navigate = React.useCallback((nextRoute: AppRoute) => {
    setRoute((currentRoute) => {
      if (shouldRememberAsPreviousAppRoute(currentRoute)) {
        previousAppRouteRef.current = currentRoute;
      }
      if (nextRoute.kind === "settings") {
        canGoBackFromSettingsRef.current =
          canGoBackFromSettingsRef.current || shouldRememberAsPreviousAppRoute(currentRoute);
      } else {
        canGoBackFromSettingsRef.current = false;
      }
      const nextPath = pathForRoute(nextRoute);
      if (shouldReplaceHistoryEntryForNavigation(currentRoute, nextRoute)) {
        window.history.replaceState({}, "", nextPath);
      } else {
        window.history.pushState({}, "", nextPath);
      }
      return routeFromPathname(nextPath);
    });
  }, []);

  const goBackFromSettings = React.useCallback(() => {
    const action = settingsBackNavigation(canGoBackFromSettingsRef.current);
    canGoBackFromSettingsRef.current = false;

    if (action.kind === "history-back") {
      window.history.back();
      return;
    }

    const nextPath = pathForRoute(action.route);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);

  return {
    route,
    navigate,
    goBackFromSettings
  };
}
