# Noema

Noema is an open-source, always-on, self-hosted personal agent operating
system.

This repository currently contains the active Rust product paths:

- `noema-core`: the local runtime host, SQLite store, Supermemory integration, provider
  adapters, GraphQL schema, local web transport, memory systems, MCP control
  plane, and generated web assets.
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
- Bun for frontend development
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
`${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`, manages local Supermemory state
under `${NOEMA_HOME:-$HOME/.noema}/supermemory/`, and stores provider credential
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

Frontend assets are built with Bun and embedded into `noema-core`:

```bash
cd crates/noema-core/web
bun install
bun run gen:types
bun run lint
bun run build
```

For standalone web development, run the combined local supervisor from the repo
root:

```bash
NOEMA_HOME=.noema-dev cargo dev
```

This expects `cargo-watch` to be installed because it restarts the Rust web
server on backend changes.

For frontend development against the desktop app, run the Tauri-oriented Vite
build/watch task:

```bash
cd crates/noema-core/web
bun run dev:tauri
```

For desktop packaging assets:

```bash
cd crates/noema-core/web
bun run build:tauri
```

Then use the Rust desktop crate for desktop-side validation:

```bash
cargo check -p noema-desktop
cargo test -p noema-desktop
```

## Repository Layout

```text
crates/noema-core/       Rust runtime, store, providers, GraphQL, and web assets
crates/noema-core/web/   React UI and GraphQL operation generation
crates/noema-desktop/    Tauri desktop app
docs/                    Current design notes and historical plans/specs
```
