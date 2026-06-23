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
- `OPENAI_API_KEY` in the environment when using `provider: openai`
- Codex CLI installed and authenticated when using `provider: codex`

## Usage

```bash
export OPENAI_API_KEY="..."
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
in `~/.noema/config.yaml`.

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
CLI flags > environment variables > ~/.noema/config.yaml > defaults
```

Supported environment variables:

- `OPENAI_API_KEY`
- `NOEMA_PROVIDER`
- `NOEMA_MODEL`
- `NOEMA_CODEX_COMMAND`
- `NOEMA_CODEX_SANDBOX`
- `NOEMA_CODEX_EPHEMERAL`
- `NOEMA_CODEX_IGNORE_RULES`
- `NOEMA_CODEX_IGNORE_USER_CONFIG`
- `NOEMA_CODEX_STARTUP_TIMEOUT_SECONDS`
- `NOEMA_CODEX_TURN_TIMEOUT_SECONDS`
- `NOEMA_CODEX_TIMEOUT_SECONDS`
- `NOEMA_CODEX_HOME`
- `OPENAI_BASE_URL`
- `OPENAI_TIMEOUT_SECONDS`
- `OPENAI_ORG_ID`
- `OPENAI_PROJECT_ID`

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
  codex_home: /Users/you/.codex
```

The one-shot Codex provider intentionally uses `codex exec` rather than
pretending that a ChatGPT/Codex subscription is an OpenAI API key. Daemon chat
uses `codex app-server --listen stdio://`, creates one Noema conversation per
`noema chat` session, and maps each conversation to one Codex thread. No
conversation database or memory is persisted yet.

## Development

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
```
