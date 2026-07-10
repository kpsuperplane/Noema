import { describe, expect, test } from "bun:test";

import { ObservationOwner, type ArtifactObservationRecorder } from "./observations";

const ORIGIN = "http://127.0.0.1:43123";

describe("ObservationOwner", () => {
  test("callbacks never throw and snapshot recorder failures as harness failures", async () => {
    const recorder = throwingRecorder();
    const owner = new ObservationOwner(recorder, ORIGIN);

    expect(() => owner.onConsoleError()).not.toThrow();
    expect(() => owner.onPageError()).not.toThrow();
    expect(() => owner.onResponse("GET", ORIGIN + "/assets/app.js", 200)).not.toThrow();
    await expect(owner.onHttpRoute(fakeHttpRoute("http://example.test/"))).resolves.toBeUndefined();

    expect(owner.snapshot().failure).toEqual({ kind: "harness", step: "boot" });
  });

  test("freeze makes late events inert while retaining external route blocking", async () => {
    const calls: string[] = [];
    const owner = new ObservationOwner(recordingRecorder(calls), ORIGIN);
    owner.freeze();

    owner.onConsoleError();
    owner.onPageError();
    owner.onRequestFailed("GET", ORIGIN + "/assets/app.js");
    const external = fakeHttpRoute("http://127.0.0.1:59999/private");
    await owner.onHttpRoute(external);

    expect(calls).toEqual([]);
    expect(external.actions).toEqual(["abort"]);
    expect(owner.snapshot().failure).toBeUndefined();
  });

  test("records an external websocket failure once and closes without exposing its URL", async () => {
    const calls: string[] = [];
    const owner = new ObservationOwner(recordingRecorder(calls), ORIGIN);
    const route = fakeWebSocketRoute("ws://127.0.0.1:59999/private");

    await expect(owner.onWebSocketRoute(route)).resolves.toBeUndefined();
    await expect(owner.onWebSocketRoute(route)).resolves.toBeUndefined();

    expect(route.actions).toEqual(["close", "close"]);
    expect(owner.snapshot().failure).toEqual({ kind: "network", step: "boot" });
    expect(calls.filter((call) => call === "network:failed")).toHaveLength(1);
    expect(calls.join(" ")).not.toContain("59999");
  });

  test("records an external HTTP request only once across route and failure callbacks", async () => {
    const calls: string[] = [];
    const owner = new ObservationOwner(recordingRecorder(calls), ORIGIN);
    const route = fakeHttpRoute("http://127.0.0.1:59999/private");
    const request = route.request();

    await owner.onHttpRoute(route);
    await owner.onHttpRoute(route);
    owner.onRequestFailed("GET", request.url(), request);

    expect(calls.filter((call) => call === "network:failed")).toHaveLength(1);
  });

  test("bridges only the equivalent same-origin websocket", async () => {
    const owner = new ObservationOwner(recordingRecorder([]), ORIGIN);
    const route = fakeWebSocketRoute("ws://127.0.0.1:43123/graphql/ws");

    await owner.onWebSocketRoute(route);

    expect(route.actions).toEqual(["connect"]);
    expect(owner.snapshot().failure).toBeUndefined();
  });

  test("awaits rejected websocket close and snapshots a stable harness failure", async () => {
    const owner = new ObservationOwner(recordingRecorder([]), ORIGIN);
    const route = fakeWebSocketRoute("ws://127.0.0.1:59999/private", true);

    await expect(owner.onWebSocketRoute(route)).resolves.toBeUndefined();

    expect(owner.snapshot().failure).toEqual({ kind: "harness", step: "boot" });
  });
});

function recordingRecorder(calls: string[]): ArtifactObservationRecorder {
  return {
    recordNetwork(observation) {
      calls.push("network:" + observation.outcome);
    },
    recordConsole() {
      calls.push("console");
    },
    recordPageError() {
      calls.push("page");
    }
  };
}

function throwingRecorder(): ArtifactObservationRecorder {
  return {
    recordNetwork() {
      throw new Error("sensitive recorder failure");
    },
    recordConsole() {
      throw new Error("sensitive recorder failure");
    },
    recordPageError() {
      throw new Error("sensitive recorder failure");
    }
  };
}

function fakeHttpRoute(url: string) {
  const actions: string[] = [];
  const request = { url: () => url };
  return {
    actions,
    request() {
      return request;
    },
    async abort() {
      actions.push("abort");
    },
    async continue() {
      actions.push("continue");
    }
  };
}

function fakeWebSocketRoute(url: string, rejectClose = false) {
  const actions: string[] = [];
  const server = {
    onMessage() {},
    send() {}
  };
  return {
    actions,
    url() {
      return url;
    },
    async close() {
      actions.push("close");
      if (rejectClose) throw new Error("sensitive close failure");
    },
    connectToServer() {
      actions.push("connect");
      return server;
    },
    onMessage() {},
    send() {}
  };
}
