import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { constants } from "node:fs";
import { access, lstat, realpath } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { interruptedError, raceWithAbortSignal } from "./abortOwner.ts";
import { awaitOwnedStdioCleanup, drainBounded, waitForChildExit } from "./cleanup.ts";

const UTF8 = new TextEncoder();
const MAX_CARGO_LINE_BYTES = 256 * 1024;
const MAX_CARGO_OUTPUT_BYTES = 8 * 1024 * 1024;
const MAX_BUILD_STDERR_LINE_BYTES = 64 * 1024;
const MAX_BUILD_STDERR_BYTES = 2 * 1024 * 1024;
const BUILD_TIMEOUT_MS = 5 * 60 * 1000;
const TASKKILL_TIMEOUT_MS = 3_000;

export const REPOSITORY_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../../../..");

export interface CargoDiscoveryOptions {
  readonly command?: string;
  readonly args?: readonly string[];
  readonly repositoryRoot?: string;
  readonly buildTimeoutMs?: number;
  readonly terminateMs?: number;
  readonly signal?: AbortSignal;
}

export interface TaskkillChildLike {
  readonly pid?: number;
  readonly exitCode: number | null;
  readonly signalCode?: NodeJS.Signals | null;
  once(event: "spawn" | "error" | "close", listener: (...args: never[]) => void): unknown;
  kill(signal?: NodeJS.Signals): boolean;
}

export interface TaskkillOptions {
  readonly spawnTaskkill?: (
    command: string,
    args: readonly string[],
    environment: NodeJS.ProcessEnv
  ) => TaskkillChildLike;
  readonly timeoutMs?: number;
}

export class CargoArtifactParser {
  readonly #repositoryRoot: string;
  readonly #decoder = new TextDecoder("utf-8", { fatal: true });
  #pending = "";
  #totalBytes = 0;
  readonly #executables: string[] = [];

  constructor(repositoryRoot: string) {
    this.#repositoryRoot = resolve(repositoryRoot);
  }

