import React from "react";

export type SettingsSection =
  | "providers"
  | "agents"
  | "mcps"
  | "trusted-identities"
  | "approvals"
  | "audit";

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
  if (pathname === "/settings" || pathname === "/settings/providers") {
    return { kind: "settings", section: "providers" };
  }
  if (pathname === "/settings/agents") {
    return { kind: "settings", section: "agents" };
  }
  if (pathname === "/settings/mcps") {
    return { kind: "settings", section: "mcps" };
  }
  if (pathname === "/settings/trusted-identities") {
    return { kind: "settings", section: "trusted-identities" };
  }
  if (pathname === "/settings/approvals") {
    return { kind: "settings", section: "approvals" };
  }
  if (pathname === "/settings/audit") {
    return { kind: "settings", section: "audit" };
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
    return `/settings/${route.section}`;
  }
  return "/";
}

export function shouldRememberAsPreviousAppRoute(
  route: AppRoute
): route is NonSettingsAppRoute {
  return route.kind !== "settings";
}

export function settingsFallbackRoute(route: NonSettingsAppRoute | null): NonSettingsAppRoute {
  return route ?? { kind: "chat" };
}

export function useBrowserRoute() {
  const [route, setRoute] = React.useState(() => routeFromPathname(window.location.pathname));
  const previousAppRouteRef = React.useRef<NonSettingsAppRoute>(
    shouldRememberAsPreviousAppRoute(route) ? route : { kind: "chat" }
  );

  React.useEffect(() => {
    const onPopState = () => {
      const nextRoute = routeFromPathname(window.location.pathname);
      if (shouldRememberAsPreviousAppRoute(nextRoute)) {
        previousAppRouteRef.current = nextRoute;
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
      const nextPath = pathForRoute(nextRoute);
      window.history.pushState({}, "", nextPath);
      return routeFromPathname(nextPath);
    });
  }, []);

  const closeSettings = React.useCallback(() => {
    const nextRoute = settingsFallbackRoute(previousAppRouteRef.current);
    const nextPath = pathForRoute(nextRoute);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);

  return {
    route,
    navigate,
    closeSettings
  };
}
