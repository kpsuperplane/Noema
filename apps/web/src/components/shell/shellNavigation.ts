import {
  Bot,
  Brain,
  Cpu,
  Gauge,
  Globe,
  MessageCircle,
  PlugZap,
  ServerCog,
  ShieldCheck,
  SquareKanban
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { AppRoute, NonSettingsAppRoute, SettingsSection } from "@/app/routes";

export type ShellMenuLevelId = "settings";

export type ShellMenuItemId =
  | "chat"
  | "work"
  | "memory"
  | "settings"
  | "settings.agents"
  | "settings.models"
  | "settings.memory"
  | "settings.tools.web"
  | "settings.tools.mcps"
  | "settings.safety.privacy"
  | "settings.safety.usage"
  | "settings.system.providers";

export type ShellMenuItem = {
  itemId: ShellMenuItemId;
  label: string;
  route?: AppRoute;
  icon: LucideIcon;
};

export type ShellMenuGroupLabel = {
  kind: "group";
  label: string;
};

export type ShellMenuItemEntry = {
  kind: "item";
  item: ShellMenuItem;
};

export type ShellMenuEntry = ShellMenuGroupLabel | ShellMenuItemEntry;

export type ShellMenuLevel = {
  levelId: ShellMenuLevelId;
  ariaLabel: string;
  title: string;
  activeItemId: ShellMenuItemId;
  items: ShellMenuEntry[];
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

export type ShellSettingsEntry =
  | { kind: "section"; item: ShellSettingsSection }
  | { kind: "group"; label: string };

export const shellSettingsEntries: ShellSettingsEntry[] = [
  {
    kind: "section",
    item: { section: "agents", itemId: "settings.agents", label: "Agents", icon: Bot }
  },
  {
    kind: "section",
    item: { section: "memory", itemId: "settings.memory", label: "Memory", icon: Brain }
  },
  { kind: "group", label: "Tools" },
  {
    kind: "section",
    item: { section: "tools-web", itemId: "settings.tools.web", label: "Web", icon: Globe }
  },
  {
    kind: "section",
    item: { section: "tools-mcps", itemId: "settings.tools.mcps", label: "MCPs", icon: PlugZap }
  },
  { kind: "group", label: "Safety" },
  {
    kind: "section",
    item: {
      section: "safety-privacy",
      itemId: "settings.safety.privacy",
      label: "Privacy",
      icon: ShieldCheck
    }
  },
  {
    kind: "section",
    item: {
      section: "safety-usage",
      itemId: "settings.safety.usage",
      label: "Usage",
      icon: Gauge
    }
  },
  { kind: "group", label: "System" },
  {
    kind: "section",
    item: {
      section: "models",
      itemId: "settings.models",
      label: "Local Models",
      icon: Cpu
    }
  },
  {
    kind: "section",
    item: {
      section: "system-providers",
      itemId: "settings.system.providers",
      label: "Providers",
      icon: ServerCog
    }
  }
];

export const shellSettingsSections = shellSettingsEntries.flatMap((entry) =>
  entry.kind === "section" ? [entry.item] : []
);

export function activeL0ItemId(
  route: NonSettingsAppRoute
): Extract<ShellMenuItemId, "chat" | "work" | "memory"> {
  if (route.kind === "work") {
    return "work";
  }
  return route.kind === "memory" ? "memory" : "chat";
}

export const shellPrimaryItems: ShellMenuItem[] = [
  {
    itemId: "chat",
    label: "Chat",
    route: { kind: "chat" },
    icon: MessageCircle
  },
  {
    itemId: "work",
    label: "Work",
    route: { kind: "work" },
    icon: SquareKanban
  },
  {
    itemId: "memory",
    label: "Memory",
    route: { kind: "memory" },
    icon: Brain
  }
];

export function settingsItemIdForSection(section: SettingsSection): ShellMenuItemId {
  return (
    shellSettingsSections.find((candidate) => candidate.section === section)?.itemId ??
    "settings.agents"
  );
}

export function shellMenuLevelForRoute(
  route: Extract<AppRoute, { kind: "settings" }>
): ShellMenuLevel {
  return {
    levelId: "settings",
    ariaLabel: "Settings",
    title: "Settings",
    activeItemId: settingsItemIdForSection(route.section),
    items: shellSettingsEntries.map((entry) => {
      if (entry.kind === "group") {
        return { kind: "group", label: entry.label };
      }
      return {
        kind: "item",
        item: {
          itemId: entry.item.itemId,
          label: entry.item.label,
          route: { kind: "settings", section: entry.item.section },
          icon: entry.item.icon
        }
      };
    })
  };
}

export function breadcrumbForRoute(route: AppRoute): ShellBreadcrumb {
  if (route.kind === "settings") {
    const current =
      shellSettingsSections.find((section) => section.section === route.section)?.label ??
      "Agents";
    return { parent: "Settings", current };
  }

  if (route.kind === "work") {
    return { current: "Work" };
  }
  return { current: route.kind === "memory" ? "Memory" : "Chat" };
}
