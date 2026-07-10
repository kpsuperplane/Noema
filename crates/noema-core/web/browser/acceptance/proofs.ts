import { type BrowserContext, type Page } from "playwright";
import { spawn } from "node:child_process";
import { readdir } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

import { assertExpectedBootSurface } from "./bootSurface.ts";
import { BrowserAbortOwner } from "./abortOwner.ts";
import { BrowserProcessOwner } from "./browserOwner.ts";
import { boundedCleanup } from "./cleanup.ts";
import { FixtureProcess } from "./fixtureProcess.ts";
import {
  ObservationOwner,
  type ArtifactObservationRecorder
} from "./observations.ts";

const PROOF_TIMEOUT_MS = 10_000;
const CLEANUP_TIMEOUT_MS = 3_000;
const noopRecorder: ArtifactObservationRecorder = {
  recordNetwork() {},
  recordConsole() {},
  recordPageError() {}
};

const abortOwner = new BrowserAbortOwner(process, (code) => {
  process.exitCode = code;
});
abortOwner.install();

try {
  const proof = process.argv[2];
  if (process.argv.length !== 3) throw new Error("invalid proof");
  if (proof === "websocket-egress") await proveWebSocketEgress(abortOwner);
  else if (proof === "status-rejection") await proveStatusRejection(abortOwner);
  else if (proof === "browser-acquisition-abort") await proveBrowserAcquisitionAbort();
  else if (proof === "signal-interruption") await proveSignalInterruption();
  else throw new Error("invalid proof");
  process.stdout.write(
    abortOwner.interruptedExitCode === undefined
      ? "NOEMA browser proof: PASS\n"
      : "NOEMA browser proof: FAIL\n"
  );
} catch {
  process.stdout.write("NOEMA browser proof: FAIL\n");
  if (abortOwner.interruptedExitCode === undefined) process.exitCode = 1;
} finally {
  abortOwner.close();
}

async function proveWebSocketEgress(abort: BrowserAbortOwner): Promise<void> {
  let externalConnections = 0;
  const external = createServer((socket) => {
    externalConnections += 1;
    socket.destroy();
  });
  try {
    await abort.race(
      new Promise<void>((resolveListen, rejectListen) => {
        external.once("error", rejectListen);
        external.listen(0, "127.0.0.1", resolveListen);
      }),
      "proof-server-listen"
    );
    const address = external.address();
    if (address === null || typeof address === "string") throw new Error("proof server failed");
    await withFixturePage(
      abort,
      async ({ page, fixture, observations }) => {
        const sameOriginSocket = fixture.ready.origin.replace("http:", "ws:") + "/graphql/ws";
        const externalSocket = "ws://127.0.0.1:" + address.port + "/blocked";
        const result = await page.evaluate(
          async ({ sameOriginSocket, externalSocket }) => {
            const sameOriginConnected = await new Promise<boolean>((resolve) => {
              const socket = new WebSocket(sameOriginSocket, "graphql-transport-ws");
              const timer = window.setTimeout(() => resolve(false), 3_000);
              socket.addEventListener("open", () => {
                window.clearTimeout(timer);
                socket.close();
                resolve(true);
              });
              socket.addEventListener("error", () => {
                window.clearTimeout(timer);
                resolve(false);
              });
            });
            const externalClosed = await new Promise<boolean>((resolve) => {
              const socket = new WebSocket(externalSocket);
              const timer = window.setTimeout(() => resolve(false), 3_000);
              socket.addEventListener("open", () => {
                window.clearTimeout(timer);
                socket.close();
                resolve(false);
              });
              socket.addEventListener("close", () => {
                window.clearTimeout(timer);
                resolve(true);
              });
              socket.addEventListener("error", () => {
                window.clearTimeout(timer);
                resolve(true);
              });
            });
            return { sameOriginConnected, externalClosed };
          },
          { sameOriginSocket, externalSocket }
        );
        await new Promise((resolve) => setTimeout(resolve, 100));
        if (!result.sameOriginConnected) throw new Error("same-origin-websocket-failed");
        if (!result.externalClosed) throw new Error("external-websocket-not-closed");
        if (externalConnections !== 0) throw new Error("external-websocket-egressed");
        if (observations.snapshot().failure?.kind !== "network") {
          throw new Error("external-websocket-not-observed");
        }
      }
    );
  } finally {
    await closeServer(external);
  }
}

async function proveStatusRejection(abort: BrowserAbortOwner): Promise<void> {
  await withFixturePage(
    abort,
    async ({ page }) => {
      let rejected = false;
      try {
        await assertExpectedBootSurface(page, PROOF_TIMEOUT_MS);
      } catch {
        rejected = true;
      }
      if (
        !rejected ||
        !(await page.locator('main[aria-label="Noema status"]').isVisible()) ||
        (await page.locator('section[aria-label="Noema onboarding"]').isVisible())
      ) {
        throw new Error("status rejection proof failed");
      }
    },
    async (context) => {
      await context.route("**/graphql", async (route) => {
        await route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify({
            errors: [{ message: "fixture boot rejection" }]
          })
        });
      });
    }
  );
}

