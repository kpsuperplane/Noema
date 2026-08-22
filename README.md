# Noema

Noema is an open-source, always-on, self-hosted personal agent operating
system.

The Rust backend is split by ownership rather than collected behind an umbrella
crate. `noema-host` composes the application from the runtime, persistence,
provider, memory, and capability crates; `noema-api` exposes the shared GraphQL
schema; and `noema-server` and `noema-desktop` provide the HTTP and Tauri
process boundaries. `apps/web` is the React product UI shared by both shells.

The old multi-command Noema CLI surface has been removed. For standalone local
web development, use the `cargo dev` supervisor. It runs the `noema_web`
GraphQL/web server watcher next to the Bun web asset watcher. Local product work
should use that web entrypoint, the owning backend crate, the web frontend
package, or the desktop app.

```bash
NOEMA_HOME=.noema-dev cargo dev
```

`cargo dev` currently requires Unix because it always enables the private Unix
GraphQL socket. On macOS, it also watches the Foundation bridge package and
runs `swift build` after bridge changes.

## Requirements

- Rust and Cargo
- Bun for frontend dependency installation and builds
- `cargo-watch` for the combined development supervisor
- CMake, Clang, and libclang for Obscura's stealth transport
- A Unix host for `cargo dev`
- For macOS desktop builds: Xcode and its command-line tools
- For Apple Foundation Models: macOS 26 and Swift 6
- One supported chat provider:
  - OpenAI Platform for `provider: openai`
  - OpenRouter for `provider: openrouter`
  - Codex for `provider: codex`
  - Local GGUF models for `provider: local_models`
  - Apple Foundation Models for `provider: foundation_local`

OpenAI uses `NOEMA_OPENAI__API_KEY`. OpenRouter and Codex credentials are
created through provider onboarding. The desktop package locks the Tauri CLI
through Bun.

## Product Surfaces

The first-party product API is GraphQL. The local web UI uses `/graphql` for
queries and mutations plus `/graphql/ws` for subscriptions. The desktop app
uses Tauri commands and events for both its embedded schema and authenticated
HTTPS connections to a remote server.

`noema-host` owns configuration, startup, composition, and dependency-ordered
shutdown. During startup it uses `noema-home` to initialize `${NOEMA_HOME}`,
composes the SQLite-backed `noema-store`, starts the native `noema-memory`
subsystem, and assembles provider and capability implementations. SQLite lives
at `${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`, durable human memory lives
under `${NOEMA_HOME:-$HOME/.noema}/memory/human/`, its rebuildable FTS index
lives under `system/indexes/`, and provider credential material lives under
`${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/`.

## Configuration

Non-secret defaults may live in the Noema directory's `config.yaml`. The default
Noema directory is `~/.noema`; set `NOEMA_HOME` to use another directory.

Example OpenAI-oriented configuration:

```yaml
provider: openai
openai:
  base_url: https://api.openai.com/v1
  organization_id: org_...
  project_id: proj_...
  timeout_seconds: 120
browser:
  max_sessions: 2
  max_old_space_mb: 1024
web:
  host: 127.0.0.1
  port: 3737
  rp_id: localhost
```

Configuration precedence is:

```text
environment variables > $NOEMA_HOME/config.yaml or ~/.noema/config.yaml > defaults
```

Supported environment variables include:

- `NOEMA_HOME`
- `NOEMA_PROVIDER`
- `NOEMA_MODEL`
- `NOEMA_REASONING_EFFORT`
- `NOEMA_OPENAI__API_KEY`
- `NOEMA_OPENAI__BASE_URL`
- `NOEMA_OPENAI__TIMEOUT_SECONDS`
- `NOEMA_OPENAI__ORGANIZATION_ID`
- `NOEMA_OPENAI__PROJECT_ID`
- `NOEMA_CODEX__MODEL`
- `NOEMA_CODEX__TIMEOUT_SECONDS`
- `NOEMA_BROWSER__MAX_SESSIONS`
- `NOEMA_BROWSER__MAX_OLD_SPACE_MB`
- `NOEMA_WEB__HOST`
- `NOEMA_WEB__PORT`
- `NOEMA_WEB__RP_ID`
- `NOEMA_WEB__PUBLIC_ORIGIN`
- `NOEMA_WEB__DEV_NO_AUTH`
- `NOEMA_WEB__LOCAL_GRAPHQL_SOCKET`
- `NOEMA_WEB__GRAPHIQL`
- `NOEMA_MCP__STDIO_ENABLED`

