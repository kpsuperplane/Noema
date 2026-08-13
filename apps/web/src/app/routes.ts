export type SettingsSection =
  | "agents"
  | "models"
  | "memory"
  | "tools-web"
  | "tools-apis"
  | "tools-mcps"
  | "safety-privacy"
  | "safety-usage"
  | "system-providers"
  | "system-notifications"
  | "system-desktop"
  | "system-clients";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "tasks"; projectId?: string }
  | { kind: "memory" }
  | { kind: "settings"; section: SettingsSection };

export type NonSettingsAppRoute = Exclude<AppRoute, { kind: "settings" }>;

export type AppPath =
  | "/"
  | "/tasks"
  | "/memory"
  | "/settings/agents"
  | "/settings/models"
  | "/settings/memory"
  | "/settings/tools/web"
  | "/settings/tools/apis"
  | "/settings/tools/mcps"
  | "/settings/safety/privacy"
  | "/settings/safety/usage"
  | "/settings/system/providers"
  | "/settings/system/notifications"
  | "/settings/system/desktop"
  | "/settings/system/clients";

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/tasks" || pathname.startsWith("/tasks/")) {
    return { kind: "tasks" };
  }
  if (pathname === "/memory" || pathname.startsWith("/memory/")) {
    return { kind: "memory" };
  }
  if (pathname === "/settings/memory") {
    return { kind: "settings", section: "memory" };
  }
  if (
    pathname === "/settings/models" ||
    pathname.startsWith("/settings/models/")
  ) {
    return { kind: "settings", section: "models" };
  }
  if (pathname === "/settings" || pathname === "/settings/agents") {
    return { kind: "settings", section: "agents" };
  }
  if (pathname === "/settings/tools/web") {
    return { kind: "settings", section: "tools-web" };
  }
  if (
    pathname === "/settings/tools/apis" ||
    pathname.startsWith("/settings/tools/apis/")
  ) {
    return { kind: "settings", section: "tools-apis" };
  }
  if (
    pathname === "/settings/tools/mcps" ||
    pathname.startsWith("/settings/tools/mcps/")
  ) {
    return { kind: "settings", section: "tools-mcps" };
  }
  if (pathname === "/settings/safety/privacy") {
    return { kind: "settings", section: "safety-privacy" };
  }
  if (pathname === "/settings/safety/usage") {
    return { kind: "settings", section: "safety-usage" };
  }
  if (
    pathname === "/settings/system/providers" ||
    pathname.startsWith("/settings/system/providers/")
  ) {
    return { kind: "settings", section: "system-providers" };
  }
  if (pathname === "/settings/system/notifications") {
    return { kind: "settings", section: "system-notifications" };
  }
  if (pathname === "/settings/system/desktop") {
    return { kind: "settings", section: "system-desktop" };
  }
  if (
    pathname === "/settings/system/clients" ||
    pathname.startsWith("/settings/system/clients/")
  ) {
    return { kind: "settings", section: "system-clients" };
  }
  return { kind: "chat" };
}

export function memoryPageUrlPath(pagePath: string): string {
  return pagePath.endsWith(".md") ? pagePath.slice(0, -3) : pagePath;
}

export function memoryPagePathFromUrl(urlPath: string): string {
  return `${urlPath}.md`;
}

export function pathForRoute(route: AppRoute): AppPath {
  if (route.kind === "tasks") {
    return "/tasks";
  }
  if (route.kind === "memory") {
    return "/memory";
  }
  if (route.kind === "settings") {
    switch (route.section) {
      case "agents":
        return "/settings/agents";
      case "models":
        return "/settings/models";
      case "memory":
        return "/settings/memory";
      case "tools-web":
        return "/settings/tools/web";
      case "tools-apis":
        return "/settings/tools/apis";
      case "tools-mcps":
        return "/settings/tools/mcps";
      case "safety-privacy":
        return "/settings/safety/privacy";
      case "safety-usage":
        return "/settings/safety/usage";
      case "system-providers":
        return "/settings/system/providers";
      case "system-notifications":
        return "/settings/system/notifications";
      case "system-desktop":
        return "/settings/system/desktop";
      case "system-clients":
        return "/settings/system/clients";
    }
  }
  return "/";
}

export function hrefForRoute(route: AppRoute): string {
  const path = pathForRoute(route);
  if (route.kind !== "tasks" || !route.projectId) return path;
  return `${path}?project=${encodeURIComponent(route.projectId)}`;
}

export function pageSurfaceKeyForPathname(pathname: string): string {
  const route = routeFromPathname(pathname);
  return route.kind === "memory" ? pathname : pathForRoute(route);
}

export function pathnamesSharePageSurface(
  currentPathname: string,
  nextPathname: string,
): boolean {
  return (
    pageSurfaceKeyForPathname(currentPathname) ===
    pageSurfaceKeyForPathname(nextPathname)
  );
}

export function shouldReplaceHistoryEntryForNavigation(
  currentRoute: AppRoute,
  nextRoute: AppRoute,
) {
  return currentRoute.kind === "settings" && nextRoute.kind === "settings";
}
