export const MAX_BROWSER_ARTIFACT_ID_LENGTH = 64;
export const MAX_BROWSER_ARTIFACT_STEPS = 128;
export const MAX_BROWSER_ARTIFACT_EVENTS = 512;
export const MAX_BROWSER_ARTIFACT_URL_BYTES = 8 * 1024;
export const MAX_BROWSER_ARTIFACT_CANARIES = 32;
export const MAX_BROWSER_ARTIFACT_CANARY_BYTES = 4 * 1024;
export const MAX_BROWSER_ARTIFACT_JSON_BYTES = 256 * 1024;
export const MAX_BROWSER_ARTIFACT_SCREENSHOT_BYTES = 8 * 1024 * 1024;
export const MAX_BROWSER_ARTIFACT_COUNT = 1_000_000;

const SAFE_ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const SAFE_DYNAMIC_SEGMENT = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;
const UTF8 = new TextEncoder();

export const BROWSER_ARTIFACT_PROFILES = [
  "ephemeral-public",
  "ephemeral-sensitive",
  "authenticated-live"
] as const;
export type BrowserArtifactProfile = (typeof BROWSER_ARTIFACT_PROFILES)[number];

export const BROWSER_ENDPOINTS = [
  "app-root",
  "memory",
  "settings",
  "settings-index",
  "settings-agents",
  "settings-memory",
  "settings-tools-web",
  "settings-tools-mcps",
  "settings-system-providers",
  "settings-safety-usage",
  "settings-safety-identities",
  "settings-safety-approvals",
  "graphql",
  "graphql-schema",
  "graphql-websocket",
  "mcp-oauth-callback",
  "asset",
  "artifact-download",
  "artifact-version-download",
  "external",
  "other"
] as const;
export type BrowserEndpoint = (typeof BROWSER_ENDPOINTS)[number];

export const BROWSER_ARTIFACT_FAILURE_KINDS = [
  "assertion",
  "navigation",
  "network",
  "console",
  "page-error",
  "graphql-error",
  "timeout",
  "harness"
] as const;
export type BrowserArtifactFailureKind = (typeof BROWSER_ARTIFACT_FAILURE_KINDS)[number];

export const BROWSER_NETWORK_METHODS = ["GET", "POST", "OPTIONS", "OTHER"] as const;
export type BrowserNetworkMethod = (typeof BROWSER_NETWORK_METHODS)[number];

export const BROWSER_NETWORK_OUTCOMES = ["response", "failed"] as const;
export type BrowserNetworkOutcome = (typeof BROWSER_NETWORK_OUTCOMES)[number];

export const BROWSER_CONSOLE_LEVELS = ["debug", "info", "log", "warn", "error", "other"] as const;
export type BrowserConsoleLevel = (typeof BROWSER_CONSOLE_LEVELS)[number];

export type BrowserArtifactEvent =
  | Readonly<{ sequence: number; kind: "step"; step: string }>
  | Readonly<{ sequence: number; kind: "navigation"; endpoint: BrowserEndpoint }>
  | Readonly<{
      sequence: number;
      kind: "network";
      method: BrowserNetworkMethod;
      endpoint: BrowserEndpoint;
      outcome: BrowserNetworkOutcome;
      status?: number;
    }>
  | Readonly<{ sequence: number; kind: "console"; level: BrowserConsoleLevel }>
  | Readonly<{ sequence: number; kind: "page-error"; count: number }>
  | Readonly<{ sequence: number; kind: "graphql-error"; count: number }>;

export interface BrowserFailureTrace {
  readonly schemaVersion: 1;
  readonly scenario: string;
  readonly outcome: "failed";
  readonly failure: Readonly<{ kind: BrowserArtifactFailureKind; step?: string }>;
  readonly events: readonly BrowserArtifactEvent[];
}

export function validateBrowserArtifactId(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length < 1 ||
    value.length > MAX_BROWSER_ARTIFACT_ID_LENGTH ||
    !SAFE_ID.test(value)
  ) {
    throw new Error("invalid browser artifact id");
  }
  return value;
}

export function validateBrowserArtifactProfile(value: unknown): BrowserArtifactProfile {
  if (!isMember(BROWSER_ARTIFACT_PROFILES, value)) {
    throw new Error("invalid browser artifact profile");
  }
  return value;
}

