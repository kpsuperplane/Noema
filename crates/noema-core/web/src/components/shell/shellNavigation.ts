import {
  ArrowLeft,
  Bot,
  Brain,
  CheckSquare,
  Fingerprint,
  Gauge,
  Globe,
  House,
  PlugZap,
  ServerCog,
  Settings
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { AppRoute, NonSettingsAppRoute, SettingsSection } from "@/app/routes";

export type ShellMenuLevelId = "l0" | "settings";

export type ShellMenuItemId =
  | "home"
  | "memory"
  | "settings"
  | "settings.agents"
  | "settings.memory"
  | "settings.tools.web"
  | "settings.tools.mcps"
  | "settings.safety.usage"
  | "settings.safety.approvals"
  | "settings.safety.identities"
  | "settings.system.providers"
  | "settings.go-back";

export type ShellMenuSelectionBehavior = "close-reveal" | "keep-reveal-open";

export type ShellMenuItem = {
  itemId: ShellMenuItemId;
  label: string;
  route?: AppRoute;
  action: "navigate" | "goBackFromSettings";
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
      section: "safety-usage",
      itemId: "settings.safety.usage",
      label: "Usage",
      icon: Gauge
    }
  },
  {
    kind: "section",
    item: {
      section: "safety-approvals",
      itemId: "settings.safety.approvals",
      label: "Approvals",
      icon: CheckSquare
    }
  },
  {
    kind: "section",
    item: {
      section: "safety-identities",
      itemId: "settings.safety.identities",
      label: "Identities",
      icon: Fingerprint
    }
  },
  { kind: "group", label: "System" },
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
): Extract<ShellMenuItemId, "home" | "memory"> {
  return route.kind === "memory" ? "memory" : "home";
}

export function settingsItemIdForSection(section: SettingsSection): ShellMenuItemId {
  return (
    shellSettingsSections.find((candidate) => candidate.section === section)?.itemId ??
    "settings.agents"
  );
}

export function shellMenuLevelForRoute(route: AppRoute): ShellMenuLevel {
  if (route.kind === "settings") {
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
            action: "navigate",
            icon: entry.item.icon
          }
        };
      }),
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
      {
        kind: "item",
        item: {
          itemId: "home",
          label: "Home",
          route: { kind: "chat" },
          action: "navigate",
          icon: House
        }
      },
      {
        kind: "item",
        item: {
          itemId: "memory",
          label: "Memory",
          route: { kind: "memory" },
          action: "navigate",
          icon: Brain
        }
      }
    ],
    bottomItem: {
      itemId: "settings",
      label: "Settings",
      route: { kind: "settings", section: "agents" },
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
      "Agents";
    return { parent: "Settings", current };
  }

  return { current: route.kind === "memory" ? "Memory" : "Home" };
}

export function shellMenuSelectionBehavior(
  itemId: ShellMenuItemId
): ShellMenuSelectionBehavior {
  return itemId === "settings" ? "keep-reveal-open" : "close-reveal";
}
