import { describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { chmod, mkdir, mkdtemp, readFile, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import {
  CargoArtifactParser,
  buildCargoEnvironment,
  discoverFixtureExecutable,
  runWindowsTaskkill,
  validateFixtureExecutable,
  validateTaskkillExit,
  windowsTaskkillArgs,
  type TaskkillChildLike
} from "./cargoFixture";

function compilerArtifact(executable: string): string {
  return JSON.stringify({
    reason: "compiler-artifact",
    package_id: "path+file:///repo/crates/noema-core#0.1.0",
    target: { name: "noema_core", kind: ["lib"], crate_types: ["lib"] },
    profile: { test: true },
    executable
  });
}

describe("CargoArtifactParser", () => {
  test("discovers one noema-core libtest artifact across chunks", () => {
    const root = resolve("/repo");
    const executable = resolve(root, "target/debug/deps/noema_core-test");
    const parser = new CargoArtifactParser(root);
    const output = `${JSON.stringify({ reason: "build-script-executed" })}\n${compilerArtifact(executable)}\n`;
    for (let index = 0; index < output.length; index += 3) {
      parser.push(output.slice(index, index + 3));
    }
    expect(parser.finish()).toBe(executable);
  });

  test("decodes UTF-8 cargo records split between bytes", () => {
    const root = resolve("/repo");
    const executable = resolve(root, "target/debug/deps/noema_core-test");
    const output = `${JSON.stringify({ reason: "build-script-executed", package_id: "fixture-🚆" })}\n${compilerArtifact(executable)}\n`;
    const parser = new CargoArtifactParser(root);
    for (const byte of new TextEncoder().encode(output)) parser.push(Uint8Array.of(byte));
    expect(parser.finish()).toBe(executable);
  });

  test("rejects none, multiple, malformed, and outside-repository executables", () => {
    const root = resolve("/repo");
    expect(() => new CargoArtifactParser(root).finish()).toThrow(
      "browser fixture build artifact missing"
    );
    const multiple = new CargoArtifactParser(root);
    multiple.push(`${compilerArtifact(resolve(root, "target/a"))}\n`);
    multiple.push(`${compilerArtifact(resolve(root, "target/b"))}\n`);
    expect(() => multiple.finish()).toThrow("browser fixture build artifact ambiguous");
    const malformed = new CargoArtifactParser(root);
    expect(() => malformed.push("{\"reason\":\n")).toThrow(
      "invalid browser fixture build output"
    );
    const outside = new CargoArtifactParser(root);
    outside.push(`${compilerArtifact(resolve("/outside/noema_core-test"))}\n`);
    expect(() => outside.finish()).toThrow("browser fixture executable is invalid");
  });
});

describe("fixture executable validation", () => {
  test("accepts only a canonical regular target file and rejects a symlink escape", async () => {
    const root = await mkdtemp(join(tmpdir(), "noema-fixture-executable-"));
    const repository = join(root, "repo");
    const target = join(repository, "target");
    const outside = join(root, "outside");
    await mkdir(target, { recursive: true });
    await writeFile(outside, "fixture");
    await chmod(outside, 0o700);
    const regular = join(target, "regular");
    await writeFile(regular, "fixture");
    await chmod(regular, 0o700);
    const escaped = join(target, "escaped");
    await symlink(outside, escaped);
    try {
      await expect(validateFixtureExecutable(repository, regular)).resolves.toBe(
        await realpath(regular)
      );
      await expect(validateFixtureExecutable(repository, escaped)).rejects.toThrow(
        "browser fixture executable is invalid"
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

describe("Cargo process ownership", () => {
  test("reports a stable spawn failure", async () => {
    await expect(
      discoverFixtureExecutable({
        command: join(tmpdir(), "missing-noema-cargo"),
        args: [],
        buildTimeoutMs: 50,
        terminateMs: 50
      })
    ).rejects.toThrow("browser fixture build could not start");
  });

  test("reports a stable nonzero build failure after draining output", async () => {
    await expect(
      discoverFixtureExecutable({
        command: process.execPath,
        args: ["-e", "process.stdout.write('{}\\n'); process.stderr.write('diagnostic\\n'); process.exit(2)"],
        buildTimeoutMs: 500,
        terminateMs: 100
      })
    ).rejects.toThrow("browser fixture build failed");
  });

  test("terminates and joins a timed-out build process tree", async () => {
    await expect(
      discoverFixtureExecutable({
        command: process.execPath,
        args: ["-e", "setInterval(() => {}, 1000)"],
        buildTimeoutMs: 30,
        terminateMs: 100
      })
    ).rejects.toThrow("browser fixture build timed out");
  });

  test("aborts Cargo discovery into joined process cleanup", async () => {
    const controller = new AbortController();
    const discovery = discoverFixtureExecutable({
      command: process.execPath,
      args: ["-e", "setInterval(() => {}, 1000)"],
      buildTimeoutMs: 5_000,
      terminateMs: 100,
      signal: controller.signal
    });
    setTimeout(() => controller.abort(), 20);
    await expect(discovery).rejects.toThrow("browser acceptance interrupted");
  });

  test("kills a signal-ignoring Unix descendant after the Cargo leader exits", async () => {
    if (process.platform === "win32") return;
    const nodeExecutable = execFileSync("which", ["node"], { encoding: "utf8" }).trim();
    const root = await mkdtemp(join(tmpdir(), "noema-process-group-"));
    const sentinel = join(root, "descendant.pid");
    const descendantScript = "process.on('SIGTERM',()=>{}); setInterval(()=>{},1000)";
    const parentScript = [
      "const fs=require('node:fs')",
      "const cp=require('node:child_process')",
      "const child=cp.spawn(process.execPath,['-e'," + JSON.stringify(descendantScript) + "],{stdio:'ignore'})",
      "fs.writeFileSync(process.argv[1],String(child.pid))",
      "setInterval(()=>{},1000)"
    ].join(";");
    try {
      await expect(
        discoverFixtureExecutable({
          command: nodeExecutable,
          args: ["-e", parentScript, sentinel],
          buildTimeoutMs: 100,
          terminateMs: 200
        })
      ).rejects.toThrow("browser fixture build timed out");
      const descendantPid = Number(await readFile(sentinel, "utf8"));
      expect(() => process.kill(descendantPid, 0)).toThrow();
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test("defines and validates the Windows taskkill tree contract", () => {
    expect(windowsTaskkillArgs(42, false)).toEqual(["/pid", "42", "/t"]);
    expect(windowsTaskkillArgs(42, true)).toEqual(["/pid", "42", "/t", "/f"]);
    expect(() => validateTaskkillExit(0)).not.toThrow();
    expect(() => validateTaskkillExit(1)).toThrow("browser fixture process termination failed");
  });

  test("taskkill spawn errors are stable", async () => {
    await expect(
      runWindowsTaskkill(42, false, {
        spawnTaskkill() {
          throw new Error("sensitive spawn failure");
        },
        timeoutMs: 5
      })
    ).rejects.toThrow("browser fixture process termination failed");
  });

  test("taskkill asynchronous spawn errors await the helper close", async () => {
    const calls: string[] = [];
    const child = fakeTaskkillSpawnError(calls);
    await expect(
      runWindowsTaskkill(42, false, {
        spawnTaskkill: () => child,
        timeoutMs: 20
      })
    ).rejects.toThrow("browser fixture process termination failed");
    expect(calls).toEqual(["helper:error", "helper:close"]);
  });

  test("taskkill timeout force-kills and joins its helper before returning", async () => {
    const calls: string[] = [];
    const child = fakeTaskkillChild(calls);
    await expect(
      runWindowsTaskkill(42, true, {
        spawnTaskkill: () => child,
        timeoutMs: 5
      })
    ).rejects.toThrow("browser fixture process termination failed");
    expect(calls).toEqual(["helper:kill", "helper:close"]);
  });
});

describe("Cargo environment", () => {
  test("preserves toolchain cache state but removes all browser harness variables", () => {
    const environment = buildCargoEnvironment({
      PATH: "/safe/bin",
      CARGO_BUILD_RUSTC_WRAPPER: "/safe/sccache",
      RUSTC_WRAPPER: "/safe/sccache",
      SCCACHE_DIR: "/safe/cache",
      NOEMA_BROWSER_SCENARIO: "secret-scenario",
      NOEMA_BROWSER_FORCE_SAFE_FAILURE: "1",
      NOEMA_BROWSER_FUTURE_TOKEN: "secret"
    });
    expect(environment).toEqual({
      PATH: "/safe/bin",
      CARGO_BUILD_RUSTC_WRAPPER: "/safe/sccache",
      RUSTC_WRAPPER: "/safe/sccache",
      SCCACHE_DIR: "/safe/cache"
    });
  });
});

function fakeTaskkillChild(calls: string[]): TaskkillChildLike {
  let closeListener: ((code: number | null) => void) | undefined;
  return {
    pid: 99,
    exitCode: null,
    signalCode: null,
    once(event, listener) {
      if (event === "close") closeListener = listener as (code: number | null) => void;
      return this;
    },
    kill() {
      calls.push("helper:kill");
      queueMicrotask(() => {
        calls.push("helper:close");
        closeListener?.(null);
      });
      return true;
    }
  };
}

function fakeTaskkillSpawnError(calls: string[]): TaskkillChildLike {
  const listeners = new Map<string, (...args: never[]) => void>();
  const child: TaskkillChildLike = {
    exitCode: null,
    signalCode: null,
    once(event, listener) {
      listeners.set(event, listener);
      if (event === "error") {
        queueMicrotask(() => {
          calls.push("helper:error");
          listeners.get("error")?.();
          setTimeout(() => {
            calls.push("helper:close");
            listeners.get("close")?.(null as never);
          }, 1);
        });
      }
      return this;
    },
    kill() {
      return false;
    }
  };
  return child;
}
