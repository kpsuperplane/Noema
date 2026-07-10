interface DestroyableStream {
  destroy(): unknown;
}

export interface OwnedStdio {
  readonly stdin: DestroyableStream;
  readonly stdout: DestroyableStream;
  readonly stderr: DestroyableStream;
}

export async function boundedCleanup<T>(
  operation: Promise<T>,
  timeoutMs: number
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_resolve, reject) => {
        timer = setTimeout(() => reject(new Error("cleanup timed out")), timeoutMs);
      })
    ]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

export async function awaitOwnedStdioCleanup(
  completion: Promise<void>,
  streams: OwnedStdio,
  timeoutMs = 3_000,
  failureMessage = "owned process output cleanup failed"
): Promise<void> {
  try {
    await boundedCleanup(completion, timeoutMs);
    return;
  } catch {
    for (const stream of [streams.stdin, streams.stdout, streams.stderr]) {
      try {
        stream.destroy();
      } catch {
        // Continue destroying every owned stream.
      }
    }
  }
  try {
    await boundedCleanup(completion, timeoutMs);
  } catch {
    throw new Error(failureMessage);
  }
}

export async function drainBounded(
  stream: AsyncIterable<Uint8Array | string>,
  maxLineBytes: number,
  maxTotalBytes: number,
  message: string
): Promise<void> {
  const utf8 = new TextEncoder();
  let total = 0;
  let pending = 0;
  for await (const rawChunk of stream) {
    const chunk = typeof rawChunk === "string" ? utf8.encode(rawChunk) : Uint8Array.from(rawChunk);
    total += chunk.byteLength;
    if (total > maxTotalBytes) throw new Error(message);
    for (const byte of chunk) {
      pending = byte === 0x0a ? 0 : pending + 1;
      if (pending > maxLineBytes) throw new Error(message);
    }
  }
}

export async function waitForChildExit(
  child: {
    readonly exitCode: number | null;
    readonly signalCode?: NodeJS.Signals | null;
    once(event: "close", listener: (code: number | null) => void): unknown;
  },
  timeoutMs: number
): Promise<number | undefined> {
  if (child.exitCode !== null) return child.exitCode;
  if (child.signalCode != null) return 128;
  return new Promise((resolveExit) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      resolveExit(undefined);
    }, timeoutMs);
    child.once("close", (code) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolveExit(code ?? 1);
    });
  });
}
