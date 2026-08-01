import {
  Archive,
  BriefcaseBusiness,
  Bot,
  Brain,
  Cable,
  Cpu,
  Folder,
  Gauge,
  Globe,
  ListTodo,
  MessageCircle,
  PlugZap,
  ServerCog,
  ShieldCheck,
  Smartphone
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { AppRoute, NonSettingsAppRoute, SettingsSection } from "@/app/routes";

export type ShellMenuLevelId = "settings" | "work";

export type ShellMenuItemId =
  | "chat"
  | "work"
  | "memory"
  | "settings"
  | "settings.agents"
  | "settings.models"
  | "settings.memory"
  | "settings.tools.web"
  | "settings.tools.apis"
  | "settings.tools.mcps"
  | "settings.safety.privacy"
  | "settings.safety.usage"
  | "settings.system.providers"
  | "settings.system.clients"
  | "work.workspace.personal"
  | "work.projects.archived"
  | `work.project.${string}`;

export type ShellMenuItem = {
  itemId: ShellMenuItemId;
  label: string;
  route?: AppRoute;
  icon: LucideIcon;
  depth?: 1;
  ariaExpanded?: boolean;
  pinned?: boolean;
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

export type ShellWorkProject = {
  projectId: string;
  name: string;
  archivedAt: string | null;
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
    item: {
      section: "tools-apis",
      itemId: "settings.tools.apis",
      label: "APIs",
      icon: Cable
    }
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
      label: "Execution",
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
  },
  {
    kind: "section",
    item: {
      section: "system-clients",
      itemId: "settings.system.clients",
      label: "Clients",
      icon: Smartphone
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
    label: "Tasks",
    route: { kind: "work" },
    icon: ListTodo
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

export function workMenuLevelForProjects(
  projects: readonly ShellWorkProject[],
  projectId?: string
): ShellMenuLevel {
  const activeItemId = projectId && projects.some((project) => project.projectId === projectId)
    ? projectItemId(projectId)
    : "work.workspace.personal";

  return {
    levelId: "work",
    ariaLabel: "Task folders",
    title: "Tasks",
    activeItemId,
    items: [
      {
        kind: "item",
        item: {
          itemId: "work.workspace.personal",
          label: "Personal",
          route: { kind: "work" },
          icon: BriefcaseBusiness
        }
      },
      ...(projects.length > 0 ? [
        ...projects.filter((project) => !project.archivedAt).map((project) => ({
          kind: "item" as const,
          item: {
            itemId: projectItemId(project.projectId),
            label: project.name,
            route: { kind: "work" as const, projectId: project.projectId },
            icon: Folder,
            depth: 1 as const
          }
        })),
        ...(projects.some((project) => project.archivedAt) ? [{
          kind: "item" as const,
          item: {
            itemId: "work.projects.archived" as const,
            label: "Archived",
            icon: Archive,
            pinned: true
          }
        }] : [])
      ] : [])
    ]
  };
}

function projectItemId(projectId: string): `work.project.${string}` {
  return `work.project.${projectId}`;
}

export function breadcrumbForRoute(route: AppRoute): ShellBreadcrumb {
  if (route.kind === "settings") {
    const current =
      shellSettingsSections.find((section) => section.section === route.section)?.label ??
      "Agents";
    return { parent: "Settings", current };
  }

  if (route.kind === "work") {
    return { current: "Tasks" };
  }
  return { current: route.kind === "memory" ? "Memory" : "Chat" };
}
