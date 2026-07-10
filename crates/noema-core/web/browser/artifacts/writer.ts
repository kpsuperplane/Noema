import {
  lstatSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync
} from "node:fs";
import {
  chmod,
  lstat,
  mkdir,
  open,
  readFile,
  readdir,
  rename,
  rm
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, sep } from "node:path";
import { randomUUID } from "node:crypto";

import {
  MAX_BROWSER_ARTIFACT_EVENTS,
  MAX_BROWSER_ARTIFACT_JSON_BYTES,
  MAX_BROWSER_ARTIFACT_SCREENSHOT_BYTES,
  MAX_BROWSER_ARTIFACT_STEPS,
  type BrowserArtifactEvent,
  type BrowserArtifactFailureKind,
  type BrowserArtifactProfile,
  type BrowserConsoleLevel,
  type BrowserFailureTrace,
  type BrowserNetworkMethod,
  type BrowserNetworkOutcome,
  buildSecretCanaryRepresentations,
  classifyBrowserUrl,
  normalizeConsoleLevel,
  validateBrowserArtifactId,
  validateBrowserArtifactProfile,
  validateDaemonOrigin,
  validateEventCount,
  validateFailureKind,
  validateHttpStatus,
  validateNetworkMethod,
  validateNetworkOutcome
} from "./model.ts";

const TRACE_FILENAME = "failure-trace.json";
const SCREENSHOT_FILENAME = "screenshot.png";
const PNG_SIGNATURE = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const UTF8 = new TextEncoder();
const INTRINSIC_UINT8_ARRAY = Uint8Array;
const INTRINSIC_UINT8_ARRAY_SET = Uint8Array.prototype.set;
const TYPED_ARRAY_PROTOTYPE = Object.getPrototypeOf(Uint8Array.prototype) as object;
const TYPED_ARRAY_BUFFER_GETTER = requiredIntrinsicGetter<ArrayBufferLike>("buffer");
const TYPED_ARRAY_BYTE_LENGTH_GETTER = requiredIntrinsicGetter<number>("byteLength");
const TYPED_ARRAY_BYTE_OFFSET_GETTER = requiredIntrinsicGetter<number>("byteOffset");
const TYPED_ARRAY_TAG_GETTER = requiredIntrinsicGetter<string>(Symbol.toStringTag);

export interface BrowserArtifactRecorderOptions {
  readonly artifactRoot: string;
  readonly scenarioId: string;
  readonly allowedStepIds: readonly string[];
  readonly daemonOrigin: string;
  readonly profile: BrowserArtifactProfile;
  readonly canaries: readonly string[];
}

export interface BrowserNetworkObservation {
  readonly method: BrowserNetworkMethod;
  readonly rawUrl: string;
  readonly status?: number;
  readonly outcome: BrowserNetworkOutcome;
}

export type BrowserArtifactFinishInput =
  | Readonly<{
      outcome: "passed";
      captureScreenshot?: () => Promise<Uint8Array>;
    }>
  | Readonly<{
      outcome: "failed";
      failureKind: BrowserArtifactFailureKind;
      step?: string;
      captureScreenshot?: () => Promise<Uint8Array>;
    }>;

export type BrowserArtifactRetentionReason =
  | "passed"
  | "authenticated-live"
  | "trace-only"
  | "trace-and-screenshot";

export interface BrowserArtifactPublication {
  readonly retainedJson: boolean;
  readonly retainedScreenshot: boolean;
  readonly reason: BrowserArtifactRetentionReason;
  readonly files: readonly string[];
}

type RecorderState = "recording" | "finishing" | "finished";
type ValidatedFinishInput =
  | Readonly<{
      outcome: "passed";
      captureScreenshot?: () => Promise<Uint8Array>;
    }>
  | Readonly<{
      outcome: "failed";
      failureKind: BrowserArtifactFailureKind;
      step?: string;
      captureScreenshot?: () => Promise<Uint8Array>;
    }>;

export class BrowserArtifactRecorder {
  readonly #artifactRoot: string;
  readonly #scenarioId: string;
  readonly #allowedStepIds: ReadonlySet<string>;
  readonly #daemonOrigin: string;
  readonly #profile: BrowserArtifactProfile;
  readonly #canaryRepresentations: readonly Uint8Array[];
  readonly #events: BrowserArtifactEvent[] = [];
  #state: RecorderState = "recording";