  push(chunk: string | Uint8Array): void {
    let text: string;
    try {
      text = typeof chunk === "string" ? chunk : this.#decoder.decode(chunk, { stream: true });
    } catch {
      throw new Error("invalid browser fixture build output");
    }
    this.#totalBytes += UTF8.encode(text).byteLength;
    if (this.#totalBytes > MAX_CARGO_OUTPUT_BYTES) {
      throw new Error("browser fixture build output exceeded limit");
    }
    this.#pending += text;
    this.#consumeLines();
    if (UTF8.encode(this.#pending).byteLength > MAX_CARGO_LINE_BYTES) {
      throw new Error("browser fixture build output line exceeded limit");
    }
  }

  finish(): string {
    try {
      this.#pending += this.#decoder.decode();
    } catch {
      throw new Error("invalid browser fixture build output");
    }
    this.#consumeLines();
    if (this.#pending !== "") throw new Error("invalid browser fixture build output");
    if (this.#executables.length === 0) throw new Error("browser fixture build artifact missing");
    if (this.#executables.length !== 1) throw new Error("browser fixture build artifact ambiguous");
    const executable = resolve(this.#executables[0]!);
    if (
      !isWithin(this.#repositoryRoot, executable) ||
      !isWithin(join(this.#repositoryRoot, "target"), executable)
    ) {
      throw new Error("browser fixture executable is invalid");
    }
    return executable;
  }

  #consumeLines(): void {
    while (true) {
      const newline = this.#pending.indexOf("\n");
      if (newline < 0) return;
      const line = this.#pending.slice(0, newline).replace(/\r$/u, "");
      this.#pending = this.#pending.slice(newline + 1);
      if (UTF8.encode(line).byteLength > MAX_CARGO_LINE_BYTES) {
        throw new Error("browser fixture build output line exceeded limit");
      }
      this.#consumeLine(line);
    }
  }

  #consumeLine(line: string): void {
    if (line === "") return;
    let message: unknown;
    try {
      message = JSON.parse(line);
    } catch {
      throw new Error("invalid browser fixture build output");
    }
    if (!isRecord(message) || message.reason !== "compiler-artifact") return;
    if (!isRecord(message.target) || !isRecord(message.profile)) return;
    if (
      message.target.name !== "noema_core" ||
      !Array.isArray(message.target.kind) ||
      !message.target.kind.includes("lib") ||
      message.profile.test !== true
    ) {
      return;
    }
    if (typeof message.executable !== "string" || !isAbsolute(message.executable)) {
      throw new Error("invalid browser fixture build output");
    }
    this.#executables.push(message.executable);
  }
}

export async function validateFixtureExecutable(
  repositoryRoot: string,
  executable: string
): Promise<string> {
  try {
    const canonicalRepository = await realpath(repositoryRoot);
    const canonicalTarget = await realpath(join(canonicalRepository, "target"));
    const executableInfo = await lstat(executable);
    if (executableInfo.isSymbolicLink() || !executableInfo.isFile()) throw new Error("invalid");
    await access(executable, constants.X_OK);
    const canonicalExecutable = await realpath(executable);
    if (!isWithin(canonicalTarget, canonicalExecutable)) throw new Error("invalid");
    return canonicalExecutable;
  } catch {
    throw new Error("browser fixture executable is invalid");
  }
}

export function buildCargoEnvironment(
  source: NodeJS.ProcessEnv = process.env
): NodeJS.ProcessEnv {
  return Object.fromEntries(
    Object.entries(source).filter(([name]) => !name.startsWith("NOEMA_BROWSER_"))
  );
}

export async function discoverFixtureExecutable(
  options: CargoDiscoveryOptions = {}
): Promise<string> {
  const repositoryRoot = options.repositoryRoot ?? REPOSITORY_ROOT;
  const command = options.command ?? "cargo";
  const args = options.args ?? [
    "test",
    "--no-run",
    "-p",
    "noema-core",
    "--lib",
    "--message-format=json-render-diagnostics"
  ];
  let child: ChildProcessWithoutNullStreams;
  try {
    child = spawn(command, [...args], {
      cwd: repositoryRoot,
      env: buildCargoEnvironment(),
      detached: process.platform !== "win32",
      stdio: ["pipe", "pipe", "pipe"]
    });
    child.stdin.end();
  } catch {
    throw new Error("browser fixture build could not start");
  }
  try {
    await waitForSpawn(child);
  } catch {
    await joinChildDrainsAfterSpawnFailure(child);
    throw new Error("browser fixture build could not start");
  }

  const parser = new CargoArtifactParser(repositoryRoot);
  const stderrDrain = drainBounded(
    child.stderr,
    MAX_BUILD_STDERR_LINE_BYTES,
    MAX_BUILD_STDERR_BYTES,
    "browser fixture build diagnostics exceeded limit"
  );
  const stdoutDrain = (async () => {
    for await (const chunk of child.stdout) parser.push(Uint8Array.from(chunk));
  })();
  const drains = Promise.all([stdoutDrain, stderrDrain]);
  let exit: number | undefined;
  try {
    exit = await raceWithAbortSignal(
      Promise.race([
        waitForChildExit(child, options.buildTimeoutMs ?? BUILD_TIMEOUT_MS),
        drains.then(
          () => new Promise<never>(() => {}),
          () => {
            throw new Error("browser fixture build output was invalid");
          }
        )
      ]),
      options.signal
    );
  } catch {
    await terminateAndDrain(child, drains, options.terminateMs ?? 3_000);
    if (options.signal?.aborted === true) throw interruptedError();
    throw new Error("browser fixture build output was invalid");
  }
  if (exit === undefined) {
    await terminateAndDrain(child, drains, options.terminateMs ?? 3_000);
    throw new Error("browser fixture build timed out");
  }
  try {
    await awaitOwnedStdioCleanup(drains.then(() => undefined), child);
  } catch {
    throw new Error("browser fixture build output was invalid");
  }
  if (exit !== 0) throw new Error("browser fixture build failed");
  return validateFixtureExecutable(repositoryRoot, parser.finish());
}

async function terminateAndDrain(
  child: ChildProcessWithoutNullStreams,
  drains: Promise<unknown>,
  terminateMs: number
): Promise<void> {
  let terminationError: unknown;
  try {
    await terminateAndJoinProcess(child, terminateMs);
  } catch (error) {
    terminationError = error;
  } finally {
    await awaitOwnedStdioCleanup(drains.then(() => undefined), child).catch(() => undefined);
  }
  if (terminationError !== undefined) throw terminationError;
}

async function waitForSpawn(child: ChildProcessWithoutNullStreams): Promise<void> {
  if (child.pid !== undefined) return;
  await new Promise<void>((resolveSpawn, rejectSpawn) => {
    child.once("spawn", resolveSpawn);
    child.once("error", rejectSpawn);
  });
}

async function joinChildDrainsAfterSpawnFailure(
  child: ChildProcessWithoutNullStreams
): Promise<void> {
  const completion = Promise.allSettled([
    drainBounded(child.stdout, MAX_CARGO_LINE_BYTES, MAX_CARGO_OUTPUT_BYTES, "invalid"),
    drainBounded(child.stderr, MAX_BUILD_STDERR_LINE_BYTES, MAX_BUILD_STDERR_BYTES, "invalid")
  ]).then(() => undefined);
  await awaitOwnedStdioCleanup(completion, child).catch(() => undefined);
}

async function terminateAndJoinProcess(
  child: ChildProcessWithoutNullStreams,
  terminateMs: number
): Promise<void> {
  const processGroupId = process.platform === "win32" || child.pid === undefined ? undefined : child.pid;
  await signalProcessTree(child, processGroupId, false, terminateMs);
  const treeExited =
    processGroupId === undefined
      ? (await waitForChildExit(child, terminateMs)) !== undefined
      : await waitForProcessGroupExit(processGroupId, terminateMs);
  if (!treeExited) await signalProcessTree(child, processGroupId, true, terminateMs);
  const finalTreeExited =
    processGroupId === undefined
      ? (await waitForChildExit(child, terminateMs)) !== undefined
      : await waitForProcessGroupExit(processGroupId, terminateMs);
  const leaderExited = (await waitForChildExit(child, terminateMs)) !== undefined;
  if (!finalTreeExited || !leaderExited) throw new Error("browser fixture process did not exit");
}

async function signalProcessTree(
  child: ChildProcessWithoutNullStreams,
  processGroupId: number | undefined,
  force: boolean,
  timeoutMs: number
): Promise<void> {
  if (process.platform === "win32" && child.pid !== undefined) {
    await runWindowsTaskkill(child.pid, force, { timeoutMs });
    return;
  }
  const signal = force ? "SIGKILL" : "SIGTERM";
  if (processGroupId !== undefined) {
    try {
      process.kill(-processGroupId, signal);
      return;
    } catch (error) {
      if (isNoSuchProcess(error)) return;
    }
  }
  if (child.exitCode === null) child.kill(signal);
}

async function waitForProcessGroupExit(processGroupId: number, timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!processGroupExists(processGroupId)) return true;
    await new Promise((resolvePoll) => setTimeout(resolvePoll, 10));
  }
  return !processGroupExists(processGroupId);
}

function processGroupExists(processGroupId: number): boolean {
  try {
    process.kill(-processGroupId, 0);
    return true;
  } catch (error) {
    return !isNoSuchProcess(error);
  }
}

function isNoSuchProcess(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code?: unknown }).code === "ESRCH"
  );
}

export async function runWindowsTaskkill(
  pid: number,
  force: boolean,
  options: TaskkillOptions = {}
): Promise<void> {
  const timeoutMs = options.timeoutMs ?? TASKKILL_TIMEOUT_MS;
  const spawnTaskkill =
    options.spawnTaskkill ??
    ((command: string, args: readonly string[], environment: NodeJS.ProcessEnv) =>
      spawn(command, [...args], { env: environment, stdio: "ignore" }) as unknown as TaskkillChildLike);
  let helper: TaskkillChildLike;
  try {
    helper = spawnTaskkill("taskkill", windowsTaskkillArgs(pid, force), buildCargoEnvironment());
  } catch {
    throw new Error("browser fixture process termination failed");
  }
  try {
    await waitForTaskkillSpawn(helper);
  } catch {
    await joinTaskkillHelper(helper, timeoutMs);
    throw new Error("browser fixture process termination failed");
  }
  const exit = await waitForChildExit(helper, timeoutMs);
  if (exit !== undefined) {
    validateTaskkillExit(exit);
    return;
  }
  await killAndJoinTaskkillHelper(helper, timeoutMs);
  throw new Error("browser fixture process termination failed");
}

async function joinTaskkillHelper(helper: TaskkillChildLike, timeoutMs: number): Promise<void> {
  if ((await waitForChildExit(helper, timeoutMs)) !== undefined) return;
  await killAndJoinTaskkillHelper(helper, timeoutMs);
}

async function killAndJoinTaskkillHelper(
  helper: TaskkillChildLike,
  timeoutMs: number
): Promise<void> {
  try {
    helper.kill("SIGKILL");
  } catch {
    throw new Error("browser fixture process termination failed");
  }
  if ((await waitForChildExit(helper, timeoutMs)) === undefined) {
    throw new Error("browser fixture process termination failed");
  }
}

function waitForTaskkillSpawn(helper: TaskkillChildLike): Promise<void> {
  if (helper.pid !== undefined) return Promise.resolve();
  return new Promise((resolveSpawn, rejectSpawn) => {
    helper.once("spawn", resolveSpawn);
    helper.once("error", rejectSpawn);
  });
}

export function windowsTaskkillArgs(pid: number, force: boolean): string[] {
  const args = ["/pid", String(pid), "/t"];
  if (force) args.push("/f");
  return args;
}

export function validateTaskkillExit(exitCode: number | undefined): void {
  if (exitCode !== 0) throw new Error("browser fixture process termination failed");
}

function isWithin(root: string, candidate: string): boolean {
  const path = relative(resolve(root), resolve(candidate));
  return path !== "" && path !== ".." && !path.startsWith(`..${sep}`) && !isAbsolute(path);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && Object.getPrototypeOf(value) === Object.prototype;
}