async function proveBrowserAcquisitionAbort(): Promise<void> {
  const controller = new AbortController();
  const acquisition = BrowserProcessOwner.launch({
    signal: controller.signal,
    acquisitionTimeoutMs: PROOF_TIMEOUT_MS,
    cleanupTimeoutMs: CLEANUP_TIMEOUT_MS
  });
  controller.abort();
  let interrupted = false;
  try {
    await acquisition;
  } catch (error) {
    interrupted =
      error instanceof Error && error.message === "browser acceptance interrupted";
  }
  if (!interrupted) throw new Error("browser acquisition abort proof failed");
}

async function proveSignalInterruption(): Promise<void> {
  const rootsBefore = await fixtureRoots();
  await runInterruptedAcceptance("SIGINT", 130);
  await runInterruptedAcceptance("SIGTERM", 143);
  const rootsAfter = await fixtureRoots();
  for (const root of rootsAfter) {
    if (!rootsBefore.has(root)) throw new Error("signal interruption retained fixture root");
  }
}

async function runInterruptedAcceptance(
  signal: "SIGINT" | "SIGTERM",
  expectedCode: number
): Promise<void> {
  const child = spawn(
    process.execPath,
    [
      "--experimental-strip-types",
      fileURLToPath(new URL("./run.ts", import.meta.url)),
      "boot"
    ],
    { cwd: process.cwd(), stdio: ["ignore", "pipe", "pipe"] }
  );
  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (chunk: Uint8Array) => {
    if (stdout.length <= 4_096) stdout += chunk.toString();
  });
  child.stderr.on("data", (chunk: Uint8Array) => {
    if (stderr.length <= 4_096) stderr += chunk.toString();
  });
  const completion = new Promise<{ code: number | null; signal: NodeJS.Signals | null }>(
    (resolveClose, rejectClose) => {
      child.once("error", rejectClose);
      child.once("close", (code, closeSignal) => resolveClose({ code, signal: closeSignal }));
    }
  );
  const interrupt = setTimeout(() => child.kill(signal), 600);
  let result: { code: number | null; signal: NodeJS.Signals | null };
  try {
    result = await boundedCleanup(completion, 20_000);
  } catch {
    child.kill("SIGKILL");
    await boundedCleanup(completion, CLEANUP_TIMEOUT_MS).catch(() => undefined);
    throw new Error("signal interruption proof failed");
  } finally {
    clearTimeout(interrupt);
  }
  if (
    result.code !== expectedCode ||
    result.signal !== null ||
    stdout !== "NOEMA browser acceptance: FAIL\n" ||
    stderr !== ""
  ) {
    throw new Error("signal interruption proof failed");
  }
}

async function fixtureRoots(): Promise<ReadonlySet<string>> {
  const entries = await readdir(tmpdir(), { withFileTypes: true });
  return new Set(
    entries
      .filter((entry) => entry.isDirectory() && entry.name.startsWith("noema-browser-fixture-"))
      .map((entry) => entry.name)
  );
}

async function withFixturePage(
  abort: BrowserAbortOwner,
  proof: (owned: {
    fixture: FixtureProcess;
    context: BrowserContext;
    page: Page;
    observations: ObservationOwner;
  }) => Promise<void>,
  beforeNavigate?: (context: BrowserContext) => Promise<void>
): Promise<void> {
  const fixture = await FixtureProcess.launch(abort.signal);
  let browserOwner: BrowserProcessOwner | undefined;
  let context: BrowserContext | undefined;
  let page: Page | undefined;
  try {
    const observations = new ObservationOwner(noopRecorder, fixture.ready.origin);
    browserOwner = await BrowserProcessOwner.launch({
      signal: abort.signal,
      acquisitionTimeoutMs: PROOF_TIMEOUT_MS,
      cleanupTimeoutMs: CLEANUP_TIMEOUT_MS
    });
    context = await browserOwner.newContext(
      { serviceWorkers: "block" },
      abort.signal,
      PROOF_TIMEOUT_MS
    );
    await abort.race(
      context.route("**/*", (route) => observations.onHttpRoute(route)),
      "http-route"
    );
    await abort.race(
      context.routeWebSocket("**/*", (route) => observations.onWebSocketRoute(route)),
      "websocket-route"
    );
    if (beforeNavigate !== undefined) {
      await abort.race(beforeNavigate(context), "before-navigation");
    }
    page = await browserOwner.newPage(abort.signal, PROOF_TIMEOUT_MS);
    await abort.race(
      page.goto(fixture.ready.startUrl, {
        waitUntil: "domcontentloaded",
        timeout: PROOF_TIMEOUT_MS
      }),
      "navigation"
    );
    await abort.race(
      proof({ fixture, context, page, observations }),
      "proof"
    );
    observations.freeze();
  } finally {
    let cleanupError: unknown;
    if (browserOwner !== undefined) {
      try {
        await browserOwner.close();
      } catch (error) {
        cleanupError = error;
      }
    }
    try {
      await fixture.close();
    } catch (error) {
      cleanupError ??= error;
    }
    assertProofCleanupSucceeded(cleanupError);
  }
}

async function closeServer(server: ReturnType<typeof createServer>): Promise<void> {
  if (!server.listening) return;
  try {
    await boundedCleanup(
      new Promise<void>((resolveClose, rejectClose) => {
        server.close((error) => {
          if (error === undefined) resolveClose();
          else rejectClose(error);
        });
      }),
      CLEANUP_TIMEOUT_MS
    );
  } catch {
    throw new Error("browser proof cleanup failed");
  }
}

function assertProofCleanupSucceeded(error: unknown): void {
  if (error !== undefined) throw new Error("browser proof cleanup failed");
}