export function validateFailureKind(value: unknown): BrowserArtifactFailureKind {
  if (!isMember(BROWSER_ARTIFACT_FAILURE_KINDS, value)) {
    throw new Error("invalid browser artifact failure kind");
  }
  return value;
}

export function validateNetworkMethod(value: unknown): BrowserNetworkMethod {
  if (!isMember(BROWSER_NETWORK_METHODS, value)) {
    throw new Error("invalid network method");
  }
  return value;
}

export function validateNetworkOutcome(value: unknown): BrowserNetworkOutcome {
  if (!isMember(BROWSER_NETWORK_OUTCOMES, value)) {
    throw new Error("invalid network outcome");
  }
  return value;
}

export function normalizeConsoleLevel(value: unknown): BrowserConsoleLevel {
  if (value === "warning") return "warn";
  if (isMember(BROWSER_CONSOLE_LEVELS, value)) return value;
  throw new Error("invalid console level");
}

export function validateHttpStatus(value: unknown): number {
  if (!Number.isInteger(value) || (value as number) < 100 || (value as number) > 599) {
    throw new Error("invalid HTTP status");
  }
  return value as number;
}

export function validateEventCount(value: unknown): number {
  if (
    !Number.isInteger(value) ||
    (value as number) < 0 ||
    (value as number) > MAX_BROWSER_ARTIFACT_COUNT
  ) {
    throw new Error("invalid event count");
  }
  return value as number;
}

export function buildSecretCanaryRepresentations(value: unknown): readonly Uint8Array[] {
  const canaries = copyPlainDataArray(value, "invalid secret canaries");
  if (canaries.length > MAX_BROWSER_ARTIFACT_CANARIES) throw new Error("too many secret canaries");
  const representations = new Set<string>();
  for (const canary of canaries) {
    if (typeof canary !== "string" || !isWellFormedUnicode(canary)) {
      throw new Error("invalid secret canary");
    }
    const bytes = UTF8.encode(canary);
    if (bytes.byteLength < 1 || bytes.byteLength > MAX_BROWSER_ARTIFACT_CANARY_BYTES) {
      throw new Error("invalid secret canary");
    }
    for (const normalized of new Set([canary.normalize("NFC"), canary.normalize("NFD")])) {
      for (const form of transformCanary(normalized)) representations.add(form);
    }
  }
  return Object.freeze([...representations].map((form) => Uint8Array.from(UTF8.encode(form))));
}

export function validateDaemonOrigin(rawOrigin: unknown): string {
  if (typeof rawOrigin !== "string" || UTF8.encode(rawOrigin).byteLength > MAX_BROWSER_ARTIFACT_URL_BYTES) {
    throw new Error("invalid daemon origin");
  }
  if (!rawDaemonOriginHasNoState(rawOrigin)) throw new Error("invalid daemon origin");

  let parsed: URL;
  try {
    parsed = new URL(rawOrigin);
  } catch {
    throw new Error("invalid daemon origin");
  }

  if (
    (parsed.protocol !== "http:" && parsed.protocol !== "https:") ||
    parsed.username !== "" ||
    parsed.password !== "" ||
    parsed.pathname !== "/" ||
    parsed.search !== "" ||
    parsed.hash !== "" ||
    !isLoopbackHostname(parsed.hostname)
  ) {
    throw new Error("invalid daemon origin");
  }
  return parsed.origin;
}

