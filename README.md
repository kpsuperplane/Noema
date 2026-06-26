# Noema

Noema is an open-source, always-on, self-hosted personal agent operating system.

This repository currently contains the first minimal Rust runtime slices:

- a small one-shot `noema` CLI that sends one prompt to a configured model
  provider and prints the response
- a foreground Noema daemon that owns a long-lived Codex app-server runtime
  for `noema chat`
- a basic React web chat served by the core daemon

Two providers are currently supported:

- `openai`: calls the OpenAI Responses API with a Platform API key.
- `codex`: shells out to `codex exec` for one-shot prompts and uses
  `codex app-server` inside the daemon for chat.

## Requirements

- Rust and Cargo
- Bun for frontend development
- `NOEMA_OPENAI__API_KEY` in the environment when using `provider: openai`
- Codex CLI installed and authenticated when using `provider: codex`
- Postgres available through `NOEMA_DATABASE_URL` for durable structured state

## Usage

```bash
export NOEMA_OPENAI__API_KEY="..."
cargo run -p noema-cli -- "Say hello in one sentence"
```

You can also pipe stdin:

```bash
echo "Say hello in one sentence" | cargo run -p noema-cli --
```

The installed binary name is `noema`.

Daemon-backed chat uses your Codex CLI authentication and keeps the Codex
runtime warm while the daemon is alive:

```bash
codex login
cargo run -p noema-cli -- config
cargo run -p noema-cli -- start
```

`noema start` also hosts the local web chat. By default it is available at:

```text
http://127.0.0.1:3737
```

In another terminal:

```bash
cargo run -p noema-cli -- chat
```

If `noema chat` cannot connect to a daemon, it starts a temporary daemon for
that chat session, prints a hint, and shuts the temporary daemon down when chat
exits.

## Configuration

Secrets are read from environment variables only. Non-secret defaults may live
in the Noema directory's `config.yaml`. The default Noema directory is
`~/.noema`; set `NOEMA_HOME` to use another directory.

Initialize the directory with:

```bash
cargo run -p noema-cli -- config
```

`noema start` also creates the Noema directory and a default Codex-oriented
config when the directory or default config file is missing. Use
`noema config --force` to rewrite `config.yaml` with the default template.

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
CLI flags > environment variables > $NOEMA_HOME/config.yaml or ~/.noema/config.yaml > defaults
```

Supported environment variables:

- `NOEMA_HOME`
- `NOEMA_DATABASE_URL`
- `NOEMA_PROVIDER`
- `NOEMA_MODEL`
- `NOEMA_OPENAI__API_KEY`
- `NOEMA_OPENAI__BASE_URL`
- `NOEMA_OPENAI__TIMEOUT_SECONDS`
- `NOEMA_OPENAI__ORGANIZATION_ID`
- `NOEMA_OPENAI__PROJECT_ID`
- `NOEMA_CODEX__COMMAND`
- `NOEMA_CODEX__MODEL`
- `NOEMA_CODEX__SANDBOX`
- `NOEMA_CODEX__EPHEMERAL`
- `NOEMA_CODEX__IGNORE_RULES`
- `NOEMA_CODEX__IGNORE_USER_CONFIG`
- `NOEMA_CODEX__STARTUP_TIMEOUT_SECONDS`
- `NOEMA_CODEX__TURN_TIMEOUT_SECONDS`
- `NOEMA_CODEX__HOME`
- `NOEMA_WEB__HOST`
- `NOEMA_WEB__PORT`

Supported CLI flags:

```bash
noema --provider openai --model gpt-5.5 --base-url https://api.openai.com/v1 "Hello"
```

To use your Codex subscription instead of a Platform API key, sign in with the
Codex CLI first:

```bash
codex login
cargo run -p noema-cli -- --provider codex "Say hello in one sentence"
```

You can also configure Codex as the default provider:

```yaml
provider: codex
codex:
  command: codex
  sandbox: read-only
  ephemeral: true
  ignore_rules: true
  ignore_user_config: false
  startup_timeout_seconds: 60
  turn_timeout_seconds: 300
  home: /Users/you/.codex
```

The one-shot Codex provider intentionally uses `codex exec` rather than
pretending that a ChatGPT/Codex subscription is an OpenAI API key. Daemon chat
uses `codex app-server --listen stdio://`, creates one Noema conversation per
`noema chat` session, and maps each conversation to one Codex thread. Explicit
`remember this:` and `/remember` chat messages are persisted to
Postgres and can be inspected with `noema memory list` and
`noema memory show <id>`.

The web chat uses Noema's native daemon WebSocket at `/api/chat/ws`, not an
OpenAI-compatible API surface. An adapter can be added later if external client
compatibility becomes a product requirement.

## Development

Copy the default environment file once:

```bash
cp .env.example .env
```

Start the auto-reloading local development stack:

```bash
docker compose up dev
```

The `dev` service starts Postgres, the Rust daemon watcher, and the core web
asset watcher. The web chat is available at <http://localhost:3737/>. Postgres
is available on the host at `postgres://noema:noema@localhost:5432/noema`, with
physical database files stored under `${NOEMA_HOME:-$HOME/.noema}/db/postgres`.

To run the one-shot server container without file watching:

```bash
docker compose --profile server up noema-server
```

Create the test database before running database-backed repository tests:

```bash
docker compose exec postgres createdb -U noema noema_test
NOEMA_TEST_DATABASE_URL=postgres://noema:noema@localhost:5432/noema_test cargo test -p noema-core memory_persistence::postgres_tests -- --nocapture
```

General Rust validation:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
```

Frontend assets are built with Bun and embedded into `noema-core`:

```bash
cd crates/noema-core/web
bun install
bun run dev
bun run gen:types
bun run lint
bun run build
```

`bun run dev`, `bun run lint`, and `bun run build` regenerate the Rust-owned
web protocol types before running TypeScript or Vite. `bun run dev` keeps Vite
in build-watch mode and writes updated assets into `noema-core`.

For host-side daemon development, install `cargo-watch` and use the repo alias
to restart the foreground daemon whenever Rust sources, Cargo manifests, or
generated web assets change. The alias also starts the core web `bun run dev`
asset watcher:

```bash
cargo install cargo-watch --locked
docker compose up -d postgres
NOEMA_HOME=.noema-dev \
NOEMA_DATABASE_URL=postgres://noema:noema@localhost:5432/noema \
cargo dev-daemon
```

Then connect from another terminal:

```bash
NOEMA_HOME=.noema-dev \
NOEMA_DATABASE_URL=postgres://noema:noema@localhost:5432/noema \
cargo run -p noema-cli -- chat
```
