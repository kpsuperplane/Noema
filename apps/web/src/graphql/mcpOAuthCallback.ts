import { invokeDesktop } from "./desktopBridge";
import { isTauriRuntime } from "./transportMode";

const MCP_OAUTH_CALLBACK_PATH = "/mcp/oauth/callback";

type CallbackLocation = Pick<Location, "origin">;
type RedirectUriOptions = {
  isDesktop?: boolean;
  invoke?: (command: string) => Promise<string>;
};

export async function mcpOAuthRedirectUri(
  location: CallbackLocation = window.location,
  options: RedirectUriOptions = {}
): Promise<string> {
  const isDesktop = options.isDesktop ?? isTauriRuntime();
  if (isDesktop) {
    const invoke = options.invoke ?? invokeDesktop;
    return String(await invoke("mcp_oauth_callback_url"));
  }
  return `${location.origin}${MCP_OAUTH_CALLBACK_PATH}`;
}
