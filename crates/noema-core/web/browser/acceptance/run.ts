import { lstat, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

import { type BrowserContext, type Page } from "playwright";

import {
  BrowserArtifactRecorder,
  type BrowserArtifactPublication
} from "../artifacts/writer.ts";
import type { BrowserArtifactFailureKind } from "../artifacts/model.ts";
import { FixtureProcess, REPOSITORY_ROOT } from "./fixtureProcess.ts";
import { ObservationOwner, isAllowedHttpUrl } from "./observations.ts";
import { assertExpectedBootSurface } from "./bootSurface.ts";
import {
  BrowserAbortOwner,
  interruptedError,
  raceWithAbortSignal
} from "./abortOwner.ts";
import { BrowserProcessOwner } from "./browserOwner.ts";
import { boundedCleanup } from "./cleanup.ts";

const ASSET_ROOT = resolve(REPOSITORY_ROOT, "crates/noema-core/target/web-assets");
const RUN_TIMEOUT_MS = 45_000;
const NAVIGATION_TIMEOUT_MS = 10_000;
const CLEANUP_TIMEOUT_MS = 3_000;
const ALLOWED_STEPS = ["boot", "built-assets", "forced-safe-failure"] as const;

type Failure = Readonly<{ kind: BrowserArtifactFailureKind; step: string }>;

const cliArgs = process.argv.slice(2);
const abortOwner = new BrowserAbortOwner(process, (code) => {
  process.exitCode = code;
});
abortOwner.install();

try {
  const publication = await runBoot(abortOwner);
  if (abortOwner.interruptedExitCode !== undefined) {
    process.stdout.write("NOEMA browser acceptance: FAIL\n");
  } else if (publication === undefined) {
    process.stdout.write("NOEMA browser acceptance: PASS\n");
  } else {
    printFailure(publication);
    if (abortOwner.interruptedExitCode === undefined) process.exitCode = 1;
  }
} catch {
  process.stdout.write("NOEMA browser acceptance: FAIL\n");
  if (abortOwner.interruptedExitCode === undefined) process.exitCode = 1;
} finally {
  abortOwner.close();
}

async function runBoot(
  abort: BrowserAbortOwner
): Promise<BrowserArtifactPublication | undefined> {
  const scenario = parseCli(cliArgs);
  if (scenario !== "boot") throw new Error("browser scenario is unsupported");
  await requireBuiltAssets();

  let fixture: FixtureProcess | undefined;
  let browserOwner: BrowserProcessOwner | undefined;
  let context: BrowserContext | undefined;
  let page: Page | undefined;
  let recorder: BrowserArtifactRecorder | undefined;
  let observations: ObservationOwner | undefined;
  let detachPageObservers: (() => void) | undefined;
  let failure: Failure | undefined;
  let publication: BrowserArtifactPublication | undefined;
  let retainArtifacts = false;
  let deadlineAt: number | undefined;
  let recorderFinished = false;

  try {
    fixture = await FixtureProcess.launch(abort.signal);
    deadlineAt = Date.now() + RUN_TIMEOUT_MS;
    recorder = new BrowserArtifactRecorder({
      artifactRoot: fixture.home.artifactRoot,
      scenarioId: "boot",
      allowedStepIds: ALLOWED_STEPS,
      daemonOrigin: fixture.ready.origin,
      profile: "ephemeral-public",
      canaries: []
    });
    recorder.recordStep("boot");
    recorder.recordNavigation(fixture.ready.startUrl);
    observations = new ObservationOwner(recorder, fixture.ready.origin);

    browserOwner = await BrowserProcessOwner.launch({
      signal: abort.signal,
      acquisitionTimeoutMs: NAVIGATION_TIMEOUT_MS,
      cleanupTimeoutMs: CLEANUP_TIMEOUT_MS
    });
    context = await browserOwner.newContext(
      {
        acceptDownloads: false,
        serviceWorkers: "block"
      },
      abort.signal,
      NAVIGATION_TIMEOUT_MS
    );
    await withinRunDeadline(
      context.route("**/*", (route) => observations!.onHttpRoute(route)),
      deadlineAt,
      abort.signal
    );
    await withinRunDeadline(
      context.routeWebSocket("**/*", (route) => observations!.onWebSocketRoute(route)),
      deadlineAt,
      abort.signal
    );
    page = await browserOwner.newPage(abort.signal, NAVIGATION_TIMEOUT_MS);
    page.setDefaultNavigationTimeout(NAVIGATION_TIMEOUT_MS);
    page.setDefaultTimeout(NAVIGATION_TIMEOUT_MS);

    const pageErrorHandler = () => observations!.onPageError();
    const consoleHandler = (message: { type(): string }) => {
      if (message.type() === "error") observations!.onConsoleError();
    };
    const requestFailedHandler = (request: {
      method(): string;
      url(): string;
    }) => observations!.onRequestFailed(request.method(), request.url(), request);
    const responseHandler = (response: {
      status(): number;
      url(): string;
      request(): { method(): string };
    }) =>
      observations!.onResponse(response.request().method(), response.url(), response.status());
    page.on("pageerror", pageErrorHandler);
    page.on("console", consoleHandler);
    page.on("requestfailed", requestFailedHandler);
    page.on("response", responseHandler);
    detachPageObservers = () => {
      page?.off("pageerror", pageErrorHandler);
      page?.off("console", consoleHandler);
      page?.off("requestfailed", requestFailedHandler);
      page?.off("response", responseHandler);
    };

    const response = await withinRunDeadline(
      page.goto(fixture.ready.startUrl, { waitUntil: "domcontentloaded" }),
      deadlineAt,
      abort.signal
    );
    if (response === null || response.status() < 200 || response.status() >= 400) {
      failure = { kind: "navigation", step: "boot" };
    }
    try {
      await withinRunDeadline(
        assertExpectedBootSurface(page, NAVIGATION_TIMEOUT_MS),
        deadlineAt,
        abort.signal
      );
    } catch {
      if (abort.signal.aborted) throw interruptedError();
      failure ??= { kind: "assertion", step: "boot" };
    }

    recorder.recordStep("built-assets");
    const referencedAssets = await withinRunDeadline(
      page.evaluate(() =>
        Array.from(
          document.querySelectorAll<HTMLScriptElement | HTMLLinkElement>(
            "script[src],link[rel='stylesheet'][href]"
          ),
          (element) =>
            element instanceof HTMLScriptElement
              ? new URL(element.src).href
              : new URL(element.href).href
        )
      ),
      deadlineAt,
      abort.signal
    );
    const responseStatuses = observations.snapshot().responseStatuses;
    if (referencedAssets.length === 0) failure ??= { kind: "assertion", step: "built-assets" };
    for (const assetUrl of referencedAssets) {
      if (
        !isAllowedHttpUrl(assetUrl, fixture.ready.origin) ||
        !assetUrl.startsWith(fixture.ready.origin + "/assets/") ||
        !isSuccessful(responseStatuses.get(assetUrl))
      ) {
        failure ??= { kind: "network", step: "built-assets" };
      }
    }
    failure ??= observations.snapshot().failure;
    if (abort.signal.aborted) failure ??= { kind: "harness", step: "boot" };

    if (process.env.NOEMA_BROWSER_FORCE_SAFE_FAILURE === "1") {
      recorder.recordStep("forced-safe-failure");
      failure = { kind: "assertion", step: "forced-safe-failure" };
    }
    if (process.env.NOEMA_BROWSER_FORCE_HARNESS_FAILURE === "1") {
      failure ??= { kind: "assertion", step: "boot" };
      throw new Error("forced browser harness failure");
    }

    if (failure === undefined) {
      await boundedCleanup(page.close(), CLEANUP_TIMEOUT_MS);
      browserOwner.releasePage(page);
      detachPageObservers();
      page = undefined;
      await boundedCleanup(context.close(), CLEANUP_TIMEOUT_MS);
      browserOwner.releaseContext(context);
      context = undefined;
      failure ??= observations.freeze().failure;
    } else {
      detachPageObservers();
      observations.freeze();
    }

    if (failure === undefined) {
      publication = await recorder.finish({ outcome: "passed" });
      recorderFinished = true;
    } else {
      publication = await recorder.finish({
        outcome: "failed",
        failureKind: failure.kind,
        step: failure.step,
        captureScreenshot:
          page === undefined
            ? undefined
            : async () =>
                Uint8Array.from(
                  await boundedCleanup(page!.screenshot({ type: "png" }), CLEANUP_TIMEOUT_MS)
                )
      });
      recorderFinished = true;
      retainArtifacts = publication.files.length > 0;
    }
  } catch {
    detachPageObservers?.();
    observations?.freeze();
    if (abort.signal.aborted) throw new Error("browser acceptance interrupted");
    if (recorder !== undefined && !recorderFinished) {
      try {
        publication = await recorder.finish({
          outcome: "failed",
          failureKind: "harness",
          step: "boot",
          captureScreenshot:
            page === undefined || abort.signal.aborted
              ? undefined
              : async () =>
                  Uint8Array.from(
                    await boundedCleanup(page!.screenshot({ type: "png" }), CLEANUP_TIMEOUT_MS)
                  )
        });
        recorderFinished = true;
        retainArtifacts = publication.files.length > 0;
      } catch {
        // The fixed outer failure summary remains the only output.
      }
    }
    if (publication === undefined) throw new Error("browser acceptance failed");
  } finally {
    let cleanupError: unknown;
    if (browserOwner !== undefined) {
      try {
        await browserOwner.close();
      } catch (error) {
        cleanupError = error;
      }
    }
    if (fixture !== undefined) {
      try {
        await fixture.close({ retainArtifacts });
      } catch (error) {
        cleanupError ??= error;
      }
    }
    assertCleanupSucceeded(cleanupError);
  }

  if (publication?.reason === "passed") return undefined;
  return publication;
}

async function requireBuiltAssets(): Promise<void> {
  const indexPath = join(ASSET_ROOT, "index.html");
  const indexInfo = await lstat(indexPath);
  if (!indexInfo.isFile() || indexInfo.isSymbolicLink()) {
    throw new Error("built browser entry is unavailable");
  }
  const index = await readFile(indexPath, "utf8");
  const references = [...index.matchAll(/(?:src|href)="\/assets\/([^"]+)"/gu)].map(
    (match) => match[1]!
  );
  if (references.length === 0) throw new Error("built browser assets are unavailable");
  for (const name of references) {
    if (name.includes("/") || name === "." || name === "..") {
      throw new Error("built browser assets are invalid");
    }
    const info = await lstat(join(ASSET_ROOT, name));
    if (!info.isFile() || info.isSymbolicLink()) {
      throw new Error("built browser assets are unavailable");
    }
  }
}

function parseCli(args: readonly string[]): "boot" {
  if (args.length === 0 || (args.length === 1 && args[0] === "boot")) return "boot";
  throw new Error("browser scenario is unsupported");
}

function isSuccessful(status: number | undefined): boolean {
  return status !== undefined && status >= 200 && status < 400;
}

async function withinRunDeadline<T>(
  operation: Promise<T>,
  deadlineAt: number,
  signal: AbortSignal
): Promise<T> {
  const timeoutMs = deadlineAt - Date.now();
  if (timeoutMs <= 0) throw new Error("browser acceptance timed out");
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await raceWithAbortSignal(
      Promise.race([
        operation,
        new Promise<never>((_resolve, reject) => {
          timer = setTimeout(() => reject(new Error("browser acceptance timed out")), timeoutMs);
        })
      ]),
      signal
    );
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

function printFailure(publication: BrowserArtifactPublication): void {
  process.stdout.write("NOEMA browser acceptance: FAIL\n");
  for (const file of publication.files) process.stdout.write(file + "\n");
}

function assertCleanupSucceeded(error: unknown): void {
  if (error !== undefined) throw new Error("browser acceptance cleanup failed");
}
