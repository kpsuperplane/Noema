type TauriCandidate = {
  __TAURI_INTERNALS__?: {
    invoke?: unknown;
  };
};

export function isTauriRuntime(
  candidate: TauriCandidate | undefined = globalThis.window as TauriCandidate | undefined
) {
  return typeof candidate?.__TAURI_INTERNALS__?.invoke === "function";
}