  constructor(options: BrowserArtifactRecorderOptions) {
    const copied = copyPlainOwnDataRecord(
      options,
      ["artifactRoot", "scenarioId", "allowedStepIds", "daemonOrigin", "profile", "canaries"],
      ["artifactRoot", "scenarioId", "allowedStepIds", "daemonOrigin", "profile", "canaries"],
      "invalid browser artifact options"
    );
    this.#artifactRoot = validateArtifactRoot(copied.artifactRoot);
    this.#scenarioId = validateBrowserArtifactId(copied.scenarioId);
    this.#allowedStepIds = validateAllowedSteps(copied.allowedStepIds);
    this.#daemonOrigin = validateDaemonOrigin(copied.daemonOrigin);
    this.#profile = validateBrowserArtifactProfile(copied.profile);
    this.#canaryRepresentations = buildSecretCanaryRepresentations(copied.canaries);
    cleanupOwnedEntries(this.#artifactRoot, this.#scenarioId);
  }

  recordStep(stepId: string): void {
    this.#ensureRecording();
    const safeStep = validateBrowserArtifactId(stepId);
    if (!this.#allowedStepIds.has(safeStep)) throw new Error("step is not predeclared");
    this.#append({ sequence: this.#nextSequence(), kind: "step", step: safeStep });
  }

  recordNavigation(rawUrl: string): void {
    this.#ensureRecording();
    const endpoint = classifyBrowserUrl(rawUrl, this.#daemonOrigin);
    this.#append({ sequence: this.#nextSequence(), kind: "navigation", endpoint });
  }

  recordNetwork(observation: BrowserNetworkObservation): void {
    this.#ensureRecording();
    const copied = copyPlainOwnDataRecord(
      observation,
      ["method", "rawUrl", "status", "outcome"],
      ["method", "rawUrl", "outcome"],
      "invalid network observation"
    );
    const method = validateNetworkMethod(copied.method);
    const outcome = validateNetworkOutcome(copied.outcome);
    const endpoint = classifyBrowserUrl(copied.rawUrl, this.#daemonOrigin);
    const status = copied.status === undefined ? undefined : validateHttpStatus(copied.status);
    this.#append(
      status === undefined
        ? { sequence: this.#nextSequence(), kind: "network", method, endpoint, outcome }
        : { sequence: this.#nextSequence(), kind: "network", method, endpoint, outcome, status }
    );
  }

  recordConsole(level: BrowserConsoleLevel | "warning"): void {
    this.#ensureRecording();
    this.#append({
      sequence: this.#nextSequence(),
      kind: "console",
      level: normalizeConsoleLevel(level)
    });
  }

  recordPageError(count: number): void {
    this.#ensureRecording();
    this.#append({
      sequence: this.#nextSequence(),
      kind: "page-error",
      count: validateEventCount(count)
    });
  }

  recordGraphqlError(count: number): void {
    this.#ensureRecording();
    this.#append({
      sequence: this.#nextSequence(),
      kind: "graphql-error",
      count: validateEventCount(count)
    });
  }

  async finish(input: BrowserArtifactFinishInput): Promise<BrowserArtifactPublication> {
    if (this.#state !== "recording") throw new Error("artifact recorder finish already started");
    this.#state = "finishing";

    try {
      const copied = this.#copyAndValidateFinishInput(input);
      if (copied.outcome === "passed") {
        cleanupOwnedEntries(this.#artifactRoot, this.#scenarioId);
        return publication(false, false, "passed", []);
      }
      if (this.#profile === "authenticated-live") {
        cleanupOwnedEntries(this.#artifactRoot, this.#scenarioId);
        return publication(false, false, "authenticated-live", []);
      }

      const failureKind = copied.failureKind;
      const step = copied.step;
      const failure = step === undefined ? { kind: failureKind } : { kind: failureKind, step };
      const trace: BrowserFailureTrace = Object.freeze({
        schemaVersion: 1,
        scenario: this.#scenarioId,
        outcome: "failed",
        failure: Object.freeze(failure),
        events: Object.freeze(this.#events.map((event) => Object.freeze({ ...event })))
      });
      const traceBytes = Uint8Array.from(this.#buildTraceForPublication(trace));
      if (traceBytes.byteLength > MAX_BROWSER_ARTIFACT_JSON_BYTES) {
        throw new Error("browser artifact JSON exceeds byte limit");
      }

      let screenshotBytes: Uint8Array | undefined;
      if (
        this.#profile === "ephemeral-public" &&
        this.#canaryRepresentations.length === 0 &&
        copied.captureScreenshot !== undefined
      ) {
        let captured: unknown;
        try {
          captured = await copied.captureScreenshot();
        } catch {
          throw new Error("screenshot capture failed");
        }
        screenshotBytes = copyScreenshotBytes(captured);
        validatePng(screenshotBytes);
      }

      const files = new Map<string, Uint8Array>([[TRACE_FILENAME, traceBytes]]);
      if (screenshotBytes !== undefined) files.set(SCREENSHOT_FILENAME, screenshotBytes);
      const relativeFiles = [...files.keys()].map((name) => `${this.#scenarioId}/${name}`);
      const stagingName = `.${this.#scenarioId}.staging-${randomUUID()}`;
      this.#rejectCanaries([
        traceBytes,
        ...(screenshotBytes === undefined ? [] : [screenshotBytes]),
        ...[...files.keys(), ...relativeFiles, this.#scenarioId, stagingName].map((name) => UTF8.encode(name))
      ]);

      await this.#publish(stagingName, files);
      return publication(
        true,
        screenshotBytes !== undefined,
        screenshotBytes === undefined ? "trace-only" : "trace-and-screenshot",
        relativeFiles
      );
    } finally {
      this.#state = "finished";
    }
  }

  #buildTraceForPublication(trace: BrowserFailureTrace): Uint8Array {
    return UTF8.encode(JSON.stringify(trace));
  }

  #copyAndValidateFinishInput(input: unknown): ValidatedFinishInput {
    const copied = copyPlainOwnDataRecord(
      input,
      ["outcome", "failureKind", "step", "captureScreenshot"],
      ["outcome"],
      "invalid artifact finish input"
    );
    const callback = copied.captureScreenshot;
    if (callback !== undefined && typeof callback !== "function") {
      throw new Error("invalid screenshot callback");
    }
    if (copied.outcome === "passed") {
      if (copied.failureKind !== undefined || copied.step !== undefined) {
        throw new Error("invalid artifact finish input");
      }
      return Object.freeze({
        outcome: "passed" as const,
        captureScreenshot: callback as (() => Promise<Uint8Array>) | undefined
      });
    }
    if (copied.outcome !== "failed" || copied.failureKind === undefined) {
      throw new Error("invalid artifact finish input");
    }
    const failureKind = validateFailureKind(copied.failureKind);
    const step = copied.step === undefined ? undefined : this.#validateFailureStep(copied.step);
    return Object.freeze({
      outcome: "failed" as const,
      failureKind,
      step,
      captureScreenshot: callback as (() => Promise<Uint8Array>) | undefined
    });
  }

  async #publish(stagingName: string, files: ReadonlyMap<string, Uint8Array>): Promise<void> {
    const stagingPath = join(this.#artifactRoot, stagingName);
    const finalPath = join(this.#artifactRoot, this.#scenarioId);
    let stagingCreated = false;
    let published = false;
    try {
      if (await pathExists(finalPath)) throw new Error("artifact publication conflict");
      await mkdir(stagingPath, { mode: 0o700 });
      stagingCreated = true;
      if (process.platform !== "win32") await chmod(stagingPath, 0o700);
      for (const [name, bytes] of files) {
        await writeExclusivePrivateFile(join(stagingPath, name), bytes);
      }
      await syncDirectory(stagingPath);
      if (await pathExists(finalPath)) throw new Error("artifact publication conflict");
      await rename(stagingPath, finalPath);
      stagingCreated = false;
      published = true;
      await syncDirectory(this.#artifactRoot);
      await this.#revalidatePublished(finalPath, files);
    } catch (error) {
      if (stagingCreated) await removeOwnedDirectory(stagingPath);
      if (published) await removeOwnedDirectory(finalPath);
      throw error;
    }
  }

  async #revalidatePublished(
    finalPath: string,
    expectedFiles: ReadonlyMap<string, Uint8Array>
  ): Promise<void> {
    const finalInfo = await lstat(finalPath);
    if (finalInfo.isSymbolicLink() || !finalInfo.isDirectory()) {
      throw new Error("published browser artifact directory is invalid");
    }
    const names = (await readdir(finalPath)).sort();
    const expectedNames = [...expectedFiles.keys()].sort();
    if (!sameStrings(names, expectedNames)) {
      throw new Error("published browser artifact files are invalid");
    }
    for (const name of names) {
      const path = join(finalPath, name);
      const info = await lstat(path);
      if (info.isSymbolicLink() || !info.isFile()) {
        throw new Error("published browser artifact file is invalid");
      }
      const actual = Uint8Array.from(await readFile(path));
      const expected = expectedFiles.get(name);
      if (expected === undefined || !Buffer.from(actual).equals(Buffer.from(expected))) {
        throw new Error("published browser artifact bytes are invalid");
      }
      this.#rejectCanaries([UTF8.encode(name), actual]);
      if (name === TRACE_FILENAME && actual.byteLength > MAX_BROWSER_ARTIFACT_JSON_BYTES) {
        throw new Error("browser artifact JSON exceeds byte limit");
      }
      if (name === SCREENSHOT_FILENAME) validatePng(actual);
    }
  }

  #rejectCanaries(values: readonly Uint8Array[]): void {
    for (const value of values) {
      const bytes = Buffer.from(value);
      for (const canary of this.#canaryRepresentations) {
        if (bytes.indexOf(Buffer.from(canary)) >= 0) {
          throw new Error("browser artifact contains a secret canary");
        }
      }
    }
  }

  #validateFailureStep(value: unknown): string {
    const step = validateBrowserArtifactId(value);
    if (!this.#allowedStepIds.has(step)) throw new Error("step is not predeclared");
    return step;
  }

  #ensureRecording(): void {
    if (this.#state === "finishing") throw new Error("artifact recorder is finishing");
    if (this.#state === "finished") throw new Error("artifact recorder is finished");
  }

  #nextSequence(): number {
    return this.#events.length + 1;
  }

  #append(event: BrowserArtifactEvent): void {
    if (this.#events.length >= MAX_BROWSER_ARTIFACT_EVENTS) {
      throw new Error("browser artifact event limit exceeded");
    }
    this.#events.push(Object.freeze(event));
  }
}

