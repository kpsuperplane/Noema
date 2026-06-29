import React from "react";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "memory_home" }
  | { kind: "memory_graph" };

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/memory") {
    return { kind: "memory_home" };
  }
  if (pathname === "/memory/graph") {
    return { kind: "memory_graph" };
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
  return "/";
}

export function useBrowserRoute() {
  const [route, setRoute] = React.useState(() => routeFromPathname(window.location.pathname));

  React.useEffect(() => {
    const onPopState = () => setRoute(routeFromPathname(window.location.pathname));
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  const navigate = React.useCallback((nextRoute: AppRoute) => {
    const nextPath = pathForRoute(nextRoute);
    window.history.pushState({}, "", nextPath);
    setRoute(routeFromPathname(nextPath));
  }, []);

  return { route, navigate };
}
