import { afterEach, describe, expect, test } from "bun:test";
import {
  chmod,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rm,
  symlink,
  writeFile
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runInNewContext } from "node:vm";

import {
  MAX_BROWSER_ARTIFACT_EVENTS,
  MAX_BROWSER_ARTIFACT_JSON_BYTES,
  MAX_BROWSER_ARTIFACT_SCREENSHOT_BYTES,
  buildSecretCanaryRepresentations,
  classifyBrowserUrl
} from "./model";
import { BrowserArtifactRecorder } from "./writer";

const DAEMON_ORIGIN = "http://127.0.0.1:3737";
const PNG = new Uint8Array([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00
]);
const roots: string[] = [];

async function privateTempRoot(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "noema-browser-artifacts-"));
  roots.push(root);
  await chmod(root, 0o700);
  return root;
}

function recorder(
  artifactRoot: string,
  overrides: Partial<ConstructorParameters<typeof BrowserArtifactRecorder>[0]> = {}
): BrowserArtifactRecorder {
  return new BrowserArtifactRecorder({
    artifactRoot,
    scenarioId: "empty-conversation",
    allowedStepIds: ["open-home", "empty-state-visible"],
    daemonOrigin: DAEMON_ORIGIN,
    profile: "ephemeral-public",
    canaries: [],
    ...overrides
  });
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

describe("classifyBrowserUrl", () => {
  const cases = [
    ["/", "app-root"],
    ["/memory", "memory"],
    ["/settings", "settings"],
    ["/settings/", "settings-index"],
    ["/settings/agents", "settings-agents"],
    ["/settings/memory", "settings-memory"],
    ["/settings/tools/web", "settings-tools-web"],
    ["/settings/tools/mcps", "settings-tools-mcps"],
    ["/settings/system/providers", "settings-system-providers"],
    ["/settings/safety/usage", "settings-safety-usage"],
    ["/settings/safety/identities", "settings-safety-identities"],
    ["/settings/safety/approvals", "settings-safety-approvals"],
    ["/graphql", "graphql"],
    ["/graphql/schema.graphql", "graphql-schema"],
    ["/graphql/ws", "graphql-websocket"],
    ["/mcp/oauth/callback", "mcp-oauth-callback"],
    ["/assets/app-AbC_123.js", "asset"],
    ["/artifacts/version-123/download", "artifact-download"],
    ["/artifacts/versions/version-123/download", "artifact-version-download"]
  ] as const;

  for (const [path, expected] of cases) {
    test(`classifies ${path} without retaining its dynamic value`, () => {
      expect(classifyBrowserUrl(`${DAEMON_ORIGIN}${path}`, DAEMON_ORIGIN)).toBe(expected);
    });
  }

  test("accepts the equivalent websocket origin only for the websocket endpoint", () => {
    expect(classifyBrowserUrl("ws://127.0.0.1:3737/graphql/ws", DAEMON_ORIGIN)).toBe(
      "graphql-websocket"
    );
    expect(classifyBrowserUrl("ws://127.0.0.1:3737/graphql", DAEMON_ORIGIN)).toBe("other");
  });

  test("classifies external origins before matching a familiar path", () => {
    expect(classifyBrowserUrl("https://example.com/graphql", DAEMON_ORIGIN)).toBe("external");
    expect(classifyBrowserUrl("https://example.com/settings/memory", DAEMON_ORIGIN)).toBe(
      "external"
    );
  });

  test("maps unsupported URL schemes to other", () => {
    expect(classifyBrowserUrl("file:///graphql", DAEMON_ORIGIN)).toBe("other");
    expect(classifyBrowserUrl("ftp://127.0.0.1/graphql", DAEMON_ORIGIN)).toBe("other");
  });

  test("maps relative, invalid, credentialed, ambiguous, encoded, queried, and fragmented URLs to other", () => {
    const rejected = [
      "/memory",
      "not a url",
      "http://user:password@127.0.0.1:3737/memory",
      `${DAEMON_ORIGIN}/memory?token=secret`,
      `${DAEMON_ORIGIN}/memory#secret`,
      `${DAEMON_ORIGIN}//memory`,
      `${DAEMON_ORIGIN}/settings/../memory`,
      `${DAEMON_ORIGIN}/settings/%2e%2e/memory`,
      `${DAEMON_ORIGIN}/assets/a%2fb.js`,
      `${DAEMON_ORIGIN}/assets/a%5cb.js`,
      `${DAEMON_ORIGIN}/assets/two/segments.js`,
      `${DAEMON_ORIGIN}/artifacts/a/b/download`
    ];
    expect(rejected.map((url) => classifyBrowserUrl(url, DAEMON_ORIGIN))).toEqual(
      rejected.map(() => "other")
    );
  });

  test("checks ambiguity before collapsing an external origin", () => {
    const ambiguousExternal = [
      "https://example.com//graphql",
      "https://example.com/a/../graphql",
      "https://example.com/%2e%2e/graphql",
      "https://example.com/assets/a%2fb.js"
    ];
    expect(ambiguousExternal.map((url) => classifyBrowserUrl(url, DAEMON_ORIGIN))).toEqual(
      ambiguousExternal.map(() => "other")
    );
  });

  test("maps oversized URL input to other without throwing or echoing it", () => {
    expect(classifyBrowserUrl(`${DAEMON_ORIGIN}/${"secret".repeat(1400)}`, DAEMON_ORIGIN)).toBe(
      "other"
    );
  });

  test("rejects non-loopback and state-bearing daemon origins with stable errors", () => {
    const invalid = [
      "https://example.com",
      "ftp://127.0.0.1:3737",
      "http://user:secret@127.0.0.1:3737",
      "http://127.0.0.1:3737/memory",
      "http://127.0.0.1:3737?secret=yes",
      "http://127.0.0.1:3737#secret"
    ];
    for (const origin of invalid) {
      expect(() => classifyBrowserUrl(`${DAEMON_ORIGIN}/`, origin)).toThrow(
        "invalid daemon origin"
      );
    }
  });

  test("rejects daemon-origin syntax that URL parsing would normalize away", () => {
    const invalid = [
      " http://127.0.0.1:3737",
      "http://127.0.0.1:3737 ",
      "http://127.0.0.1:3737/%2e",
      "http://127.0.0.1:3737/%2f",
      "http://127.0.0.1:3737/a/..",
      "http://127.0.0.1:3737/a/../"
    ];
    for (const origin of invalid) {
      expect(() => classifyBrowserUrl(`${DAEMON_ORIGIN}/`, origin)).toThrow(
        "invalid daemon origin"
      );
    }
  });
});

describe("BrowserArtifactRecorder validation and schema", () => {
  test("publishes the exact trace schema and no raw observation fields", async () => {
    const root = await privateTempRoot();
    const artifacts = recorder(root);
    artifacts.recordStep("open-home");
    artifacts.recordNavigation(`${DAEMON_ORIGIN}/artifacts/sensitive-id/download`);
    artifacts.recordNetwork({
      method: "POST",
      rawUrl: `${DAEMON_ORIGIN}/graphql`,
      status: 500,
      outcome: "response"
    });
    artifacts.recordConsole("warning");
    artifacts.recordPageError(2);
    artifacts.recordGraphqlError(1);

    const result = await artifacts.finish({
      outcome: "failed",
      failureKind: "assertion",
      step: "empty-state-visible"
    });

    expect(result).toEqual({
      retainedJson: true,
      retainedScreenshot: false,
      reason: "trace-only",
      files: ["empty-conversation/failure-trace.json"]
    });
    const trace = JSON.parse(
      await readFile(join(root, "empty-conversation", "failure-trace.json"), "utf8")
    );
    expect(Object.keys(trace)).toEqual([
      "schemaVersion",
      "scenario",
      "outcome",
      "failure",
      "events"
    ]);
    expect(trace).toEqual({
      schemaVersion: 1,
      scenario: "empty-conversation",
      outcome: "failed",
      failure: { kind: "assertion", step: "empty-state-visible" },
      events: [
        { sequence: 1, kind: "step", step: "open-home" },
        { sequence: 2, kind: "navigation", endpoint: "artifact-download" },
        {
          sequence: 3,
          kind: "network",
          method: "POST",
          endpoint: "graphql",
          outcome: "response",
          status: 500
        },
        { sequence: 4, kind: "console", level: "warn" },
        { sequence: 5, kind: "page-error", count: 2 },
        { sequence: 6, kind: "graphql-error", count: 1 }
      ]
    });
    expect(JSON.stringify(trace)).not.toContain("sensitive-id");
  });

  test("rejects unsafe identifiers, duplicate steps, and unknown enum values at runtime", async () => {
    const root = await privateTempRoot();
    const unsafeValues = ["Uppercase", "two words", "../escape", "", "a".repeat(65)];
    for (const scenarioId of unsafeValues) {
      expect(() => recorder(root, { scenarioId })).toThrow("invalid browser artifact id");
    }
    expect(() => recorder(root, { allowedStepIds: ["same", "same"] })).toThrow(
      "duplicate allowed step id"
    );
    expect(() => recorder(root, { profile: "secret" as "ephemeral-public" })).toThrow(
      "invalid browser artifact profile"
    );

    const artifacts = recorder(root);
    expect(() => artifacts.recordStep("not-allowed")).toThrow("step is not predeclared");
    expect(() => artifacts.recordConsole("fatal")).toThrow("invalid console level");
    expect(() =>
      artifacts.recordNetwork({
        method: "PATCH",
        rawUrl: `${DAEMON_ORIGIN}/graphql`,
        outcome: "response"
      })
    ).toThrow("invalid network method");
    expect(() => artifacts.recordPageError(-1)).toThrow("invalid event count");
    expect(() => artifacts.recordGraphqlError(1_000_001)).toThrow("invalid event count");
    expect(() =>
      artifacts.recordNetwork({
        method: "POST",
        rawUrl: `${DAEMON_ORIGIN}/graphql`,
        outcome: "response",
        headers: { authorization: "secret" }
      } as never)
    ).toThrow("invalid network observation");
  });

  test("copies constructor inputs and emits stable errors without rejected data", async () => {
    const root = await privateTempRoot();
    const steps = ["open-home"];
    const canaries = ["secret-value"];
    const artifacts = recorder(root, { allowedStepIds: steps, canaries });
    steps[0] = "mutated-step";
    canaries[0] = "mutated-secret";
    expect(() => artifacts.recordStep("mutated-step")).toThrow("step is not predeclared");

    let message = "";
    try {
      artifacts.recordStep("rejected-secret-value");
    } catch (error) {
      message = String(error);
    }
    expect(message).toBe("Error: step is not predeclared");
    expect(message).not.toContain("secret-value");
  });

  test("accepts only exact plain own constructor data without invoking accessors", async () => {
    const root = await privateTempRoot();
    const base = {
      artifactRoot: root,
      scenarioId: "empty-conversation",
      allowedStepIds: ["open-home"],
      daemonOrigin: DAEMON_ORIGIN,
      profile: "ephemeral-public",
      canaries: []
    };

    let getterReads = 0;
    const accessor = { ...base };
    Object.defineProperty(accessor, "profile", {
      enumerable: true,
      get: () => {
        getterReads += 1;
        return "ephemeral-public";
      }
    });
    expect(() => new BrowserArtifactRecorder(accessor as never)).toThrow(
      "invalid browser artifact options"
    );
    expect(getterReads).toBe(0);

    const inherited = Object.create(base) as typeof base;
    expect(() => new BrowserArtifactRecorder(inherited)).toThrow(
      "invalid browser artifact options"
    );

    const symbolExtra = { ...base, [Symbol("secret")]: "hidden" };
    expect(() => new BrowserArtifactRecorder(symbolExtra)).toThrow(
      "invalid browser artifact options"
    );

    const nonEnumerableExtra = { ...base };
    Object.defineProperty(nonEnumerableExtra, "hidden", { value: "secret", enumerable: false });
    expect(() => new BrowserArtifactRecorder(nonEnumerableExtra)).toThrow(
      "invalid browser artifact options"
    );

    const hostileProxy = new Proxy(base, {
      ownKeys: () => {
        throw new Error("/private/secret");
      }
    });
    expect(() => new BrowserArtifactRecorder(hostileProxy)).toThrow(
      "invalid browser artifact options"
    );
    expect(await readdir(root)).toEqual([]);
  });

  test("copies exact network observations once without invoking accessors", async () => {
    const root = await privateTempRoot();
    const artifacts = recorder(root);
    let reads = 0;
    const observation = {
      method: "POST",
      outcome: "response",
      status: 200
    };
    Object.defineProperty(observation, "rawUrl", {
      enumerable: true,
      get: () => {
        reads += 1;
        return `${DAEMON_ORIGIN}/graphql`;
      }
    });
    expect(() => artifacts.recordNetwork(observation as never)).toThrow(
      "invalid network observation"
    );
    expect(reads).toBe(0);
  });

  test("copies constructor collections from plain data arrays without reading accessors", async () => {
    const root = await privateTempRoot();
    const steps = ["open-home"];
    let reads = 0;
    Object.defineProperty(steps, "0", {
      enumerable: true,
      get: () => {
        reads += 1;
        return "open-home";
      }
    });
    expect(() => recorder(root, { allowedStepIds: steps })).toThrow("invalid allowed step ids");
    expect(reads).toBe(0);
  });
});

describe("BrowserArtifactRecorder lifecycle", () => {
  test("success removes stale owned output and never calls screenshot capture", async () => {
    const root = await privateTempRoot();
    await mkdir(join(root, "empty-conversation"));
    await writeFile(join(root, "empty-conversation", "failure-trace.json"), "stale");
    let called = false;
    const result = await recorder(root).finish({
      outcome: "passed",
      captureScreenshot: async () => {
        called = true;
        return PNG;
      }
    });
    expect(called).toBe(false);
    expect(result).toEqual({
      retainedJson: false,
      retainedScreenshot: false,
      reason: "passed",
      files: []
    });
    expect(await readdir(root)).toEqual([]);
  });

  test("authenticated-live failures retain nothing and never call screenshot capture", async () => {
    const root = await privateTempRoot();
    let called = false;
    const result = await recorder(root, { profile: "authenticated-live" }).finish({
      outcome: "failed",
      failureKind: "assertion",
      captureScreenshot: async () => {
        called = true;
        return PNG;
      }
    });
    expect(called).toBe(false);
    expect(result.reason).toBe("authenticated-live");
    expect(result.files).toEqual([]);
    expect(await readdir(root)).toEqual([]);
  });

  test("validates authenticated-live failure details before applying no-retention policy", async () => {
    const root = await privateTempRoot();
    const artifacts = recorder(root, { profile: "authenticated-live" });
    await expect(
      artifacts.finish({
        outcome: "failed",
        failureKind: "assertion",
        step: "not-predeclared"
      })
    ).rejects.toThrow("step is not predeclared");
    expect(await readdir(root)).toEqual([]);
  });

  test("sensitive failure retains JSON only and never calls screenshot capture", async () => {
    const root = await privateTempRoot();
    let called = false;
    const result = await recorder(root, { profile: "ephemeral-sensitive" }).finish({
      outcome: "failed",
      failureKind: "network",
      captureScreenshot: async () => {
        called = true;
        return PNG;
      }
    });
    expect(called).toBe(false);
    expect(result.reason).toBe("trace-only");
    expect(result.files).toEqual(["empty-conversation/failure-trace.json"]);
  });

  test("public failure publishes a bounded valid PNG from an eligible callback", async () => {
    const root = await privateTempRoot();
    const result = await recorder(root).finish({
      outcome: "failed",
      failureKind: "assertion",
      captureScreenshot: async () => PNG
    });
    expect(result).toEqual({
      retainedJson: true,
      retainedScreenshot: true,
      reason: "trace-and-screenshot",
      files: [
        "empty-conversation/failure-trace.json",
        "empty-conversation/screenshot.png"
      ]
    });
    expect(new Uint8Array(await readFile(join(root, "empty-conversation", "screenshot.png")))).toEqual(
      PNG
    );
  });

  test("canary presence disables screenshot callback even for the public profile", async () => {
    const root = await privateTempRoot();
    let called = false;
    const result = await recorder(root, { canaries: ["a-real-secret"] }).finish({
      outcome: "failed",
      failureKind: "assertion",
      captureScreenshot: async () => {
        called = true;
        return PNG;
      }
    });
    expect(called).toBe(false);
    expect(result.reason).toBe("trace-only");
  });

  test("finish is single-use, non-reentrant, and closes recording immediately", async () => {
    const root = await privateTempRoot();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const artifacts = recorder(root);
    const finishing = artifacts.finish({
      outcome: "failed",
      failureKind: "assertion",
      captureScreenshot: async () => {
        await gate;
        return PNG;
      }
    });
    expect(() => artifacts.recordStep("open-home")).toThrow("artifact recorder is finishing");
    await expect(
      artifacts.finish({ outcome: "failed", failureKind: "assertion" })
    ).rejects.toThrow("artifact recorder finish already started");
    release();
    await finishing;
    expect(() => artifacts.recordStep("open-home")).toThrow("artifact recorder is finished");
  });

  test("rejects accessor finish inputs without reading them", async () => {
    const root = await privateTempRoot();
    const artifacts = recorder(root);
    let reads = 0;
    const input = { outcome: "failed", failureKind: "assertion" };
    Object.defineProperty(input, "step", {
      enumerable: true,
      get: () => {
        reads += 1;
        return "open-home";
      }
    });
    await expect(artifacts.finish(input as never)).rejects.toThrow(
      "invalid artifact finish input"
    );
    expect(reads).toBe(0);
  });

  test("wraps screenshot callback failures without retaining sensitive error details", async () => {
    const root = await privateTempRoot();
    const secret = "/private/token-value";
    let message = "";
    try {
      await recorder(root).finish({
        outcome: "failed",
        failureKind: "assertion",
        captureScreenshot: async () => {
          throw new Error(secret);
        }
      });
    } catch (error) {
      message = String(error);
    }
    expect(message).toBe("Error: screenshot capture failed");
    expect(message).not.toContain(secret);
    expect(await readdir(root)).toEqual([]);
  });

  test("copies screenshot bytes from intrinsic slots without reading subclass length accessors", async () => {
    const root = await privateTempRoot();
    class HostileBytes extends Uint8Array {
      override get byteLength(): number {
        throw new Error("/private/result-token");
      }
    }
    const hostile = new HostileBytes(PNG);
    const result = await recorder(root).finish({
      outcome: "failed",
      failureKind: "assertion",
      captureScreenshot: async () => hostile
    });
    expect(result.retainedScreenshot).toBe(true);
    expect(new Uint8Array(await readFile(join(root, "empty-conversation", "screenshot.png")))).toEqual(
      PNG
    );
  });

  test("does not invoke a screenshot subclass iterator or species while copying exact bytes", async () => {
    const root = await privateTempRoot();
    let iteratorRead = false;
    let speciesRead = false;
    class AdversarialBytes extends Uint8Array {
      static override get [Symbol.species](): Uint8ArrayConstructor {
        speciesRead = true;
        throw new Error("species must not run");
      }

      override *[Symbol.iterator](): ArrayIterator<number> {
        iteratorRead = true;
        for (let index = 0; index < PNG.byteLength + 5; index += 1) {
          yield 0;
        }
      }
    }
    const adversarial = new AdversarialBytes(PNG);
    const result = await recorder(root).finish({
      outcome: "failed",
      failureKind: "assertion",
      captureScreenshot: async () => adversarial
    });
    expect(iteratorRead).toBe(false);
    expect(speciesRead).toBe(false);
    expect(result.retainedScreenshot).toBe(true);
    expect(new Uint8Array(await readFile(join(root, "empty-conversation", "screenshot.png")))).toEqual(
      PNG
    );
  });

  test("accepts a real cross-realm Uint8Array by intrinsic brand and copies exact bytes", async () => {
    const root = await privateTempRoot();
    const crossRealm = runInNewContext(
      "new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10, 0])"
    ) as Uint8Array;
    expect(crossRealm instanceof Uint8Array).toBe(false);
    const result = await recorder(root).finish({
      outcome: "failed",
      failureKind: "assertion",
      captureScreenshot: async () => crossRealm
    });
    expect(result.retainedScreenshot).toBe(true);
    expect(new Uint8Array(await readFile(join(root, "empty-conversation", "screenshot.png")))).toEqual(
      PNG
    );
  });

  test("enforces event, status, count, JSON, screenshot, step, and canary bounds", async () => {
    const root = await privateTempRoot();
    expect(() =>
      recorder(root, {
        allowedStepIds: Array.from({ length: 129 }, (_, index) => `step-${index}`)
      })
    ).toThrow("too many allowed step ids");
    expect(() => recorder(root, { canaries: Array.from({ length: 33 }, () => "secret") })).toThrow(
      "too many secret canaries"
    );
    expect(() => recorder(root, { canaries: [""] })).toThrow(
      "invalid secret canary"
    );
    expect(() => recorder(root, { canaries: ["x".repeat(4097)] })).toThrow(
      "invalid secret canary"
    );

    const artifacts = recorder(root);
    expect(() =>
      artifacts.recordNetwork({
        method: "GET",
        rawUrl: `${DAEMON_ORIGIN}/`,
        status: 99,
        outcome: "response"
      })
    ).toThrow("invalid HTTP status");
    for (let index = 0; index < MAX_BROWSER_ARTIFACT_EVENTS; index += 1) {
      artifacts.recordPageError(0);
    }
    expect(() => artifacts.recordPageError(0)).toThrow("browser artifact event limit exceeded");

    const hugeTrace = recorder(await privateTempRoot(), {
      allowedStepIds: Array.from({ length: 128 }, (_, index) => `step-${index}`)
    });
    for (let index = 0; index < MAX_BROWSER_ARTIFACT_EVENTS; index += 1) {
      hugeTrace.recordNavigation(`${DAEMON_ORIGIN}/`);
    }
    expect(MAX_BROWSER_ARTIFACT_JSON_BYTES).toBe(256 * 1024);
    await expect(
      hugeTrace.finish({ outcome: "failed", failureKind: "assertion" })
    ).resolves.toMatchObject({ retainedJson: true });

    const badPng = recorder(await privateTempRoot());
    await expect(
      badPng.finish({
        outcome: "failed",
        failureKind: "assertion",
        captureScreenshot: async () => new Uint8Array([1, 2, 3])
      })
    ).rejects.toThrow("invalid screenshot PNG");
    const oversizedPng = recorder(await privateTempRoot());
    await expect(
      oversizedPng.finish({
        outcome: "failed",
        failureKind: "assertion",
        captureScreenshot: async () => new Uint8Array(MAX_BROWSER_ARTIFACT_SCREENSHOT_BYTES + 1)
      })
    ).rejects.toThrow("screenshot exceeds byte limit");
    const nonBytesPng = recorder(await privateTempRoot());
    await expect(
      nonBytesPng.finish({
        outcome: "failed",
        failureKind: "assertion",
        captureScreenshot: async () => null as never
      })
    ).rejects.toThrow("invalid screenshot PNG");
  });
});

describe("BrowserArtifactRecorder filesystem and canary safety", () => {
  test("production source cannot import browser harness code", async () => {
    const webRoot = join(import.meta.dir, "..", "..");
    const sourceFiles = await collectTypeScriptFiles(join(webRoot, "src"));
    const forbiddenImports: string[] = [];
    for (const path of sourceFiles) {
      const source = await readFile(path, "utf8");
      if (/(?:from\s*|import\s*\()\s*["'][^"']*browser\//u.test(source)) {
        forbiddenImports.push(path);
      }
    }
    expect(forbiddenImports).toEqual([]);
  });

  test("requires an existing canonical private OS-temp directory", async () => {
    const root = await privateTempRoot();
    await chmod(root, 0o755);
    expect(() => recorder(root)).toThrow("artifact root is not private");
    await chmod(root, 0o700);

    const link = `${root}-link`;
    roots.push(link);
    await symlink(root, link);
    expect(() => recorder(link)).toThrow("artifact root must not be a symlink");
    expect(() => recorder(join(root, "missing"))).toThrow("artifact root is unavailable");
    expect(() => recorder(process.cwd())).toThrow("artifact root must be in OS temporary storage");
  });

  test("publishes directories and files with private permissions where supported", async () => {
    if (process.platform === "win32") return;
    const root = await privateTempRoot();
    await recorder(root).finish({ outcome: "failed", failureKind: "assertion" });
    const directoryMode = (await lstat(join(root, "empty-conversation"))).mode & 0o777;
    const fileMode = (await lstat(join(root, "empty-conversation", "failure-trace.json"))).mode &
      0o777;
    expect(directoryMode).toBe(0o700);
    expect(fileMode).toBe(0o600);
  });

  test("rejects symlink final and owned staging entries without traversing targets", async () => {
    const root = await privateTempRoot();
    const target = await privateTempRoot();
    await writeFile(join(target, "sentinel"), "preserve");
    await symlink(target, join(root, "empty-conversation"));
    expect(() => recorder(root)).toThrow("unsafe owned artifact entry");
    expect(await readFile(join(target, "sentinel"), "utf8")).toBe("preserve");

    await rm(join(root, "empty-conversation"));
    await symlink(
      target,
      join(root, ".empty-conversation.staging-00000000-0000-4000-8000-000000000001")
    );
    expect(() => recorder(root)).toThrow("unsafe owned artifact entry");
    expect(await readFile(join(target, "sentinel"), "utf8")).toBe("preserve");
  });

  test("cleans stale owned directories but leaves unrelated entries untouched", async () => {
    const root = await privateTempRoot();
    await mkdir(join(root, "empty-conversation"));
    await writeFile(join(root, "empty-conversation", "failure-trace.json"), "stale");
    const staleStaging = join(
      root,
      ".empty-conversation.staging-00000000-0000-4000-8000-000000000001"
    );
    await mkdir(staleStaging);
    await writeFile(join(staleStaging, "owned"), "stale");
    await writeFile(join(root, "unrelated"), "preserve");
    const artifacts = recorder(root);
    expect((await readdir(root)).sort()).toEqual(["unrelated"]);
    await artifacts.finish({ outcome: "failed", failureKind: "assertion" });
    expect(await readFile(join(root, "unrelated"), "utf8")).toBe("preserve");
  });

  test("fails closed when absent-only final publication conflicts", async () => {
    const root = await privateTempRoot();
    const artifacts = recorder(root);
    await mkdir(join(root, "empty-conversation"));
    await writeFile(join(root, "empty-conversation", "attacker"), "preserve");
    await expect(
      artifacts.finish({ outcome: "failed", failureKind: "assertion" })
    ).rejects.toThrow("artifact publication conflict");
    expect(await readdir(root)).toEqual(["empty-conversation"]);
    expect(await readFile(join(root, "empty-conversation", "attacker"), "utf8")).toBe("preserve");
  });

  test("builds complete byte-wise canary representations without touching serialization", () => {
    const canary = "A é😀";
    const forms = buildSecretCanaryRepresentations([canary]).map((bytes) =>
      new TextDecoder().decode(bytes)
    );
    expect(forms).toContain("%41%20%C3%A9%F0%9F%98%80");
    expect(forms).toContain("%41%20%c3%a9%f0%9f%98%80");
    expect(forms).toContain("%41+%C3%A9%F0%9F%98%80");
    expect(forms).toContain("%41+%c3%a9%f0%9f%98%80");
    expect(forms).toContain("\\u0041\\u0020\\u00e9\\ud83d\\ude00");
    expect(forms).toContain("\\u0041\\u0020\\u00E9\\uD83D\\uDE00");
  });

  test("fixed-schema canary detection fails before any write and never echoes the canary", async () => {
    const root = await privateTempRoot();
    const canary = "failed";
    const artifacts = recorder(root, { canaries: [canary] });
    let errorText = "";
    try {
      await artifacts.finish({ outcome: "failed", failureKind: "assertion" });
    } catch (error) {
      errorText = String(error);
    }
    expect(errorText).toContain("browser artifact contains a secret canary");
    expect(errorText).not.toContain(canary);
    expect(await readdir(root)).toEqual([]);
  });

  test("rejects malformed Unicode canaries with stable zero-write failure", async () => {
    for (const canary of ["high-\ud800", "low-\udc00", "reversed-\udc00\ud800"]) {
      const root = await privateTempRoot();
      expect(() => recorder(root, { canaries: [canary] })).toThrow("invalid secret canary");
      expect(await readdir(root)).toEqual([]);
    }
  });

  test("canonical root remains the publication boundary", async () => {
    const root = await privateTempRoot();
    const canonical = await realpath(root);
    await recorder(root).finish({ outcome: "failed", failureKind: "assertion" });
    expect(await realpath(root)).toBe(canonical);
    expect((await readdir(root)).sort()).toEqual(["empty-conversation"]);
  });
});

async function collectTypeScriptFiles(directory: string): Promise<string[]> {
  const files: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectTypeScriptFiles(path)));
    } else if (entry.isFile() && (entry.name.endsWith(".ts") || entry.name.endsWith(".tsx"))) {
      files.push(path);
    }
  }
  return files;
}