function validateAllowedSteps(value: unknown): ReadonlySet<string> {
  const copied = copyPlainDataArray(value, "invalid allowed step ids");
  if (copied.length > MAX_BROWSER_ARTIFACT_STEPS) throw new Error("too many allowed step ids");
  const steps = copied.map((step) => validateBrowserArtifactId(step));
  const unique = new Set(steps);
  if (unique.size !== steps.length) throw new Error("duplicate allowed step id");
  return Object.freeze(unique);
}

function validateArtifactRoot(value: unknown): string {
  if (typeof value !== "string") throw new Error("artifact root is unavailable");
  let info;
  try {
    info = lstatSync(value);
  } catch {
    throw new Error("artifact root is unavailable");
  }
  if (info.isSymbolicLink()) throw new Error("artifact root must not be a symlink");
  if (!info.isDirectory()) throw new Error("artifact root is unavailable");

  let canonical: string;
  let canonicalTemp: string;
  try {
    canonical = realpathSync(value);
    canonicalTemp = realpathSync(tmpdir());
  } catch {
    throw new Error("artifact root is unavailable");
  }
  if (canonical === canonicalTemp || !canonical.startsWith(`${canonicalTemp}${sep}`)) {
    throw new Error("artifact root must be in OS temporary storage");
  }
  const canonicalInfo = statSync(canonical);
  if (process.platform !== "win32" && (canonicalInfo.mode & 0o777) !== 0o700) {
    throw new Error("artifact root is not private");
  }
  if (
    typeof process.getuid === "function" &&
    typeof canonicalInfo.uid === "number" &&
    canonicalInfo.uid !== process.getuid()
  ) {
    throw new Error("artifact root has the wrong owner");
  }
  return canonical;
}

