import {
  chromium,
  type Browser,
  type BrowserContext,
  type Page
} from "playwright";

import { interruptedError, raceWithAbortSignal } from "./abortOwner.ts";
import { boundedCleanup } from "./cleanup.ts";

const DEFAULT_ACQUISITION_TIMEOUT_MS = 10_000;
const DEFAULT_CLEANUP_TIMEOUT_MS = 3_000;

export interface BrowserServerLike {
  wsEndpoint(): string;
  close(): Promise<void>;
  kill(): Promise<void>;
}

export interface BrowserConnectionLike {
  newContext(options?: Parameters<Browser["newContext"]>[0]): Promise<BrowserContext>;
  close(): Promise<void>;
}

export interface BrowserLauncherLike {
  launchServer(options: {
    readonly timeout: number;
    readonly host: "127.0.0.1";
    readonly handleSIGHUP: false;
    readonly handleSIGINT: false;
    readonly handleSIGTERM: false;
  }): Promise<BrowserServerLike>;
  connect(endpoint: string, options: { readonly timeout: number }): Promise<BrowserConnectionLike>;
}

export interface BrowserOwnerLaunchOptions {
  readonly launcher?: BrowserLauncherLike;
  readonly signal?: AbortSignal;
  readonly acquisitionTimeoutMs?: number;
  readonly cleanupTimeoutMs?: number;
}

const playwrightLauncher: BrowserLauncherLike = {
  launchServer: (options) => chromium.launchServer(options),
  connect: (endpoint, options) => chromium.connect(endpoint, options)
};

export class BrowserProcessOwner {
  readonly #launcher: BrowserLauncherLike;
  readonly #cleanupTimeoutMs: number;
  #server: BrowserServerLike | undefined;
  #browser: BrowserConnectionLike | undefined;
  #context: BrowserContext | undefined;
  #page: Page | undefined;
  #closed = false;

  private constructor(launcher: BrowserLauncherLike, cleanupTimeoutMs: number) {
    this.#launcher = launcher;
    this.#cleanupTimeoutMs = cleanupTimeoutMs;
  }

