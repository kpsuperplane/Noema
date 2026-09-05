# Noema

Noema is an open-source, always-on, self-hosted personal agent operating
system.

The Go server owns composition, runtime, persistence, providers, capabilities,
GraphQL, and HTTP. Rust remains for the Tauri shell, model evaluations, and the
external Obscura worker. `apps/web` is the React UI shared by both shells.

The old multi-command Noema CLI surface has been removed. For standalone local
web development, use the `cargo dev` supervisor. It runs the Go server watcher
next to the Bun web asset watcher. Local product work
should use that web entrypoint, the owning backend crate, the web frontend
package, or the desktop app.

```bash
NOEMA_HOME=.noema-dev cargo dev
```

`cargo dev` currently requires Unix because it always enables the private Unix
GraphQL socket. On macOS, it also watches the Foundation bridge package and
runs `swift build` after bridge changes.

## Requirements

- Go 1.26.6
- Rust and Cargo for desktop, evaluations, and the Obscura release worker
- Bun for frontend dependency installation and builds
- `cargo-watch` for the combined development supervisor
- CMake, Clang, and libclang when building Obscura release workers
- Tesseract OCR and `prlimit` for printed English text in raster images
- A Unix host for `cargo dev`
- For macOS desktop builds: Xcode and its command-line tools
- For Apple Foundation Models: macOS 26 and Swift 6
- One supported chat provider:
  - OpenAI Platform for `provider: openai`
  - OpenRouter for `provider: openrouter`
  - Codex for `provider: codex`
  - Local GGUF models for `provider: local_models`
  - Apple Foundation Models for `provider: foundation_local`

OpenAI uses `NOEMA_OPENAI__API_KEY`. On a fresh home, this key completes the
OpenAI provider and model setup. OpenRouter and Codex credentials use provider
onboarding. The desktop package locks the Tauri CLI through Bun.

## Product Surfaces

The first-party product API is GraphQL. The local web UI uses `/graphql` for
queries and mutations plus `/graphql/ws` for subscriptions. The desktop app
uses Tauri transport for its packaged Go sidecar and authenticated remote server.

`cmd/noema` owns startup, composition, and dependency-ordered shutdown.
Packages under `internal/` own runtime, store, memory, providers, capabilities,
GraphQL, and HTTP. SQLite lives
at `${NOEMA_HOME:-$HOME/.noema}/db/noema.sqlite3`, durable human memory lives
under `${NOEMA_HOME:-$HOME/.noema}/memory/human/`, its rebuildable FTS index
lives under `system/indexes/`, and provider credential material lives under
`${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/`.

## Configuration

Browser, web, and MCP defaults may live in the Noema directory's `config.yaml`.
The default Noema directory is `~/.noema`. Set `NOEMA_HOME` to use another
directory.

Example server configuration:

```yaml
browser:
  max_sessions: 2
  max_old_space_mb: 1024
web:
  host: 127.0.0.1
  port: 3737
  rp_id: localhost
```

For settings available in both places, environment variables override
`config.yaml` values.

Supported environment variables include:

- `NOEMA_HOME`
- `NOEMA_PROVIDER` with the `openai` value for environment setup
- `NOEMA_OPENAI__API_KEY`
- `NOEMA_OPENAI__ORGANIZATION_ID`
- `NOEMA_OPENAI__PROJECT_ID`
- `NOEMA_MODEL`
- `NOEMA_REASONING_EFFORT`
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

The OpenAI key selects OpenAI when `NOEMA_PROVIDER` is unset. You can also set
`NOEMA_PROVIDER=openai`. The key enters the protected provider credential
store. It does not enter `config.yaml` or SQLite.

If you set `NOEMA_MODEL`, select a current OpenAI profile and also set
`NOEMA_REASONING_EFFORT`. Otherwise, Noema stores its current recommended model
assignments. Existing provider credentials and model assignments remain
unchanged.

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

Codex authentication is handled as Noema-owned provider account state. The web
onboarding flow blocks chat until an active provider account is authenticated.
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

Go server validation:

```bash
CGO_ENABLED=0 go test ./cmd/... ./internal/...
CGO_ENABLED=0 go vet ./cmd/... ./internal/...
```

Retained Rust validation:

```bash
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
```

Use `go test` with package paths for focused server checks. Use
`cargo validate <cargo-command> [arguments]` for focused retained Rust checks.
The supervisor coalesces short save bursts. The frontend keeps GraphQL
generation and Vite's build graph warm between edits.

Frontend assets are built with Bun. The web build is emitted under
`target/web-assets`. The Go release build validates and embeds those assets:

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
run the Go command:

```bash
NOEMA_HOME=.noema-dev go run ./cmd/noema
```

`cargo dev` sets the runtime `web.local_graphql_socket` option. It also binds
the development server to `127.0.0.1`. Authentication follows `config.yaml` and
is enabled by default. Local tools can use `${NOEMA_HOME}/run/graphql.sock`
without a passkey. Set `web.dev_no_auth: true` only on a trusted network.

When root starts `./attach`, Cargo keeps root ownership of the build process.
The launcher stages generated files under `/run/noema-dev` and runs only the
Noema server as `noema-dev`. Its home is `/var/lib/noema-dev`.

The supervisor watches the Go server and Foundation bridge.

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
remote OAuth family, removes its credential, and restarts the local Go sidecar.
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
cmd/noema/                    Go server entrypoint and process composition
internal/                     Go server runtime, store, API, and integrations
apps/web/                     React UI and GraphQL operation generation
apps/ios/                     Native SwiftUI client and Live Activity extension
graphql/                      Generated shared GraphQL schema
crates/noema-home/            Home layout, initialization, safe paths, diagnostics
crates/noema-conversations/   Conversation and transcript domain contracts
crates/noema-artifacts/       Governed artifact contracts and filesystem service
crates/noema-capabilities/    Provider-neutral capability and tool contracts
  adapters/                   Native HTTP adapter manifests and compiler
  mcp/                        MCP records retained by model evaluations
crates/noema-providers/       Provider contracts, adapters, and local GGUF models
crates/noema-tasks/           Task, run, submission, and review domain contracts
crates/noema-workspaces/      Workspace and project domain contracts
crates/noema-memory/          Native Markdown memory and derived search
crates/noema-store/           SQLite persistence and persistence read models
crates/noema-runtime/         Governed, transport-neutral agent execution
crates/noema-desktop/         Tauri shell for local Go and remote servers
crates/noema-dev/             Development supervisor and validation launcher
crates/noema-model-evals/     Opt-in local-model qualification runner
docs/                         Current contracts, active plans, and dated evidence
```