function cleanupOwnedEntries(root: string, scenarioId: string): void {
  const ownedNames = readdirSync(root).filter(
    (name) => name === scenarioId || isOwnedStagingName(name, scenarioId)
  );
  for (const name of ownedNames) {
    const path = join(root, name);
    const info = lstatSync(path);
    if (info.isSymbolicLink() || !info.isDirectory()) {
      throw new Error("unsafe owned artifact entry");
    }
  }
  for (const name of ownedNames) rmSync(join(root, name), { recursive: true, force: true });
}

function isOwnedStagingName(name: string, scenarioId: string): boolean {
  const prefix = `.${scenarioId}.staging-`;
  if (!name.startsWith(prefix)) return false;
  return /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u.test(
    name.slice(prefix.length)
  );
}

function validatePng(bytes: Uint8Array): void {
  if (bytes.byteLength > MAX_BROWSER_ARTIFACT_SCREENSHOT_BYTES) {
    throw new Error("screenshot exceeds byte limit");
  }
  if (bytes.byteLength < PNG_SIGNATURE.byteLength) throw new Error("invalid screenshot PNG");
  for (let index = 0; index < PNG_SIGNATURE.byteLength; index += 1) {
    if (bytes[index] !== PNG_SIGNATURE[index]) throw new Error("invalid screenshot PNG");
  }
}