  static async launch(options: BrowserOwnerLaunchOptions = {}): Promise<BrowserProcessOwner> {
    const timeoutMs = options.acquisitionTimeoutMs ?? DEFAULT_ACQUISITION_TIMEOUT_MS;
    const cleanupTimeoutMs = options.cleanupTimeoutMs ?? DEFAULT_CLEANUP_TIMEOUT_MS;
    const owner = new BrowserProcessOwner(options.launcher ?? playwrightLauncher, cleanupTimeoutMs);
    const serverLaunch = owner.#launcher.launchServer({
      timeout: timeoutMs,
      host: "127.0.0.1",
      handleSIGHUP: false,
      handleSIGINT: false,
      handleSIGTERM: false
    });
    try {
      owner.#server = await acquireOwned(
        serverLaunch,
        options.signal,
        timeoutMs,
        cleanupTimeoutMs,
        (server) => server.kill()
      );
      owner.#browser = await acquireOwned(
        owner.#launcher.connect(owner.#server.wsEndpoint(), { timeout: timeoutMs }),
        options.signal,
        timeoutMs,
        cleanupTimeoutMs,
        (browser) => browser.close()
      );
      return owner;
    } catch (error) {
      let cleanupFailed =
        error instanceof Error && error.message === "browser acquisition cleanup failed";
      try {
        await owner.#killServer();
      } catch {
        cleanupFailed = true;
      }
      if (cleanupFailed) throw new Error("browser acquisition cleanup failed");
      if (options.signal?.aborted === true) throw interruptedError();
      if (error instanceof Error && error.message === "browser acceptance interrupted") {
        throw error;
      }
      throw new Error("browser acquisition failed");
    }
  }

  async newContext(
    options: Parameters<Browser["newContext"]>[0],
    signal?: AbortSignal,
    timeoutMs = DEFAULT_ACQUISITION_TIMEOUT_MS
  ): Promise<BrowserContext> {
    if (this.#browser === undefined || this.#closed) {
      throw new Error("browser owner is unavailable");
    }
    this.#context = await acquireOwned(
      this.#browser.newContext(options),
      signal,
      timeoutMs,
      this.#cleanupTimeoutMs,
      (context) => context.close()
    );
    return this.#context;
  }

  async newPage(
    signal?: AbortSignal,
    timeoutMs = DEFAULT_ACQUISITION_TIMEOUT_MS
  ): Promise<Page> {
    if (this.#context === undefined || this.#closed) {
      throw new Error("browser context is unavailable");
    }
    this.#page = await acquireOwned(
      this.#context.newPage(),
      signal,
      timeoutMs,
      this.#cleanupTimeoutMs,
      (page) => page.close()
    );
    return this.#page;
  }

  releasePage(page: Page): void {
    if (this.#page === page) this.#page = undefined;
  }

  releaseContext(context: BrowserContext): void {
    if (this.#context === context) this.#context = undefined;
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    let failed = false;
    let forceKill = false;

    if (this.#page !== undefined) {
      try {
        await boundedCleanup(this.#page.close(), this.#cleanupTimeoutMs);
      } catch {
        failed = true;
      }
      this.#page = undefined;
    }
    if (this.#context !== undefined) {
      try {
        await boundedCleanup(this.#context.close(), this.#cleanupTimeoutMs);
      } catch {
        failed = true;
      }
      this.#context = undefined;
    }
    if (this.#browser !== undefined) {
      try {
        await boundedCleanup(this.#browser.close(), this.#cleanupTimeoutMs);
      } catch {
        failed = true;
        forceKill = true;
      }
      this.#browser = undefined;
    }

    if (this.#server !== undefined) {
      if (forceKill) {
        try {
          await this.#killServer();
        } catch {
          failed = true;
        }
      } else {
        try {
          await boundedCleanup(this.#server.close(), this.#cleanupTimeoutMs);
          this.#server = undefined;
        } catch {
          failed = true;
          try {
            await this.#killServer();
          } catch {
            // Preserve one stable cleanup failure.
          }
        }
      }
    }

    if (failed) throw new Error("browser cleanup failed");
  }

  async #killServer(): Promise<void> {
    const server = this.#server;
    this.#server = undefined;
    if (server === undefined) return;
    await boundedCleanup(server.kill(), this.#cleanupTimeoutMs);
  }
}

async function acquireOwned<T>(
  operation: Promise<T>,
  signal: AbortSignal | undefined,
  timeoutMs: number,
  cleanupTimeoutMs: number,
  dispose: (value: T) => Promise<unknown>
): Promise<T> {
  const deadlineAt = Date.now() + timeoutMs;
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await raceWithAbortSignal(
      Promise.race([
        operation,
        new Promise<never>((_resolve, reject) => {
          timer = setTimeout(() => reject(new Error("browser acquisition timed out")), timeoutMs);
        })
      ]),
      signal
    );
  } catch (error) {
    if (timer !== undefined) {
      clearTimeout(timer);
      timer = undefined;
    }
    const settlement = await settleThroughDeadline(operation, deadlineAt);
    if (settlement.kind === "pending") {
      throw new Error("browser acquisition cleanup failed");
    }
    if (settlement.kind === "fulfilled") {
      try {
        await boundedCleanup(
          Promise.resolve().then(() => dispose(settlement.value)),
          cleanupTimeoutMs
        );
      } catch {
        throw new Error("browser acquisition cleanup failed");
      }
    }
    if (signal?.aborted === true) throw interruptedError();
    throw error;
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

type AcquisitionSettlement<T> =
  | Readonly<{ kind: "fulfilled"; value: T }>
  | Readonly<{ kind: "rejected" }>
  | Readonly<{ kind: "pending" }>;

async function settleThroughDeadline<T>(
  operation: Promise<T>,
  deadlineAt: number
): Promise<AcquisitionSettlement<T>> {
  const remainingMs = Math.max(0, deadlineAt - Date.now());
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation.then<AcquisitionSettlement<T>, AcquisitionSettlement<T>>(
        (value) => ({ kind: "fulfilled", value }),
        () => ({ kind: "rejected" })
      ),
      new Promise<AcquisitionSettlement<T>>((resolvePending) => {
        timer = setTimeout(() => resolvePending({ kind: "pending" }), remainingMs);
      })
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}
