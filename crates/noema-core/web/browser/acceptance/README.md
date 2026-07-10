# Deterministic browser acceptance

This directory owns the external Node/Playwright acceptance runner. Foundation
F5c1 supports only `boot`: it compiles the existing `noema_core` libtest,
launches the ignored fixture on a random loopback port, loads the already-built
web application in a fresh Chromium context, and then shuts down every owned
process and temporary home.

Build the frontend before invoking the runner:

Node 26.3.0 is required exactly, as pinned by the repository `.node-version`
and the web package `engines.node` contract. F5c3 browser CI must run
`actions/setup-node` with `node-version: 26.3.0` before invoking these commands.

```bash
bun run build
bunx playwright install chromium
node --experimental-strip-types browser/acceptance/run.ts boot
```

The Chromium-only security/negative proofs are intentionally outside ordinary
`test:ci` because they launch the real fixture and browser:

```bash
node --experimental-strip-types browser/acceptance/proofs.ts browser-acquisition-abort
node --experimental-strip-types browser/acceptance/proofs.ts signal-interruption
node --experimental-strip-types browser/acceptance/proofs.ts websocket-egress
node --experimental-strip-types browser/acceptance/proofs.ts status-rejection
```

The WebSocket proof confirms that the same-origin GraphQL socket connects while
an external loopback socket is closed by Playwright routing before its local
server receives any connection. The status proof injects a deterministic boot
GraphQL failure and confirms the shared boot assertion rejects the visible
`Noema status` surface instead of accepting it as a successful nonblank page.

The entrypoints run under Node because Bun's WebSocket transport cannot connect
to Playwright 1.61.1 BrowserServer on loopback. Bun remains the test, generation,
and frontend-build runtime. The acquisition-abort proof uses pinned real
Chromium and verifies that a launch resolving after cancellation is force-killed
before the proof returns.

The signal-interruption proof sends SIGINT and SIGTERM after browser work is in
flight. It requires the exact fixed FAIL summary, exit 130/143, empty stderr,
and no newly retained synthetic fixture root.

`NOEMA_BROWSER_FORCE_HARNESS_FAILURE=1 node --experimental-strip-types browser/acceptance/run.ts boot`
exercises catch-path finalization after an earlier assertion state exists. It
must emit the same fixed safe file allowlist as other public failures, with a
`harness` failure in the bounded trace. Like the safe-failure switch, this is a
runner-only proof input and is stripped from Cargo and never forwarded to Rust.

The fixture child receives an allowlisted environment rooted in a fresh private
OS-temporary directory. It never receives caller homes, provider/API variables,
Codex variables, or the Bun-only forced-failure switch. Cargo keeps the caller's
configured compiler wrapper and cache environment.

The runner permits only HTTP requests to the validated random-port loopback
origin. It disables Playwright trace, video, storage-state, and automatic report
capture. It does not read browser cookies, storage, request headers, response
bodies, or WebSocket frames.

The runner and proof commands own SIGINT/SIGTERM. The first signal aborts any
in-flight Cargo discovery, fixture readiness, or browser operation, performs
bounded cleanup, and exits with 130/143. Their handlers are removed immediately
so a second signal retains the operating system's default force behavior.
Browser owners, fixture streams/processes, local proof servers, and private
temporary roots all have finite cleanup deadlines.

Browser ownership starts with `chromium.launchServer()` on IPv4 loopback and a
separate Playwright connection. The owner tracks the server, connection,
context, and page. A late acquisition is observed and destroyed; a timed-out or
rejected browser close escalates to bounded `BrowserServer.kill()`.

Successful ephemeral runs retain no artifacts. To verify the safe F5b failure
path, set `NOEMA_BROWSER_FORCE_SAFE_FAILURE=1`. The command exits nonzero and
prints only the fixed failure summary and the safe relative F5b allowlist. The
retained temporary ownership root contains only the artifact directory; fixture
home, cache, runtime, database, and temp directories are removed.

Streaming, cancellation, authentication, provider, memory, MCP, download, and
GraphQL body/WebSocket scenarios intentionally remain deferred to later F5c
units and remediation milestones.
