import { describe, expect, test } from "bun:test";

import {
  BrowserProcessOwner,
  type BrowserConnectionLike,
  type BrowserLauncherLike,
  type BrowserServerLike
} from "./browserOwner";

describe("BrowserProcessOwner", () => {
  test("awaits force cleanup when launch resolves after abort", async () => {
    const calls: string[] = [];
    let resolveServer!: (server: BrowserServerLike) => void;
    const serverPromise = new Promise<BrowserServerLike>((resolve) => {
      resolveServer = resolve;
    });
    const launcher: BrowserLauncherLike = {
      launchServer: () => serverPromise,
      connect: async () => {
        throw new Error("connect must not run");
      }
    };
    const controller = new AbortController();
    const acquisition = BrowserProcessOwner.launch({
      launcher,
      signal: controller.signal,
      acquisitionTimeoutMs: 100,
      cleanupTimeoutMs: 100
    });

    controller.abort();
    resolveServer(fakeServer(calls));

    await expect(acquisition).rejects.toThrow("browser acceptance interrupted");
    expect(calls).toEqual(["server:kill"]);
  });

  test("waits through the remaining acquisition deadline for a late server", async () => {
    const calls: string[] = [];
    let resolveServer!: (server: BrowserServerLike) => void;
    const serverPromise = new Promise<BrowserServerLike>((resolve) => {
      resolveServer = resolve;
    });
    const launcher: BrowserLauncherLike = {
      launchServer: () => serverPromise,
      connect: async () => {
        throw new Error("connect must not run");
      }
    };
    const controller = new AbortController();
    let settled = false;
    const acquisition = BrowserProcessOwner.launch({
      launcher,
      signal: controller.signal,
      acquisitionTimeoutMs: 50,
      cleanupTimeoutMs: 5
    }).finally(() => {
      settled = true;
    });

    controller.abort();
    await delay(10);
    expect(settled).toBeFalse();
    resolveServer({
      wsEndpoint: () => "ws://127.0.0.1:43123/browser",
      async close() {},
      async kill() {
        calls.push("server:kill:start");
        await delay(1);
        calls.push("server:kill:end");
      }
    });

    await expect(acquisition).rejects.toThrow("browser acceptance interrupted");
    expect(calls).toEqual(["server:kill:start", "server:kill:end"]);
  });

  test("late acquisition cleanup failure is stable", async () => {
    let resolveServer!: (server: BrowserServerLike) => void;
    const serverPromise = new Promise<BrowserServerLike>((resolve) => {
      resolveServer = resolve;
    });
    const controller = new AbortController();
    const acquisition = BrowserProcessOwner.launch({
      launcher: {
        launchServer: () => serverPromise,
        connect: async () => {
          throw new Error("connect must not run");
        }
      },
      signal: controller.signal,
      acquisitionTimeoutMs: 50,
      cleanupTimeoutMs: 5
    });
    controller.abort();
    resolveServer({
      wsEndpoint: () => "ws://127.0.0.1:43123/browser",
      async close() {},
      async kill() {
        throw new Error("sensitive cleanup failure");
      }
    });

    await expect(acquisition).rejects.toThrow("browser acquisition cleanup failed");
  });

  test("browser close timeout escalates to an awaited server kill", async () => {
    const calls: string[] = [];
    const launcher: BrowserLauncherLike = {
      async launchServer() {
        return fakeServer(calls);
      },
      async connect() {
        return fakeBrowser(calls, new Promise<void>(() => {}));
      }
    };
    const owner = await BrowserProcessOwner.launch({
      launcher,
      acquisitionTimeoutMs: 100,
      cleanupTimeoutMs: 5
    });

    await expect(owner.close()).rejects.toThrow("browser cleanup failed");
    expect(calls).toEqual(["browser:close", "server:kill"]);
  });

  test("browser close rejection escalates before cleanup returns", async () => {
    const calls: string[] = [];
    const launcher: BrowserLauncherLike = {
      async launchServer() {
        return fakeServer(calls);
      },
      async connect() {
        return fakeBrowser(calls, Promise.reject(new Error("sensitive close failure")));
      }
    };
    const owner = await BrowserProcessOwner.launch({
      launcher,
      acquisitionTimeoutMs: 100,
      cleanupTimeoutMs: 100
    });

    await expect(owner.close()).rejects.toThrow("browser cleanup failed");
    expect(calls).toEqual(["browser:close", "server:kill"]);
  });
});

function fakeServer(calls: string[]): BrowserServerLike {
  return {
    wsEndpoint: () => "ws://127.0.0.1:43123/browser",
    async close() {
      calls.push("server:close");
    },
    async kill() {
      calls.push("server:kill");
    }
  };
}

function fakeBrowser(calls: string[], close: Promise<void>): BrowserConnectionLike {
  return {
    async newContext() {
      throw new Error("context must not be acquired");
    },
    async close() {
      calls.push("browser:close");
      return close;
    }
  };
}

function delay(timeoutMs: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, timeoutMs));
}
