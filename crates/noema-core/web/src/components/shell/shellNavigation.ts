import {
  ArrowLeft,
  Bot,
  Brain,
  CheckSquare,
  Fingerprint,
  House,
  PlugZap,
  ServerCog,
  Settings
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { AppRoute, SettingsSection } from "@/app/routes";

export type ShellMenuLevelId = "l0" | "settings";

export type ShellMenuItemId =
  | "home"
  | "memory"
  | "settings"
  | "settings.providers"
  | "settings.agents"
  | "settings.mcps"
  | "settings.trusted-identities"
  | "settings.approvals"
  | "settings.go-back";

export type ShellMenuSelectionBehavior = "close-reveal" | "keep-reveal-open";

export type ShellMenuItem = {
  itemId: ShellMenuItemId;
  label: string;
  route?: AppRoute;
  action: "navigate" | "goBackFromSettings";
  icon: LucideIcon;
};

export type ShellMenuLevel = {
  levelId: ShellMenuLevelId;
  ariaLabel: string;
  title: string;
  activeItemId: ShellMenuItemId;
  items: ShellMenuItem[];
  bottomItem: ShellMenuItem;
  supportsAttention: boolean;
};

export type ShellBreadcrumb =
  | { current: string; parent?: undefined }
  | { parent: string; current: string };

export type ShellSettingsSection = {
  section: SettingsSection;
  itemId: Extract<ShellMenuItemId, `settings.${string}`>;
  label: string;
  icon: LucideIcon;
};

export const shellSettingsSections: ShellSettingsSection[] = [
  { section: "providers", itemId: "settings.providers", label: "Providers", icon: ServerCog },
  { section: "agents", itemId: "settings.agents", label: "Agents", icon: Bot },
  { section: "mcps", itemId: "settings.mcps", label: "MCPs", icon: PlugZap },
  {
    section: "trusted-identities",
    itemId: "settings.trusted-identities",
    label: "Trusted identities",
    icon: Fingerprint
  },
  { section: "approvals", itemId: "settings.approvals", label: "Approvals", icon: CheckSquare }
];

export function activeL0ItemId(route: AppRoute): Extract<ShellMenuItemId, "home" | "memory"> {
  if (route.kind === "memory_home" || route.kind === "memory_graph") {
    return "memory";
  }
  return "home";
}

export function settingsItemIdForSection(section: SettingsSection): ShellMenuItemId {
  return `settings.${section}` as ShellMenuItemId;
}

export function shellMenuLevelForRoute(route: AppRoute): ShellMenuLevel {
  if (route.kind === "settings") {
    return {
      levelId: "settings",
      ariaLabel: "Settings",
      title: "Settings",
      activeItemId: settingsItemIdForSection(route.section),
      items: shellSettingsSections.map((item) => ({
        itemId: item.itemId,
        label: item.label,
        route: { kind: "settings", section: item.section },
        action: "navigate",
        icon: item.icon
      })),
      bottomItem: {
        itemId: "settings.go-back",
        label: "Go back",
        action: "goBackFromSettings",
        icon: ArrowLeft
      },
      supportsAttention: false
    };
  }

  return {
    levelId: "l0",
    ariaLabel: "Primary",
    title: "Noema",
    activeItemId: activeL0ItemId(route),
    items: [
      { itemId: "home", label: "Home", route: { kind: "chat" }, action: "navigate", icon: House },
      {
        itemId: "memory",
        label: "Memory",
        route: { kind: "memory_home" },
        action: "navigate",
        icon: Brain
      }
    ],
    bottomItem: {
      itemId: "settings",
      label: "Settings",
      route: { kind: "settings", section: "providers" },
      action: "navigate",
      icon: Settings
    },
    supportsAttention: true
  };
}

export function breadcrumbForRoute(route: AppRoute): ShellBreadcrumb {
  if (route.kind === "settings") {
    const current =
      shellSettingsSections.find((section) => section.section === route.section)?.label ??
      "Providers";
    return { parent: "Settings", current };
  }

  if (route.kind === "memory_home" || route.kind === "memory_graph") {
    return { current: "Memory" };
  }

  return { current: "Home" };
}

export function shellMenuSelectionBehavior(
  itemId: ShellMenuItemId
): ShellMenuSelectionBehavior {
  return itemId === "settings" ? "keep-reveal-open" : "close-reveal";
}
