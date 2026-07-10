export const READY_PREFIX = "NOEMA_BROWSER_READY ";
export const MAX_READY_FRAME_BYTES = 4 * 1024;
export const MAX_READY_JSON_BYTES = 2 * 1024;
export const MAX_READY_OUTPUT_BYTES = 64 * 1024;

const UTF8 = new TextEncoder();
const READY_KEYS = ["kind", "origin", "scenario", "schemaVersion", "startUrl"] as const;

export type BrowserScenario = "boot";

export interface BrowserReady {
  readonly schemaVersion: 1;
  readonly kind: "ready";
  readonly scenario: BrowserScenario;
  readonly origin: string;
  readonly startUrl: string;
}

export class ReadinessParser {
  readonly #decoder = new TextDecoder("utf-8", { fatal: true });
  #pending = "";
  #totalBytes = 0;
  #ready: BrowserReady | undefined;

  push(chunk: string | Uint8Array): void {
    let text: string;
    let byteLength: number;
    try {
      if (typeof chunk === "string") {
        text = chunk;
        byteLength = UTF8.encode(chunk).byteLength;
      } else {
        byteLength = chunk.byteLength;
        text = this.#decoder.decode(chunk, { stream: true });
      }
    } catch {
      throw new Error("invalid browser fixture output");
    }
    this.#totalBytes += byteLength;
    if (this.#totalBytes > MAX_READY_OUTPUT_BYTES) {
      throw new Error("browser fixture output exceeded limit");
    }
    this.#pending += text;
    this.#consumeLines();
    if (UTF8.encode(this.#pending).byteLength > MAX_READY_FRAME_BYTES) {
      throw new Error("browser fixture output line exceeded limit");
    }
  }

  finish(): BrowserReady {
    try {
      this.#pending += this.#decoder.decode();
    } catch {
      throw new Error("invalid browser fixture output");
    }
    this.#consumeLines();
    if (this.#ready === undefined) throw new Error("browser readiness frame missing");
    return this.#ready;
  }

  get ready(): BrowserReady | undefined {
    return this.#ready;
  }

  #consumeLines(): void {
    while (true) {
      const newline = this.#pending.indexOf("\n");
      if (newline < 0) return;
      const line = this.#pending.slice(0, newline).replace(/\r$/u, "");
      this.#pending = this.#pending.slice(newline + 1);
      if (UTF8.encode(line).byteLength > MAX_READY_FRAME_BYTES) {
        throw new Error("browser fixture output line exceeded limit");
      }
      if (!line.startsWith(READY_PREFIX)) continue;
      if (this.#ready !== undefined) throw new Error("duplicate browser readiness frame");
      const json = line.slice(READY_PREFIX.length);
      if (UTF8.encode(json).byteLength > MAX_READY_JSON_BYTES) {
        throw new Error("invalid browser readiness frame");
      }
      this.#ready = parseReadyJson(json);
    }
  }
}

function parseReadyJson(json: string): BrowserReady {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    throw new Error("invalid browser readiness frame");
  }
  if (!isPlainRecord(value)) throw new Error("invalid browser readiness frame");
  const keys = Object.keys(value).sort();
  if (keys.length !== READY_KEYS.length || keys.some((key, index) => key !== READY_KEYS[index])) {
    throw new Error("invalid browser readiness frame");
  }
  if (
    value.schemaVersion !== 1 ||
    value.kind !== "ready" ||
    value.scenario !== "boot" ||
    typeof value.origin !== "string" ||
    typeof value.startUrl !== "string"
  ) {
    throw new Error("invalid browser readiness frame");
  }
  validateReadyUrls(value.origin, value.startUrl);
  return Object.freeze({
    schemaVersion: 1,
    kind: "ready",
    scenario: "boot",
    origin: value.origin,
    startUrl: value.startUrl
  });
}

function validateReadyUrls(origin: string, startUrl: string): void {
  if (
    UTF8.encode(origin).byteLength > MAX_READY_JSON_BYTES ||
    UTF8.encode(startUrl).byteLength > MAX_READY_JSON_BYTES
  ) {
    throw new Error("invalid browser readiness frame");
  }
  let parsedOrigin: URL;
  let parsedStart: URL;
  try {
    parsedOrigin = new URL(origin);
    parsedStart = new URL(startUrl);
  } catch {
    throw new Error("invalid browser readiness frame");
  }
  if (
    parsedOrigin.protocol !== "http:" ||
    parsedOrigin.hostname !== "127.0.0.1" ||
    parsedOrigin.username !== "" ||
    parsedOrigin.password !== "" ||
    parsedOrigin.port === "" ||
    Number(parsedOrigin.port) < 1 ||
    Number(parsedOrigin.port) > 65_535 ||
    parsedOrigin.pathname !== "/" ||
    parsedOrigin.search !== "" ||
    parsedOrigin.hash !== "" ||
    parsedOrigin.origin !== origin
  ) {
    throw new Error("invalid browser readiness frame");
  }
  if (
    parsedStart.protocol !== "http:" ||
    parsedStart.hostname !== "127.0.0.1" ||
    parsedStart.username !== "" ||
    parsedStart.password !== "" ||
    parsedStart.port !== parsedOrigin.port ||
    parsedStart.pathname !== "/" ||
    parsedStart.search !== "" ||
    parsedStart.hash !== "" ||
    parsedStart.origin !== origin ||
    parsedStart.href !== `${origin}/`
  ) {
    throw new Error("invalid browser readiness frame");
  }
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return (
    typeof value === "object" &&
    value !== null &&
    Object.getPrototypeOf(value) === Object.prototype
  );
}