For a server deployment, keep Noema bound to loopback and terminate TLS in a
reverse proxy on the same host. Configure the exact browser origin and the
stable WebAuthn relying-party id explicitly:

```yaml
web:
  host: 127.0.0.1
  port: 3737
  rp_id: noema.example.com
  public_origin: https://noema.example.com
```

On a fresh database, Noema writes `web.recovery_code` to `config.yaml`.
Read that code from the server filesystem. Enter it in the setup screen.
Noema rotates the code after every attempt. A successful attempt permits one
passkey enrollment. Other application routes remain blocked until enrollment.

Later browsers authenticate with a passkey. Noema supports multiple passkeys.
It does not permit removal of the final passkey. Browser sessions use private
`HttpOnly` and `SameSite=Strict` cookies. HTTPS origins also use `Secure`
cookies. Browser sessions and their private cookie key survive server restarts.

Treat `web.rp_id` as durable identity configuration. A change invalidates
credentials registered under the old identifier.

Run public release binaries with a dedicated `noema` account. Release binaries
reject root on Unix. The example [systemd unit](deploy/systemd/noema.service)
sets a private home and removes Linux capabilities. The example
[nginx configuration](deploy/nginx/noema.conf.example) terminates TLS and adds
connection, request, body, and header limits. Replace its host and certificate
paths before use. Keep Noema bound to loopback.

Example Codex-oriented configuration:

```yaml
provider: codex
codex:
  timeout_seconds: 300
```

Codex authentication is handled as Noema-owned provider account state. The web
onboarding flow blocks chat until an active provider account is authenticated.
If you set an explicit OpenAI or Codex model, also set `reasoning_effort`.
Fast mode is disabled by default. It requests OpenAI's Fast service tier and
costs more than standard processing.

## Development

Use `.env.example` as a reference for local environment variables. Host-side
commands read variables from your shell, so export them directly or load them
with your usual environment manager:

```bash
export NOEMA_HOME="$PWD/.noema-dev"
export NOEMA_OPENAI__API_KEY="..."
```

General Rust validation:

```bash
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
```

For the normal edit loop, use the package-scoped commands instead of the full
workspace gate:

```bash
cargo fmt --all --check
cargo fast                         # dev server dependency check
cargo focused-lint && cargo focused-test
cargo gate-lint && cargo gate-test
```

`cargo fast` is intended for frequent edits. The focused pair is the unit-level
completion check. The gate pair is for cross-crate changes, milestone boundaries,
and pre-handoff validation; Clippy and tests already compile the workspace, so
the gate does not run a redundant separate `cargo check`. Cargo aliases are
single commands, so the format check is kept as an explicit first step.

These aliases keep validation artifacts in a disposable, size-bounded target
while `cargo dev` retains its own development target. Both continue to use the
required `sccache` compiler wrapper. The development server favors incremental
compilation without debug information for edit latency, while validation uses
non-incremental profiles for cache reuse. Its backend watcher coalesces short
save bursts, and the frontend keeps GraphQL Codegen and Vite's build graph warm
between edits. The launcher checks each target at most every six hours and
automatically rebuilds it after it exceeds its configured budget. Use
`cargo validate <cargo-command> [arguments]` for other focused Rust commands;
direct Cargo invocations also use the managed validation target and
non-incremental profiles by default.

Frontend assets are built with Bun. The web build is emitted under
`noema-server`, which validates and embeds those assets in release builds:

