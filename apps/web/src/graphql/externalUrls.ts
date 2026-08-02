import { invokeDesktop } from "./desktopBridge";
import { isTauriRuntime } from "./transportMode";

type OpenExternalUrlOptions = {
  isDesktop?: boolean;
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
};

export type ReservedExternalAuthNavigation = {
  open: (url: string) => Promise<void>;
  cancel: () => void;
};

export async function openExternalUrlForAuth(url: string, options: OpenExternalUrlOptions = {}) {
  const isDesktop = options.isDesktop ?? isTauriRuntime();
  if (!isDesktop) {
    return false;
  }

  const invoke = options.invoke ?? invokeDesktop;
  try {
    await invoke("open_external_url", { url });
    return true;
  } catch {
    return false;
  }
}

export function reserveExternalAuthNavigation(
  options: OpenExternalUrlOptions = {}
): ReservedExternalAuthNavigation {
  const isDesktop = options.isDesktop ?? isTauriRuntime();
  const authWindow = isDesktop ? null : window.open("about:blank", "_blank");
  if (authWindow) authWindow.opener = null;

  return {
    open: async (url) => {
      if (isDesktop) {
        const handled = await openExternalUrlForAuth(url, options);
        if (!handled) window.open(url, "_blank", "noopener,noreferrer");
      } else if (authWindow && !authWindow.closed) {
        authWindow.location.replace(url);
      } else {
        window.location.assign(url);
      }
    },
    cancel: () => authWindow?.close()
  };
}