export function classifyBrowserUrl(rawUrl: unknown, daemonOrigin: unknown): BrowserEndpoint {
  const localOrigin = validateDaemonOrigin(daemonOrigin);
  if (
    typeof rawUrl !== "string" ||
    rawUrl.trim() !== rawUrl ||
    /\s/u.test(rawUrl) ||
    UTF8.encode(rawUrl).byteLength > MAX_BROWSER_ARTIFACT_URL_BYTES
  ) {
    return "other";
  }

  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    return "other";
  }

  if (parsed.username !== "" || parsed.password !== "" || parsed.search !== "" || parsed.hash !== "") {
    return "other";
  }
  if (!["http:", "https:", "ws:", "wss:"].includes(parsed.protocol)) return "other";
  if (!rawPathIsUnambiguous(rawUrl)) return "other";

  const isEquivalentWebSocket =
    (parsed.protocol === "ws:" || parsed.protocol === "wss:") &&
    websocketHttpOrigin(parsed) === localOrigin;
  if (parsed.origin !== localOrigin && !isEquivalentWebSocket) return "external";
  if (isEquivalentWebSocket) {
    return parsed.pathname === "/graphql/ws" ? "graphql-websocket" : "other";
  }

  const exact: Readonly<Record<string, BrowserEndpoint>> = {
    "/": "app-root",
    "/memory": "memory",
    "/settings": "settings",
    "/settings/": "settings-index",
    "/settings/agents": "settings-agents",
    "/settings/memory": "settings-memory",
    "/settings/tools/web": "settings-tools-web",
    "/settings/tools/mcps": "settings-tools-mcps",
    "/settings/system/providers": "settings-system-providers",
    "/settings/safety/usage": "settings-safety-usage",
    "/settings/safety/identities": "settings-safety-identities",
    "/settings/safety/approvals": "settings-safety-approvals",
    "/graphql": "graphql",
    "/graphql/schema.graphql": "graphql-schema",
    "/graphql/ws": "graphql-websocket",
    "/mcp/oauth/callback": "mcp-oauth-callback"
  };
  const exactEndpoint = exact[parsed.pathname];
  if (exactEndpoint !== undefined) return exactEndpoint;

  const segments = parsed.pathname.split("/");
  if (segments.length === 3 && segments[1] === "assets" && isSafeDynamicSegment(segments[2])) {
    return "asset";
  }
  if (
    segments.length === 4 &&
    segments[1] === "artifacts" &&
    isSafeDynamicSegment(segments[2]) &&
    segments[3] === "download"
  ) {
    return "artifact-download";
  }
  if (
    segments.length === 5 &&
    segments[1] === "artifacts" &&
    segments[2] === "versions" &&
    isSafeDynamicSegment(segments[3]) &&
    segments[4] === "download"
  ) {
    return "artifact-version-download";
  }
  return "other";
}

function rawPathIsUnambiguous(rawUrl: string): boolean {
  const schemeEnd = rawUrl.indexOf("://");
  if (schemeEnd < 0) return false;
  const pathStart = rawUrl.indexOf("/", schemeEnd + 3);
  const rawPath = pathStart < 0 ? "/" : rawUrl.slice(pathStart);
  return (
    !rawPath.includes("\\") &&
    !/%(?:2e|2f|5c)/i.test(rawPath) &&
    !/\/(?:\.{1,2})(?:\/|$)/.test(rawPath) &&
    !rawPath.includes("//")
  );
}

