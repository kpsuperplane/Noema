import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { chmod, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ReadinessParser, type BrowserReady } from "./protocol.ts";
import {
  awaitOwnedStdioCleanup,
  boundedCleanup,
  drainBounded,
  waitForChildExit
} from "./cleanup.ts";
import { raceWithAbortSignal } from "./abortOwner.ts";
import { discoverFixtureExecutable, REPOSITORY_ROOT } from "./cargoFixture.ts";

const MAX_FIXTURE_STDERR_LINE_BYTES = 16 * 1024;
const MAX_FIXTURE_STDERR_BYTES = 256 * 1024;
const READINESS_TIMEOUT_MS = 30 * 1000;
const CLEANUP_TIMEOUT_MS = 3_000;

export { REPOSITORY_ROOT } from "./cargoFixture.ts";

export interface FixtureHome {
  readonly root: string;
  readonly home: string;
  readonly cache: string;
  readonly runtime: string;
  readonly noemaHome: string;
  readonly artifactRoot: string;
  readonly temp: string;
  dispose(options?: { readonly retainArtifacts?: boolean }): Promise<void>;
}

export interface StoppableFixtureChild {
  writeShutdown(): Promise<void>;
  waitForExit(timeoutMs: number): Promise<number | undefined>;
  terminate(): void;
  forceKill(): void;
}

export interface ShutdownDeadlines {
  readonly gracefulMs: number;
  readonly terminateMs: number;
}

export interface FixtureStreamOwner {
  readonly readiness: Promise<BrowserReady>;
  readonly completion: Promise<void>;
}

export interface FixtureStreamLimits {
  readonly maxStderrLineBytes?: number;
  readonly maxStderrBytes?: number;
}

export async function createFixtureHome(): Promise<FixtureHome> {
  const root = await mkdtemp(join(tmpdir(), "noema-browser-fixture-"));
  const home = join(root, "home");
  const cache = join(root, "cache");
  const runtime = join(root, "runtime");
  const noemaHome = join(root, "noema");
  const artifactRoot = join(root, "artifacts");
  const temp = join(root, "temp");
  try {
    await privateDirectory(root);
    await Promise.all(
      [home, cache, runtime, noemaHome, artifactRoot, temp].map((path) => privateDirectory(path))
    );
  } catch (error) {
    await rm(root, { recursive: true, force: true });
    throw error;
  }
  let disposed = false;
  return Object.freeze({
    root,
    home,
    cache,
    runtime,
    noemaHome,
    artifactRoot,
    temp,
    async dispose(options: { readonly retainArtifacts?: boolean } = {}) {
      if (disposed) return;
      disposed = true;
      if (options.retainArtifacts === true) {
        await Promise.all(
          [home, cache, runtime, noemaHome, temp].map((path) =>
            rm(path, { recursive: true, force: true })
          )
        );
        return;
      }
      await rm(root, { recursive: true, force: true });
    }
  });
}

export function buildFixtureEnvironment(
  fixture: FixtureHome,
  source: NodeJS.ProcessEnv = process.env
): NodeJS.ProcessEnv {
  if (typeof source.PATH !== "string" || source.PATH.length === 0) {
    throw new Error("browser fixture PATH is unavailable");
  }
  const environment: NodeJS.ProcessEnv = {
    PATH: source.PATH,
    HOME: fixture.home,
    USERPROFILE: fixture.home,
    XDG_CACHE_HOME: fixture.cache,
    XDG_RUNTIME_DIR: fixture.runtime,
    TMPDIR: fixture.temp,
    TMP: fixture.temp,
    TEMP: fixture.temp,
    NOEMA_HOME: fixture.noemaHome,
    NOEMA_BROWSER_SCENARIO: "boot",
    RUST_BACKTRACE: "0",
    LANG: "C.UTF-8",
    LC_ALL: "C.UTF-8"
  };
  if (process.platform === "win32") {
    for (const name of ["SystemRoot", "WINDIR", "ComSpec", "PATHEXT"] as const) {
      if (typeof source[name] === "string") environment[name] = source[name];
    }
  }
  return environment;
}

