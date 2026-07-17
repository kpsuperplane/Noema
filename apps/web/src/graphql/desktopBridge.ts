type Unlisten = () => void;

type TauriCore = {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
};

type TauriEvent = {
  listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<Unlisten>;
};

async function importTauriCore(): Promise<TauriCore> {
  return await import("@tauri-apps/api/core");
}

async function importTauriEvent(): Promise<TauriEvent> {
  return await import("@tauri-apps/api/event");
}

export async function invokeDesktop<T>(command: string, args?: Record<string, unknown>) {
  const { invoke } = await importTauriCore();
  return invoke<T>(command, args);
}

export async function listenDesktop<T>(eventName: string, handler: (payload: T) => void) {
  const { listen } = await importTauriEvent();
  return listen<T>(eventName, (event) => handler(event.payload));
}
