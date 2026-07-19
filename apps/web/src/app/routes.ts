export type SettingsSection =
  | "agents"
  | "models"
  | "memory"
  | "tools-web"
  | "tools-mcps"
  | "safety-privacy"
  | "safety-usage"
  | "system-providers";

export type AppRoute =
  | { kind: "chat" }
  | { kind: "work" }
  | { kind: "memory" }
  | { kind: "settings"; section: SettingsSection };

export type NonSettingsAppRoute = Exclude<AppRoute, { kind: "settings" }>;

export type AppPath =
  | "/"
  | "/work"
  | "/memory"
  | "/settings/agents"
  | "/settings/models"
  | "/settings/memory"
  | "/settings/tools/web"
  | "/settings/tools/mcps"
  | "/settings/safety/privacy"
  | "/settings/safety/usage"
  | "/settings/system/providers";

export function routeFromPathname(pathname: string): AppRoute {
  if (pathname === "/work" || pathname.startsWith("/work/")) {
    return { kind: "work" };
  }
  if (pathname === "/memory") {
    return { kind: "memory" };
  }
  if (pathname === "/settings/memory") {
    return { kind: "settings", section: "memory" };
  }
  if (pathname === "/settings/models") {
    return { kind: "settings", section: "models" };
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
  if (pathname === "/settings/safety/privacy") {
    return { kind: "settings", section: "safety-privacy" };
  }
  if (pathname === "/settings/safety/usage") {
    return { kind: "settings", section: "safety-usage" };
  }
  if (pathname === "/settings/system/providers") {
    return { kind: "settings", section: "system-providers" };
  }
  return { kind: "chat" };
}

export function pathForRoute(route: AppRoute): AppPath {
  if (route.kind === "work") {
    return "/work";
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
      case "tools-mcps":
        return "/settings/tools/mcps";
      case "safety-privacy":
        return "/settings/safety/privacy";
      case "safety-usage":
        return "/settings/safety/usage";
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
