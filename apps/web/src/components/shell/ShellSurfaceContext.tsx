import React from "react";

export type ShellSurfaceVisibility = "visible" | "hiding" | "hidden" | "showing";

export type ShellMemoryBreadcrumb = {
  ancestors: Array<{ path: string; title: string }>;
  current: string;
  currentPath: string;
};

export type ShellSurfaceState = {
  visibility: ShellSurfaceVisibility;
  sidebarAvailable: boolean;
  setMemoryBreadcrumb: (breadcrumb: ShellMemoryBreadcrumb | null) => void;
};

export const defaultShellSurfaceState: ShellSurfaceState = {
  visibility: "visible",
  sidebarAvailable: false,
  setMemoryBreadcrumb: () => undefined
};

const ShellSurfaceContext = React.createContext<ShellSurfaceState>(defaultShellSurfaceState);

export function ShellSurfaceProvider({
  value,
  children
}: {
  value: ShellSurfaceState;
  children: React.ReactNode;
}) {
  return <ShellSurfaceContext.Provider value={value}>{children}</ShellSurfaceContext.Provider>;
}

export function useShellSurface() {
  return React.useContext(ShellSurfaceContext);
}
