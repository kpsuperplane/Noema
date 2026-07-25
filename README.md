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

## Requirements

- Rust and Cargo
- Bun for frontend dependency installation and builds
- For macOS desktop builds: Xcode (including its command-line tools) and the
  Cargo Tauri CLI
- A provider account for chat:
  - OpenAI Platform API key for `provider: openai`
  - Noema-managed Codex OAuth credentials for `provider: codex`, created through
    provider onboarding

## Product Surfaces

The first-party product API is GraphQL. The local web UI uses `/graphql` for
queries and mutations plus `/graphql/ws` for subscriptions. The desktop app
uses the same GraphQL schema through Tauri commands and events instead of a
local HTTP server.

`noema-host` owns configuration, startup, composition, and dependency-ordered
shutdown. During startup it uses `noema-home` to initialize `${NOEMA_HOME}`,
composes the SQLite-backed `noema-store`, starts the native `noema-memory`
subsystem, and assembles provider and capability implementations. SQLite lives
at `${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`, canonical human memory lives
under `${NOEMA_HOME:-$HOME/.noema}/memory/human/`, its rebuildable FTS index
lives under `system/indexes/`, and provider credential material lives under
`${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/`.

## Configuration

Non-secret defaults may live in the Noema directory's `config.yaml`. The default
Noema directory is `~/.noema`; set `NOEMA_HOME` to use another directory.

Example OpenAI-oriented configuration:

```yaml
provider: openai
model: gpt-5.5
openai:
  base_url: https://api.openai.com/v1
  organization_id: org_...
  project_id: proj_...
  timeout_seconds: 120
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
- `NOEMA_OPENAI__API_KEY`
- `NOEMA_OPENAI__BASE_URL`
- `NOEMA_OPENAI__TIMEOUT_SECONDS`
- `NOEMA_OPENAI__ORGANIZATION_ID`
- `NOEMA_OPENAI__PROJECT_ID`
- `NOEMA_CODEX__MODEL`
- `NOEMA_CODEX__TIMEOUT_SECONDS`
- `NOEMA_WEB__HOST`
- `NOEMA_WEB__PORT`
- `NOEMA_WEB__RP_ID`
- `NOEMA_WEB__PUBLIC_ORIGIN`

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

On a fresh database, the server prints a one-use setup URL. Opening that URL
authorizes the browser to register the first FIDO passkey for `human:local`;
ordinary first visitors cannot claim an unconfigured public instance. Later
browsers authenticate with that passkey and receive a private `HttpOnly`,
`SameSite=Strict`, and, for HTTPS origins, `Secure` session cookie. Sessions are
process-local, so a server restart requires passkey authentication again.

Treat `web.rp_id` as durable identity configuration: changing it invalidates
credentials registered under the old id. Passkey recovery and additional-key
management are not implemented yet, so do not deploy without retaining access
to the registered synced passkey or authenticator.

Example Codex-oriented configuration:

```yaml
provider: codex
codex:
  model: gpt-5.5
  timeout_seconds: 300
```

Codex authentication is handled as Noema-owned provider account state. The web
onboarding flow blocks chat until an active provider account is authenticated.

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
required `sccache` compiler wrapper; the launcher checks each target at most
every six hours and automatically rebuilds it after it exceeds its configured
budget, so routine development does not grow `target/debug` without bound.

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
NOEMA_HOME=.noema-dev cargo run
```

`cargo dev` enables the explicit debug-only `dev-no-auth` feature and binds the
development server to `0.0.0.0`, so the UI does not require the one-shot browser
bootstrap URL and can be opened from another device on the LAN. This is an
unauthenticated development server: use it only on a trusted network. Direct
daemon runs and release builds remain loopback-only and retain the authenticated
browser session boundary.

This expects `cargo-watch` to be installed because it restarts the Rust web
server on backend changes.

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

Use the Rust desktop crate for desktop-side validation:

```bash
cargo check -p noema-desktop
cargo test -p noema-desktop
```

## Repository Layout

```text
apps/web/                     React UI and GraphQL operation generation
crates/noema-home/            Home layout, initialization, safe paths, diagnostics
crates/noema-conversations/   Conversation and transcript domain contracts
crates/noema-artifacts/       Governed artifact contracts and filesystem service
crates/noema-capabilities/    Provider-neutral capability and tool contracts
  mcp/                        MCP contracts and optional local transport adapter
crates/noema-providers/       Provider contracts, adapters, and local GGUF models
crates/noema-tasks/           Task, run, submission, and review domain contracts
crates/noema-memory/          Native Markdown memory and derived search
crates/noema-store/           SQLite persistence and persistence read models
crates/noema-runtime/         Governed, transport-neutral agent execution
crates/noema-host/            Configuration, composition, startup, and shutdown
crates/noema-api/             Transport-neutral GraphQL schema and resolvers
crates/noema-server/          HTTP/WebSocket shell and release web-asset owner
crates/noema-desktop/         Tauri shell using the shared host and GraphQL API
crates/noema-model-evals/     Opt-in local-model qualification runner
docs/                         Current design notes and historical plans/specs
```
