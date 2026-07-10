import { describe, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { Readable } from "node:stream";

import { awaitOwnedStdioCleanup } from "./cleanup";
import {
  buildFixtureEnvironment,
  createFixtureHome,
  observeFixtureStreams,
  stopFixtureChild,
  type StoppableFixtureChild
} from "./fixtureProcess";

describe("fixture environment", () => {
  test("uses a private owned root and excludes ambient credentials and caller homes", async () => {
    const fixture = await createFixtureHome();
    try {
      expect(existsSync(fixture.root)).toBeTrue();
      const environment = buildFixtureEnvironment(fixture, {
        PATH: "/safe/bin",
        HOME: "/caller/home",
        USERPROFILE: "C:\\caller",
        OPENAI_API_KEY: "secret",
        CODEX_HOME: "/caller/codex",
        NOEMA_BROWSER_FORCE_SAFE_FAILURE: "1",
        LANG: "host-locale"
      });
      expect(environment.PATH).toBe("/safe/bin");
      expect(environment.HOME).toBe(fixture.home);
      expect(environment.USERPROFILE).toBe(fixture.home);
      expect(environment.NOEMA_HOME).toBe(fixture.noemaHome);
      expect(environment.NOEMA_BROWSER_SCENARIO).toBe("boot");
      expect(environment.RUST_BACKTRACE).toBe("0");
      expect(environment.LANG).toBe("C.UTF-8");
      expect(environment.OPENAI_API_KEY).toBeUndefined();
      expect(environment.CODEX_HOME).toBeUndefined();
      expect(environment.NOEMA_BROWSER_FORCE_SAFE_FAILURE).toBeUndefined();
    } finally {
      await fixture.dispose();
    }
    expect(existsSync(fixture.root)).toBeFalse();
  });
});

describe("fixture shutdown", () => {
  test("accepts graceful zero exit without escalation", async () => {
    const child = fakeChild([0]);
    await stopFixtureChild(child, { gracefulMs: 1, terminateMs: 1 });
    expect(child.calls).toEqual(["shutdown", "wait:1"]);
  });

  test("escalates from shutdown to terminate then force-kill", async () => {
    const child = fakeChild([undefined, undefined, 0]);
    await stopFixtureChild(child, { gracefulMs: 1, terminateMs: 2 });
    expect(child.calls).toEqual([
      "shutdown",
      "wait:1",
      "terminate",
      "wait:2",
      "kill",
      "wait:2"
    ]);
  });

  test("rejects a nonzero graceful fixture exit", async () => {
    const child = fakeChild([2]);
    await expect(stopFixtureChild(child, { gracefulMs: 1, terminateMs: 1 })).rejects.toThrow(
      "browser fixture exited unsuccessfully"
    );
  });

  test("escalates when the shutdown pipe cannot be written", async () => {
    const calls: string[] = [];
    const child: StoppableFixtureChild = {
      async writeShutdown() {
        calls.push("shutdown");
        throw new Error("closed");
      },
      async waitForExit(timeoutMs) {
        calls.push("wait:" + timeoutMs);
        return calls.length >= 5 ? 0 : undefined;
      },
      terminate() {
        calls.push("terminate");
      },
      forceKill() {
        calls.push("kill");
      }
    };

    await stopFixtureChild(child, { gracefulMs: 1, terminateMs: 2 });
    expect(calls).toEqual(["shutdown", "terminate", "wait:2", "kill", "wait:2"]);
  });
});

describe("fixture stream ownership", () => {
  const readyFrame =
    'NOEMA_BROWSER_READY {"schemaVersion":1,"kind":"ready","scenario":"boot","origin":"http://127.0.0.1:43123","startUrl":"http://127.0.0.1:43123/"}\n';

  test("joins an early EOF and rejects readiness with a stable error", async () => {
    const owner = observeFixtureStreams(Readable.from([]), Readable.from([]));
    await expect(owner.readiness).rejects.toThrow("browser fixture output was invalid");
    await expect(owner.completion).rejects.toThrow("browser fixture output was invalid");
  });

  test("surfaces a duplicate readiness frame that arrives after readiness", async () => {
    const owner = observeFixtureStreams(
      Readable.from([readyFrame, "libtest noise\n", readyFrame]),
      Readable.from([])
    );
    await expect(owner.readiness).resolves.toMatchObject({ scenario: "boot" });
    await expect(owner.completion).rejects.toThrow("browser fixture output was invalid");
  });

  test("rejects truncated readiness at EOF", async () => {
    const owner = observeFixtureStreams(
      Readable.from([readyFrame.slice(0, -1)]),
      Readable.from([])
    );
    await expect(owner.readiness).rejects.toThrow("browser fixture output was invalid");
    await expect(owner.completion).rejects.toThrow("browser fixture output was invalid");
  });

  test("joins stderr and enforces its byte cap", async () => {
    const owner = observeFixtureStreams(
      Readable.from([readyFrame]),
      Readable.from(["12345"]),
      { maxStderrBytes: 4, maxStderrLineBytes: 4 }
    );
    await expect(owner.readiness).resolves.toMatchObject({ scenario: "boot" });
    await expect(owner.completion).rejects.toThrow("browser fixture diagnostics were invalid");
  });

  test("bounds hung drain cleanup and destroys every owned stdio stream", async () => {
    const destroyed: string[] = [];
    const streams = {
      stdin: { destroy() { destroyed.push("stdin"); } },
      stdout: { destroy() { destroyed.push("stdout"); } },
      stderr: { destroy() { destroyed.push("stderr"); } }
    };

    await expect(
      awaitOwnedStdioCleanup(
        new Promise<void>(() => {}),
        streams,
        5,
        "browser fixture output cleanup failed"
      )
    ).rejects.toThrow("browser fixture output cleanup failed");
    expect(destroyed).toEqual(["stdin", "stdout", "stderr"]);
  });
});

function fakeChild(exits: Array<number | undefined>): StoppableFixtureChild & {
  calls: string[];
} {
  const calls: string[] = [];
  return {
    calls,
    async writeShutdown() {
      calls.push("shutdown");
    },
    async waitForExit(timeoutMs) {
      calls.push(`wait:${timeoutMs}`);
      return exits.shift();
    },
    terminate() {
      calls.push("terminate");
    },
    forceKill() {
      calls.push("kill");
    }
  };
}