export async function stopFixtureChild(
  child: StoppableFixtureChild,
  deadlines: ShutdownDeadlines = { gracefulMs: 5_000, terminateMs: 3_000 }
): Promise<void> {
  try {
    await boundedCleanup(child.writeShutdown(), deadlines.gracefulMs);
  } catch {
    await escalateFixtureChild(child, deadlines.terminateMs);
    return;
  }
  const gracefulExit = await child.waitForExit(deadlines.gracefulMs);
  if (gracefulExit !== undefined) {
    if (gracefulExit !== 0) throw new Error("browser fixture exited unsuccessfully");
    return;
  }
  await escalateFixtureChild(child, deadlines.terminateMs);
}

async function escalateFixtureChild(
  child: StoppableFixtureChild,
  terminateMs: number
): Promise<void> {
  child.terminate();
  const terminatedExit = await child.waitForExit(terminateMs);
  if (terminatedExit !== undefined) return;
  child.forceKill();
  const killedExit = await child.waitForExit(terminateMs);
  if (killedExit === undefined) throw new Error("browser fixture did not exit");
}

export class FixtureProcess {
  readonly ready: BrowserReady;
  readonly home: FixtureHome;
  #child: ChildProcessWithoutNullStreams | undefined;
  readonly #outputCompletion: Promise<void>;
  #closed = false;

  private constructor(
    child: ChildProcessWithoutNullStreams,
    ready: BrowserReady,
    home: FixtureHome,
    outputCompletion: Promise<void>
  ) {
    this.#child = child;
    this.ready = ready;
    this.home = home;
    this.#outputCompletion = outputCompletion;
  }

  static async launch(signal?: AbortSignal): Promise<FixtureProcess> {
    const executable = await discoverFixtureExecutable({ signal });
    const home = await createFixtureHome();
    let child: ChildProcessWithoutNullStreams | undefined;
    let outputCompletion: Promise<void> | undefined;
    try {
      child = spawn(
        executable,
        [
          "--ignored",
          "--exact",
          "daemon::browser_acceptance::fixture_server",
          "--nocapture",
          "--test-threads=1"
        ],
        {
          cwd: REPOSITORY_ROOT,
          env: buildFixtureEnvironment(home),
          stdio: ["pipe", "pipe", "pipe"]
        }
      );
      const output = observeFixtureStreams(child.stdout, child.stderr);
      outputCompletion = output.completion;
      const ready = await raceWithAbortSignal(
        withDeadline(
          output.readiness,
          READINESS_TIMEOUT_MS,
          "browser fixture readiness timed out"
        ),
        signal
      );
      return new FixtureProcess(child, ready, home, output.completion);
    } catch (error) {
      if (child !== undefined) {
        try {
          await stopFixtureChild(new NodeFixtureChild(child));
        } catch {
          child.kill("SIGKILL");
          await waitForChildExit(child, 3_000);
        }
      }
      if (child !== undefined && outputCompletion !== undefined) {
        await awaitOwnedStdioCleanup(
          outputCompletion,
          child,
          CLEANUP_TIMEOUT_MS,
          "browser fixture output cleanup failed"
        ).catch(() => undefined);
      }
      let homeCleanupFailed = false;
      try {
        await disposeFixtureHome(home);
      } catch {
        homeCleanupFailed = true;
      }
      if (homeCleanupFailed) throw new Error("browser fixture private root cleanup failed");
      throw stableError(error, "browser fixture failed to start");
    }
  }

  async close(options: { readonly retainArtifacts?: boolean } = {}): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    const child = this.#child;
    this.#child = undefined;
    let shutdownError: unknown;
    let outputError: unknown;
    try {
      if (child !== undefined) {
        try {
          await stopFixtureChild(new NodeFixtureChild(child));
        } catch (error) {
          shutdownError = error;
        }
      }
      try {
        if (child !== undefined) {
          await awaitOwnedStdioCleanup(
            this.#outputCompletion,
            child,
            CLEANUP_TIMEOUT_MS,
            "browser fixture output cleanup failed"
          );
        } else {
          await boundedCleanup(this.#outputCompletion, CLEANUP_TIMEOUT_MS);
        }
      } catch (error) {
        outputError = error;
      }
      if (shutdownError !== undefined) throw shutdownError;
      if (outputError !== undefined) throw outputError;
    } finally {
      await disposeFixtureHome(this.home, options);
    }
  }
}

