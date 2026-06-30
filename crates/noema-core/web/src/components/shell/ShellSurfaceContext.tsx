import React from "react";

export type ShellSurfaceVisibility = "visible" | "hiding" | "hidden" | "showing";

export type ShellSurfaceState = {
  visibility: ShellSurfaceVisibility;
};

export const defaultShellSurfaceState: ShellSurfaceState = {
  visibility: "visible"
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
