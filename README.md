# Noema

Noema is an open-source, always-on, self-hosted personal agent operating
system.

This repository currently contains the active Rust product paths:

- `noema-core`: the local runtime host, SQLite store, Mnemosyne integration,
  provider adapters, GraphQL schema, memory systems, and MCP control plane.
- `noema-server`: the loopback HTTP transport, browser session boundary,
  embedded web assets, and standalone web development entrypoints.
- `noema-desktop`: a macOS Tauri app that starts the Noema runtime host inside
  the desktop process and talks to it through Tauri IPC/events.
- `crates/noema-core/web`: the React product UI used by the web and desktop
  surfaces.

The old multi-command Noema CLI surface has been removed. For standalone local
web development, use the `cargo dev` supervisor. It runs the `noema_web`
GraphQL/web server watcher next to the Bun web asset watcher. Local product work
should use that web entrypoint, the core library, the web frontend package, or
the desktop app.

```bash
NOEMA_HOME=.noema-dev cargo dev
```

## Requirements

- Rust and Cargo
- Bun for frontend dependency installation and builds
- Python 3.10 or newer for the managed Mnemosyne sidecar
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

`noema-core` owns the runtime and store. It initializes `${NOEMA_HOME}` when the
runtime host starts, opens SQLite at
`${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`, manages local Mnemosyne state
under `${NOEMA_HOME:-$HOME/.noema}/mnemosyne/`, and stores provider credential
material under `${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/`.

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
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
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

Frontend assets are built with Bun. The web build is emitted under
`noema-server`, which validates and embeds those assets in release builds:

```bash
cd crates/noema-core/web
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
cd crates/noema-core/web
bun run dev:tauri
```

For an unsigned macOS desktop bundle, install frontend dependencies exactly
from the lockfile and run the Tauri build from the desktop crate. Tauri's build
hook creates the desktop frontend assets:

```bash
cd crates/noema-core/web
bun install --frozen-lockfile
```

Then, from the repository root:

```bash
cd crates/noema-desktop && cargo tauri build
```

The current desktop bundle is developer-only, not a clean-machine
self-contained release: managed Mnemosyne still launches through an ambient
Python 3.10+ installation, requires its dependencies to be installed in that
interpreter, and uses the source-tree sidecar directory. The desktop bundle does
not provision that environment; an explicit sidecar command is the current
alternative.

Use the Rust desktop crate for desktop-side validation:

```bash
cargo check -p noema-desktop
cargo test -p noema-desktop
```

## Repository Layout

```text
crates/noema-core/       Rust runtime, store, providers, and GraphQL
crates/noema-core/web/   React UI and GraphQL operation generation
crates/noema-server/     Loopback HTTP transport and release web-asset owner
crates/noema-desktop/    Tauri desktop app
docs/                    Current design notes and historical plans/specs
```
