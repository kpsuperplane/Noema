import React from "react";
import type { SettingsSection } from "./routes";

export type AppRuntime = {
  chatView: React.ReactNode;
  openMemoryGraph: () => void;
  settingsSection: SettingsSection | null;
};

const AppRuntimeContext = React.createContext<AppRuntime | null>(null);

export function AppRuntimeProvider({
  value,
  children
}: {
  value: AppRuntime;
  children: React.ReactNode;
}) {
  return (
    <AppRuntimeContext.Provider value={value}>
      {children}
    </AppRuntimeContext.Provider>
  );
}

export function useAppRuntime() {
  const runtime = React.useContext(AppRuntimeContext);
  if (!runtime) {
    throw new Error("Noema route rendered outside the app runtime.");
  }
  return runtime;
}