function rawDaemonOriginHasNoState(rawOrigin: string): boolean {
  if (rawOrigin.trim() !== rawOrigin || /\s/u.test(rawOrigin) || rawOrigin.includes("\\")) {
    return false;
  }
  const match = /^[a-z][a-z0-9+.-]*:\/\/([^/?#]+)(\/?)$/iu.exec(rawOrigin);
  return match !== null && (match[2] === "" || match[2] === "/");
}

function websocketHttpOrigin(url: URL): string {
  const protocol = url.protocol === "ws:" ? "http:" : "https:";
  return `${protocol}//${url.host}`;
}

function isLoopbackHostname(hostname: string): boolean {
  if (hostname === "localhost" || hostname === "[::1]" || hostname === "::1") return true;
  const octets = hostname.split(".");
  return (
    octets.length === 4 &&
    octets[0] === "127" &&
    octets.every((octet) => /^\d{1,3}$/.test(octet) && Number(octet) <= 255)
  );
}

function isSafeDynamicSegment(segment: string): boolean {
  return segment !== "." && segment !== ".." && SAFE_DYNAMIC_SEGMENT.test(segment);
}

function isMember<const T extends readonly string[]>(values: T, value: unknown): value is T[number] {
  return typeof value === "string" && (values as readonly string[]).includes(value);
}

function transformCanary(value: string): readonly string[] {
  const bytes = Buffer.from(value, "utf8");
  const jsonEscaped = JSON.stringify(value).slice(1, -1);
  const unicodeLower = [...value]
    .flatMap((character) => utf16CodeUnits(character))
    .map((unit) => `\\u${unit.toString(16).padStart(4, "0")}`)
    .join("");
  const unicodeUpper = unicodeLower.replace(/[a-f]/g, (character) => character.toUpperCase());
  const jsonUnicodeLower = jsonUnicodeEscaped(value, false);
  const jsonUnicodeUpper = jsonUnicodeEscaped(value, true);
  const percentUpper = encodeURIComponent(value);
  const percentLower = lowercasePercentEscapes(percentUpper);
  const formUpper = percentUpper.replace(/%20/g, "+");
  const formLower = lowercasePercentEscapes(formUpper);
  const fullyEncodedUpper = fullyPercentEncode(bytes, false, false);
  const fullyEncodedLower = fullyPercentEncode(bytes, true, false);
  const fullyFormUpper = fullyPercentEncode(bytes, false, true);
  const fullyFormLower = fullyPercentEncode(bytes, true, true);
  const base64 = bytes.toString("base64");
  const base64UrlPadded = base64.replace(/\+/g, "-").replace(/\//g, "_");
  const hexLower = bytes.toString("hex");
  return [
    value,
    jsonEscaped,
    jsonUnicodeLower,
    jsonUnicodeUpper,
    unicodeLower,
    unicodeUpper,
    percentUpper,
    percentLower,
    formUpper,
    formLower,
    fullyEncodedUpper,
    fullyEncodedLower,
    fullyFormUpper,
    fullyFormLower,
    base64,
    base64.replace(/=+$/u, ""),
    base64UrlPadded,
    base64UrlPadded.replace(/=+$/u, ""),
    hexLower,
    hexLower.toUpperCase()
  ];
}

function fullyPercentEncode(bytes: Uint8Array, lowercase: boolean, formSpace: boolean): string {
  let encoded = "";
  for (const byte of bytes) {
    if (formSpace && byte === 0x20) {
      encoded += "+";
      continue;
    }
    const hex = byte.toString(16).padStart(2, "0");
    encoded += `%${lowercase ? hex : hex.toUpperCase()}`;
  }
  return encoded;
}

function lowercasePercentEscapes(value: string): string {
  return value.replace(/%[0-9A-F]{2}/g, (escape) => escape.toLowerCase());
}

function utf16CodeUnits(value: string): readonly number[] {
  const units: number[] = [];
  for (let index = 0; index < value.length; index += 1) units.push(value.charCodeAt(index));
  return units;
}

function jsonUnicodeEscaped(value: string, uppercase: boolean): string {
  let escaped = "";
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0x20 && unit <= 0x7e && unit !== 0x22 && unit !== 0x5c) {
      escaped += value[index];
      continue;
    }
    const hex = unit.toString(16).padStart(4, "0");
    escaped += `\\u${uppercase ? hex.toUpperCase() : hex}`;
  }
  return escaped;
}

function isWellFormedUnicode(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function copyPlainDataArray(value: unknown, message: string): readonly unknown[] {
  try {
    if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
      throw new Error(message);
    }
    const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as Record<
      PropertyKey,
      PropertyDescriptor
    >;
    const ownKeys = Reflect.ownKeys(descriptors);
    if (ownKeys.some((key) => typeof key === "symbol")) throw new Error(message);
    const lengthDescriptor = descriptors["length"];
    if (lengthDescriptor === undefined || !("value" in lengthDescriptor)) throw new Error(message);
    const length = lengthDescriptor.value;
    if (!Number.isSafeInteger(length) || length < 0) throw new Error(message);
    const copied: unknown[] = [];
    for (let index = 0; index < length; index += 1) {
      const descriptor = descriptors[String(index)];
      if (descriptor === undefined || !descriptor.enumerable || !("value" in descriptor)) {
        throw new Error(message);
      }
      copied.push(descriptor.value);
    }
    const expectedKeys = new Set(["length", ...copied.map((_, index) => String(index))]);
    if (ownKeys.some((key) => typeof key !== "string" || !expectedKeys.has(key))) {
      throw new Error(message);
    }
    return Object.freeze(copied);
  } catch {
    throw new Error(message);
  }
}