async function disposeFixtureHome(
  home: FixtureHome,
  options: { readonly retainArtifacts?: boolean } = {}
): Promise<void> {
  try {
    await boundedCleanup(home.dispose(options), CLEANUP_TIMEOUT_MS);
    return;
  } catch {
    const removals =
      options.retainArtifacts === true
        ? [home.home, home.cache, home.runtime, home.noemaHome, home.temp]
        : [home.root];
    try {
      await boundedCleanup(
        Promise.all(removals.map((path) => rm(path, { recursive: true, force: true }))),
        CLEANUP_TIMEOUT_MS
      );
    } catch {
      throw new Error("browser fixture private root cleanup failed");
    }
    throw new Error("browser fixture private root cleanup failed");
  }
}

export function observeFixtureStreams(
  stdout: AsyncIterable<Uint8Array | string>,
  stderr: AsyncIterable<Uint8Array | string>,
  limits: FixtureStreamLimits = {}
): FixtureStreamOwner {
  const parser = new ReadinessParser();
  let resolveReadiness!: (ready: BrowserReady) => void;
  let rejectReadiness!: (error: Error) => void;
  let readinessSettled = false;
  const readiness = new Promise<BrowserReady>((resolveReady, rejectReady) => {
    resolveReadiness = resolveReady;
    rejectReadiness = rejectReady;
  });
  const stdoutCompletion = (async () => {
    try {
      for await (const chunk of stdout) {
        parser.push(typeof chunk === "string" ? chunk : Uint8Array.from(chunk));
        if (!readinessSettled && parser.ready !== undefined) {
          readinessSettled = true;
          resolveReadiness(parser.ready);
        }
      }
      const finalReady = parser.finish();
      if (!readinessSettled) {
        readinessSettled = true;
        resolveReadiness(finalReady);
      }
    } catch {
      const error = new Error("browser fixture output was invalid");
      if (!readinessSettled) {
        readinessSettled = true;
        rejectReadiness(error);
      }
      throw error;
    }
  })();
  const stderrCompletion = drainBounded(
    stderr,
    limits.maxStderrLineBytes ?? MAX_FIXTURE_STDERR_LINE_BYTES,
    limits.maxStderrBytes ?? MAX_FIXTURE_STDERR_BYTES,
    "browser fixture diagnostics were invalid"
  ).catch(() => {
    const error = new Error("browser fixture diagnostics were invalid");
    if (!readinessSettled) {
      readinessSettled = true;
      rejectReadiness(error);
    }
    throw error;
  });
  const completion = Promise.all([stdoutCompletion, stderrCompletion]).then(() => undefined);
  void readiness.catch(() => undefined);
  void completion.catch(() => undefined);
  return Object.freeze({ readiness, completion });
}

class NodeFixtureChild implements StoppableFixtureChild {
  readonly #child: ChildProcessWithoutNullStreams;

  constructor(child: ChildProcessWithoutNullStreams) {
    this.#child = child;
  }

  async writeShutdown(): Promise<void> {
    if (this.#child.exitCode !== null) return;
    await new Promise<void>((resolveWrite, rejectWrite) => {
      this.#child.stdin.end("shutdown\n", (error?: Error | null) => {
        if (error) rejectWrite(new Error("browser fixture shutdown write failed"));
        else resolveWrite();
      });
    });
  }

  waitForExit(timeoutMs: number): Promise<number | undefined> {
    return waitForChildExit(this.#child, timeoutMs);
  }

  terminate(): void {
    this.#child.kill("SIGTERM");
  }

  forceKill(): void {
    this.#child.kill("SIGKILL");
  }
}

async function withDeadline<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      promise,
      new Promise<never>((_resolve, reject) => {
        timer = setTimeout(() => reject(new Error(message)), timeoutMs);
      })
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

async function privateDirectory(path: string): Promise<void> {
  await mkdir(path, { recursive: true, mode: 0o700 });
  if (process.platform !== "win32") await chmod(path, 0o700);
}

function stableError(error: unknown, fallback: string): Error {
  if (error instanceof Error && error.message.startsWith("browser fixture")) return error;
  return new Error(fallback);
}
