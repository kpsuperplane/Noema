export type KeyValueParseResult =
  | { value: Record<string, string>; error: null }
  | { value: null; error: string };

export type McpSetupFormSubmission = {
  displayName: string;
  transportKind: "stdio" | "streamable_http";
  authPreference?: "PROMPT_IF_AVAILABLE" | "USE_ANONYMOUS";
  stdio?: {
    command: string;
    args: string[];
    cwd?: string | null;
    env: Record<string, string>;
    secretEnv: Record<string, string>;
  } | null;
  http?: {
    url: string;
    headers: Record<string, string>;
    secretHeaders: Record<string, string>;
    oauthClientCredentials?: McpOAuthClientCredentials | null;
  } | null;
};

export type McpOAuthClientCredentials = {
  clientId: string;
  clientSecret: string;
  scopes: string[];
};

export type McpSetupContinueSubmission = {
  mcpServerId: string;
  secretEnv: Record<string, string>;
  secretHeaders: Record<string, string>;
  oauthClientCredentials?: McpOAuthClientCredentials | null;
};

export function parseKeyValueLines(input: string): KeyValueParseResult {
  const value: Record<string, string> = {};
  const lines = input.split(/\r?\n/);
  for (const [index, rawLine] of lines.entries()) {
    const line = rawLine.trim();
    if (!line) continue;
    const equalsIndex = line.indexOf("=");
    if (equalsIndex <= 0) {
      return { value: null, error: `Use KEY=value on line ${index + 1}.` };
    }
    const key = line.slice(0, equalsIndex).trim();
    const parsedValue = line.slice(equalsIndex + 1).trim();
    if (!key) {
      return { value: null, error: `Add a key to line ${index + 1}.` };
    }
    if (Object.prototype.hasOwnProperty.call(value, key)) {
      return { value: null, error: `The key "${key}" appears more than once.` };
    }
    value[key] = parsedValue;
  }
  return { value, error: null };
}

export function parseArgsLines(input: string): string[] {
  return input
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}
