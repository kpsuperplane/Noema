type TauriCandidate = {
  __TAURI_INTERNALS__?: unknown;
};

export function isTauriRuntime(
  candidate: TauriCandidate | undefined = globalThis.window as TauriCandidate | undefined
) {
  return Boolean(candidate?.__TAURI_INTERNALS__);
}
