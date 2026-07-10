import type { Route, WebSocketRoute } from "playwright";

import type {
  BrowserArtifactFailureKind,
  BrowserNetworkMethod
} from "../artifacts/model.ts";

export interface ArtifactObservationRecorder {
  recordNetwork(observation: {
    readonly method: BrowserNetworkMethod;
    readonly rawUrl: string;
    readonly status?: number;
    readonly outcome: "response" | "failed";
  }): void;
  recordConsole(level: "error"): void;
  recordPageError(count: number): void;
}

export type ObservationFailure = Readonly<{
  kind: BrowserArtifactFailureKind;
  step: "boot";
}>;

export interface ObservationSnapshot {
  readonly failure: ObservationFailure | undefined;
  readonly responseStatuses: ReadonlyMap<string, number>;
}

type HttpRouteLike = Pick<Route, "abort" | "continue" | "request">;
type WebSocketRouteLike = Pick<
  WebSocketRoute,
  "close" | "connectToServer" | "onMessage" | "send" | "url"
>;

export class ObservationOwner {
  readonly #recorder: ArtifactObservationRecorder;
  readonly #origin: string;
  readonly #externalHttpRequests = new WeakSet<object>();
  readonly #observedExternalHttpRequests = new WeakSet<object>();
  readonly #externalSockets = new WeakSet<object>();
  readonly #responseStatuses = new Map<string, number>();
  #failure: ObservationFailure | undefined;
  #active = true;

  constructor(recorder: ArtifactObservationRecorder, origin: string) {
    this.#recorder = recorder;
    this.#origin = origin;
  }

  freeze(): ObservationSnapshot {
    this.#active = false;
    return this.snapshot();
  }

  snapshot(): ObservationSnapshot {
    return Object.freeze({
      failure: this.#failure,
      responseStatuses: new Map(this.#responseStatuses)
    });
  }

  async onHttpRoute(route: HttpRouteLike): Promise<void> {
    try {
      const request = route.request();
      const rawUrl = request.url();
      if (!isAllowedHttpUrl(rawUrl, this.#origin)) {
        this.#externalHttpRequests.add(request);
        if (this.#active && !this.#observedExternalHttpRequests.has(request)) {
          this.#observedExternalHttpRequests.add(request);
          this.#recordNetwork({
            method: networkMethod(request.method?.() ?? "OTHER"),
            rawUrl,
            outcome: "failed"
          });
          this.#setFailure("network");
        }
        await route.abort("blockedbyclient");
        return;
      }
      await route.continue();
    } catch {
      this.#setHarnessFailure();
      try {
        await route.abort("blockedbyclient");
      } catch {
        // Route ownership remains fail-closed.
      }
    }
  }

  async onWebSocketRoute(route: WebSocketRouteLike): Promise<void> {
    try {
      if (!isAllowedWebSocketUrl(route.url(), this.#origin)) {
        if (this.#active && !this.#externalSockets.has(route)) {
          this.#externalSockets.add(route);
          this.#recordNetwork({
            method: "GET",
            rawUrl: route.url(),
            outcome: "failed"
          });
          this.#setFailure("network");
        }
        await route.close({ code: 1008, reason: "blocked" });
        return;
      }
      const server = route.connectToServer();
      route.onMessage((message) => {
        try {
          server.send(message);
        } catch {
          this.#setHarnessFailure();
        }
      });
      server.onMessage((message) => {
        try {
          route.send(message);
        } catch {
          this.#setHarnessFailure();
        }
      });
    } catch {
      this.#setHarnessFailure();
      try {
        await route.close({ code: 1011, reason: "unavailable" });
      } catch {
        // Callback methods are total and never expose route values.
      }
    }
  }

  onConsoleError(): void {
    if (!this.#active) return;
    try {
      this.#recorder.recordConsole("error");
      this.#setFailure("console");
    } catch {
      this.#setHarnessFailure();
    }
  }

  onPageError(): void {
    if (!this.#active) return;
    try {
      this.#recorder.recordPageError(1);
      this.#setFailure("page-error");
    } catch {
      this.#setHarnessFailure();
    }
  }

  onRequestFailed(method: string, rawUrl: string, request?: object): void {
    if (!this.#active || (request !== undefined && this.#externalHttpRequests.has(request))) return;
    this.#recordNetwork({
      method: networkMethod(method),
      rawUrl,
      outcome: "failed"
    });
    this.#setFailure("network");
  }

  onResponse(method: string, rawUrl: string, status: number): void {
    if (!this.#active) return;
    this.#responseStatuses.set(rawUrl, status);
    this.#recordNetwork({
      method: networkMethod(method),
      rawUrl,
      status,
      outcome: "response"
    });
    if (status < 200 || status >= 400) this.#setFailure("network");
  }

  #recordNetwork(
    observation: Parameters<ArtifactObservationRecorder["recordNetwork"]>[0]
  ): void {
    if (!this.#active) return;
    try {
      this.#recorder.recordNetwork(observation);
    } catch {
      this.#setHarnessFailure();
    }
  }

  #setFailure(kind: BrowserArtifactFailureKind): void {
    if (!this.#active || this.#failure !== undefined) return;
    this.#failure = Object.freeze({ kind, step: "boot" });
  }

  #setHarnessFailure(): void {
    if (!this.#active) return;
    this.#failure = Object.freeze({ kind: "harness", step: "boot" });
  }
}

export function isAllowedHttpUrl(rawUrl: string, origin: string): boolean {
  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    return false;
  }
  return (
    parsed.protocol === "http:" &&
    parsed.origin === origin &&
    parsed.username === "" &&
    parsed.password === ""
  );
}

export function isAllowedWebSocketUrl(rawUrl: string, origin: string): boolean {
  let parsed: URL;
  let parsedOrigin: URL;
  try {
    parsed = new URL(rawUrl);
    parsedOrigin = new URL(origin);
  } catch {
    return false;
  }
  const expectedProtocol = parsedOrigin.protocol === "https:" ? "wss:" : "ws:";
  return (
    parsed.protocol === expectedProtocol &&
    parsed.hostname === parsedOrigin.hostname &&
    parsed.port === parsedOrigin.port &&
    parsed.username === "" &&
    parsed.password === ""
  );
}

function networkMethod(method: string): BrowserNetworkMethod {
  if (method === "GET" || method === "POST" || method === "OPTIONS") return method;
  return "OTHER";
}
