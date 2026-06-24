# Noema

Noema is an open-source, local-first personal agent operating system.

This repository currently contains the first minimal Rust runtime slices:

- a small one-shot `noema` CLI that sends one prompt to a configured model
  provider and prints the response
- a foreground Noema daemon that owns a long-lived Codex app-server runtime
  for `noema chat`

Two providers are currently supported:

- `openai`: calls the OpenAI Responses API with a Platform API key.
- `codex`: shells out to `codex exec` for one-shot prompts and uses
  `codex app-server` inside the daemon for chat.

## Requirements

- Rust and Cargo
- `NOEMA_OPENAI__API_KEY` in the environment when using `provider: openai`
- Codex CLI installed and authenticated when using `provider: codex`

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
```

Configuration precedence is:

```text
CLI flags > environment variables > $NOEMA_HOME/config.yaml or ~/.noema/config.yaml > defaults
```

Supported environment variables:

- `NOEMA_HOME`
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
`db/noema.sqlite` and can be inspected with `noema memory list` and
`noema memory show <id>`.

## Development

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

For daemon development, install `cargo-watch` and use the repo alias to restart
the foreground daemon whenever Rust sources or Cargo manifests change:

```bash
cargo install cargo-watch --locked
NOEMA_HOME=.noema-dev cargo dev-daemon
```

Then connect from another terminal:

```bash
NOEMA_HOME=.noema-dev cargo run -p noema-cli -- chat
```