```bash
cd apps/web
bun install --frozen-lockfile
bun run check:generated
bun run lint
bun run build
```

For standalone web development, run the combined local supervisor from the repo
root:

```bash
NOEMA_HOME=.noema-dev cargo dev
```

To run the authenticated loopback server without the development asset watcher,
use the workspace default binary:

```bash
NOEMA_HOME=.noema-dev cargo validate run
```

`cargo dev` sets the runtime `web.local_graphql_socket` option. It also binds
the development server to `127.0.0.1`. Authentication follows `config.yaml` and
is enabled by default. Local tools can use `.noema-dev/run/graphql.sock`
without a passkey. Set `web.dev_no_auth: true` only on a trusted network.

The supervisor uses `cargo-watch` for the Rust server and Foundation bridge.

For frontend development against the desktop app, run the Tauri-oriented Vite
build/watch task:

```bash
cd apps/web
bun run dev:tauri
```

For a local desktop build without installer bundles, install frontend and
desktop CLI dependencies exactly from their lockfiles. Tauri's build hook
creates the desktop frontend assets:

```bash
cd apps/web
bun install --frozen-lockfile
cd ../../crates/noema-desktop
bun install --frozen-lockfile
```

Then, from the repository root:

```bash
cd crates/noema-desktop && bun run build:no-bundle
```

The current desktop bundle is developer-only; signing, notarization, updates,
and clean-machine distribution validation remain future release work.

### Connect the desktop app to a server

The desktop app uses its embedded local Noema instance by default. To connect
it to a server, create a connection link under **Settings > System > Clients**.
The link contains only the server origin. Open it with the installed desktop
app, or paste it under **Settings > System > Desktop**.

Remote connections require the server's exact `https` public origin and an
operating-system-trusted certificate. Noema rejects HTTP, localhost, IP-address
origins, redirects, and custom certificate authorities. The desktop app opens
the system browser for OAuth. Authorization requires PKCE and recent passkey
approval.

The operating system credential store keeps the origin, client identifier, and
rotating refresh credential. Access tokens stay in memory.

The desktop app keeps one active remote server. **Use local Noema** revokes the
remote OAuth family, removes its credential, and restarts the embedded instance.
Noema also restarts after a successful connection to clear server-specific UI
state.
If remote startup fails, retry the connection or return to local mode. If the
server is unavailable, **Forget this server** only removes local credentials.
Revoke that client later from another authenticated client.

Use the Rust desktop crate for desktop-side validation:

```bash
cargo validate check -p noema-desktop
cargo validate test -p noema-desktop
```

## Repository Layout

```text
apps/web/                     React UI and GraphQL operation generation
apps/ios/                     Native SwiftUI client and Live Activity extension
graphql/                      Generated shared GraphQL schema
crates/noema-home/            Home layout, initialization, safe paths, diagnostics
crates/noema-conversations/   Conversation and transcript domain contracts
crates/noema-artifacts/       Governed artifact contracts and filesystem service
crates/noema-capabilities/    Provider-neutral capability and tool contracts
  adapters/                   Native HTTP adapter manifests and compiler
  mcp/                        MCP contracts and optional local transport adapter
crates/noema-providers/       Provider contracts, adapters, and local GGUF models
crates/noema-tasks/           Task, run, submission, and review domain contracts
crates/noema-workspaces/      Workspace and project domain contracts
crates/noema-memory/          Native Markdown memory and derived search
crates/noema-store/           SQLite persistence and persistence read models
crates/noema-runtime/         Governed, transport-neutral agent execution
crates/noema-host/            Configuration, composition, startup, and shutdown
crates/noema-api/             Transport-neutral GraphQL schema and resolvers
crates/noema-server/          HTTP/WebSocket shell and release web-asset owner
crates/noema-desktop/         Tauri shell using the shared host and GraphQL API
crates/noema-dev/             Development supervisor and validation launcher
crates/noema-model-evals/     Opt-in local-model qualification runner
docs/                         Current contracts, active plans, and dated evidence
```
