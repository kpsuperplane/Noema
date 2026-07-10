type BrowserSignal = "SIGINT" | "SIGTERM";

export interface BrowserSignalTarget {
  once(event: BrowserSignal, listener: () => void): unknown;
  removeListener(event: BrowserSignal, listener: () => void): unknown;
}

export class BrowserAbortOwner {
  readonly #target: BrowserSignalTarget;
  readonly #setExitCode: (code: number) => void;
  readonly #controller = new AbortController();
  #installed = false;
  #exitCode: number | undefined;

  readonly #onSigint = () => this.#interrupt("SIGINT");
  readonly #onSigterm = () => this.#interrupt("SIGTERM");

  constructor(target: BrowserSignalTarget, setExitCode: (code: number) => void) {
    this.#target = target;
    this.#setExitCode = setExitCode;
  }

  get signal(): AbortSignal {
    return this.#controller.signal;
  }

  get interruptedExitCode(): number | undefined {
    return this.#exitCode;
  }

  install(): void {
    if (this.#installed) return;
    this.#installed = true;
    this.#target.once("SIGINT", this.#onSigint);
    this.#target.once("SIGTERM", this.#onSigterm);
  }

  close(): void {
    if (!this.#installed) return;
    this.#installed = false;
    this.#target.removeListener("SIGINT", this.#onSigint);
    this.#target.removeListener("SIGTERM", this.#onSigterm);
  }

  async race<T>(operation: Promise<T>, phase: string): Promise<T> {
    void phase;
    return raceWithAbortSignal(operation, this.signal);
  }

  #interrupt(signal: BrowserSignal): void {
    if (this.signal.aborted) return;
    this.#exitCode = signal === "SIGINT" ? 130 : 143;
    this.#setExitCode(this.#exitCode);
    this.close();
    this.#controller.abort(interruptedError());
  }
}

export async function raceWithAbortSignal<T>(
  operation: Promise<T>,
  signal?: AbortSignal
): Promise<T> {
  const ownedOperation = observeOwnedOperation(operation);
  if (signal === undefined) return ownedOperation;
  if (signal.aborted) throw interruptedError();
  let abortListener: (() => void) | undefined;
  const aborted = new Promise<never>((_resolve, reject) => {
    abortListener = () => reject(interruptedError());
    signal.addEventListener("abort", abortListener, { once: true });
  });
  try {
    return await Promise.race([ownedOperation, aborted]);
  } finally {
    if (abortListener !== undefined) signal.removeEventListener("abort", abortListener);
  }
}

export function observeOwnedOperation<T>(operation: Promise<T>): Promise<T> {
  void operation.catch(() => undefined);
  return operation;
}

export function interruptedError(): Error {
  return new Error("browser acceptance interrupted");
}
