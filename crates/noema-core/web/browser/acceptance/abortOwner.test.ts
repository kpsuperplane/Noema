import { describe, expect, test } from "bun:test";
import { EventEmitter } from "node:events";

import { BrowserAbortOwner, raceWithAbortSignal } from "./abortOwner";

describe("BrowserAbortOwner", () => {
  test("first SIGINT aborts immediately with code 130 and removes signal handlers", async () => {
    const target = new EventEmitter();
    let exitCode: number | undefined;
    const owner = new BrowserAbortOwner(target, (code) => {
      exitCode = code;
    });
    owner.install();
    const pending = owner.race(new Promise<never>(() => {}), "browser wait");

    target.emit("SIGINT");

    await expect(pending).rejects.toThrow("browser acceptance interrupted");
    expect(owner.signal.aborted).toBeTrue();
    expect(exitCode).toBe(130);
    expect(target.listenerCount("SIGINT")).toBe(0);
    expect(target.listenerCount("SIGTERM")).toBe(0);
  });

  test("first SIGTERM uses code 143 and leaves the second signal to the default owner", () => {
    const target = new EventEmitter();
    let exitCode: number | undefined;
    const owner = new BrowserAbortOwner(target, (code) => {
      exitCode = code;
    });
    owner.install();

    target.emit("SIGTERM");

    expect(exitCode).toBe(143);
    expect(target.listenerCount("SIGINT")).toBe(0);
    expect(target.listenerCount("SIGTERM")).toBe(0);
    expect(target.emit("SIGTERM")).toBeFalse();
  });

  for (const phase of ["Cargo discovery", "fixture readiness", "browser wait"]) {
    test(phase + " cancellation enters ordered cleanup", async () => {
      const target = new EventEmitter();
      const owner = new BrowserAbortOwner(target, () => {});
      owner.install();
      let cleanupStarted = false;
      const run = (async () => {
        try {
          await owner.race(new Promise<never>(() => {}), phase);
        } finally {
          cleanupStarted = true;
        }
      })();

      target.emit("SIGINT");

      await expect(run).rejects.toThrow("browser acceptance interrupted");
      expect(cleanupStarted).toBeTrue();
    });
  }

  test("an already-aborted race consumes a later operation rejection", async () => {
    const controller = new AbortController();
    controller.abort();
    let rejectOperation!: (error: Error) => void;
    const operation = new Promise<void>((_resolve, reject) => {
      rejectOperation = reject;
    });
    const unhandled: unknown[] = [];
    const onUnhandled = (error: unknown) => unhandled.push(error);
    process.on("unhandledRejection", onUnhandled);
    try {
      await expect(raceWithAbortSignal(operation, controller.signal)).rejects.toThrow(
        "browser acceptance interrupted"
      );
      rejectOperation(new Error("late owned operation rejection"));
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(unhandled).toEqual([]);
    } finally {
      process.removeListener("unhandledRejection", onUnhandled);
    }
  });
});
