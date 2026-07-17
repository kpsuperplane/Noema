import { invokeDesktop } from "./desktopBridge";
import { isTauriRuntime } from "./transportMode";

type OpenExternalUrlOptions = {
  isDesktop?: boolean;
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
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
