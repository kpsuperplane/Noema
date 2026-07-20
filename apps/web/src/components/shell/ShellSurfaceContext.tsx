import React from "react";

export type ShellSurfaceVisibility = "visible" | "hiding" | "hidden" | "showing";

export type ShellMemoryBreadcrumb = {
  ancestors: Array<{ path: string; title: string }>;
  current: string;
};

export type ShellSurfaceState = {
  visibility: ShellSurfaceVisibility;
  setMemoryBreadcrumb: (breadcrumb: ShellMemoryBreadcrumb | null) => void;
};

export const defaultShellSurfaceState: ShellSurfaceState = {
  visibility: "visible",
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