function copyScreenshotBytes(value: unknown): Uint8Array {
  let tag: string;
  let buffer: ArrayBufferLike;
  let byteLength: number;
  let byteOffset: number;
  try {
    tag = TYPED_ARRAY_TAG_GETTER.call(value);
    buffer = TYPED_ARRAY_BUFFER_GETTER.call(value);
    byteLength = TYPED_ARRAY_BYTE_LENGTH_GETTER.call(value);
    byteOffset = TYPED_ARRAY_BYTE_OFFSET_GETTER.call(value);
  } catch {
    throw new Error("invalid screenshot PNG");
  }
  if (
    tag !== "Uint8Array" ||
    !Number.isSafeInteger(byteLength) ||
    byteLength < 0 ||
    !Number.isSafeInteger(byteOffset) ||
    byteOffset < 0
  ) {
    throw new Error("invalid screenshot PNG");
  }
  if (byteLength > MAX_BROWSER_ARTIFACT_SCREENSHOT_BYTES) {
    throw new Error("screenshot exceeds byte limit");
  }
  try {
    const source = new INTRINSIC_UINT8_ARRAY(buffer, byteOffset, byteLength);
    const copied = new INTRINSIC_UINT8_ARRAY(byteLength);
    INTRINSIC_UINT8_ARRAY_SET.call(copied, source);
    return copied;
  } catch {
    throw new Error("invalid screenshot PNG");
  }
}

function requiredIntrinsicGetter<T>(key: PropertyKey): (this: unknown) => T {
  const getter = Object.getOwnPropertyDescriptor(TYPED_ARRAY_PROTOTYPE, key)?.get;
  if (getter === undefined) throw new Error("typed array runtime is unavailable");
  return getter as (this: unknown) => T;
}

async function writeExclusivePrivateFile(path: string, bytes: Uint8Array): Promise<void> {
  const handle = await open(path, "wx", 0o600);
  try {
    await handle.writeFile(bytes);
    if (process.platform !== "win32") await handle.chmod(0o600);
    await handle.sync();
  } finally {
    await handle.close();
  }
}

async function syncDirectory(path: string): Promise<void> {
  let handle;
  try {
    handle = await open(path, "r");
    await handle.sync();
  } catch (error) {
    if (!isUnsupportedDirectorySync(error)) throw error;
  } finally {
    await handle?.close();
  }
}

function isUnsupportedDirectorySync(error: unknown): boolean {
  if (typeof error !== "object" || error === null || !("code" in error)) return false;
  const code = Reflect.get(error, "code");
  return code === "EINVAL" || code === "ENOTSUP" || code === "EBADF" || code === "EISDIR";
}

async function pathExists(path: string): Promise<boolean> {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT") {
      return false;
    }
    throw error;
  }
}

async function removeOwnedDirectory(path: string): Promise<void> {
  try {
    const info = await lstat(path);
    if (!info.isSymbolicLink() && info.isDirectory()) {
      await rm(path, { recursive: true, force: true });
    }
  } catch (error) {
    if (typeof error !== "object" || error === null || Reflect.get(error, "code") !== "ENOENT") {
      throw error;
    }
  }
}

function publication(
  retainedJson: boolean,
  retainedScreenshot: boolean,
  reason: BrowserArtifactRetentionReason,
  files: readonly string[]
): BrowserArtifactPublication {
  return Object.freeze({
    retainedJson,
    retainedScreenshot,
    reason,
    files: Object.freeze([...files])
  });
}

function copyPlainOwnDataRecord(
  value: unknown,
  allowed: readonly string[],
  required: readonly string[],
  message: string
): Readonly<Record<string, unknown>> {
  try {
    if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error(message);
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) throw new Error(message);
    const descriptors = Object.getOwnPropertyDescriptors(value) as unknown as Record<
      PropertyKey,
      PropertyDescriptor
    >;
    const ownKeys = Reflect.ownKeys(descriptors);
    if (ownKeys.some((key) => typeof key !== "string" || !allowed.includes(key))) {
      throw new Error(message);
    }
    const copied: Record<string, unknown> = {};
    for (const key of ownKeys) {
      if (typeof key !== "string") throw new Error(message);
      const descriptor = descriptors[key];
      if (descriptor === undefined || !descriptor.enumerable || !("value" in descriptor)) {
        throw new Error(message);
      }
      copied[key] = descriptor.value;
    }
    if (required.some((key) => !Object.hasOwn(copied, key))) throw new Error(message);
    return Object.freeze(copied);
  } catch {
    throw new Error(message);
  }
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

function sameStrings(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}
