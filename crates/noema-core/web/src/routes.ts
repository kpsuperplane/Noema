import React from "react";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "memory_home" }
  | { kind: "memory_graph" }
  | { kind: "not_found"; path: string };

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/" || pathname === "/chat") {
    return { kind: "chat" };
  }
  if (pathname === "/memory") {
    return { kind: "memory_home" };
  }
  if (pathname === "/memory/graph") {
    return { kind: "memory_graph" };
  }
  return { kind: "not_found", path: pathname };
}

export function pathForRoute(route: Exclude<AppRoute, { kind: "not_found" }>): string {
  if (route.kind === "chat") {
    return "/";
  }
  if (route.kind === "memory_home") {
    return "/memory";
  }
  return "/memory/graph";
}

export function useBrowserRoute() {
  const [route, setRoute] = React.useState(() => routeFromPathname(window.location.pathname));

  React.useEffect(() => {
    const onPopState = () => setRoute(routeFromPathname(window.location.pathname));
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  const navigate = React.useCallback((nextRoute: Exclude<AppRoute, { kind: "not_found" }>) => {
    const nextPath = pathForRoute(nextRoute);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);

  return { route, navigate };
}
