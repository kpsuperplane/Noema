# Tauri macOS App Design

## Status

Approved design for adding a self-contained unsigned macOS developer build of
Noema as a Tauri app. No implementation has been done in this spec. The next
step is an implementation plan.

## Context

Noema currently has three relevant pieces:

- `noema-core`, which owns configuration, paths, embedded SurrealDB storage,
  provider account state, the Codex runtime, GraphQL schema/resolvers, and the
  daemon web server.
- `noema-cli`, which starts the daemon, initializes `~/.noema`, runs local
  chat, and supports the combined Rust/web dev daemon workflow.
- `crates/noema-core/web`, a Bun/Vite React app that talks to Noema through
  GraphQL over HTTP plus WebSocket subscriptions.

The current daemon web server binds a local TCP listener and serves the React
assets plus `/graphql` and `/graphql/ws`. That is fine for CLI/local web
development, but it is not the desired privacy shape for a non-technical
desktop app. The macOS app should feel like one ordinary app: double-click
Noema, chat with Noema, quit Noema. It should not expose a web server to other
processes on the user's machine.

## Goals

- Add a Tauri-based macOS app target for Noema.
- Produce an unsigned developer macOS app bundle in the first implementation
  unit.
- Keep the normal user path terminal-free.
- Start Noema's local runtime invisibly when the app launches.
- Shut down the app-owned runtime cleanly when the app quits.
- Avoid binding Noema's HTTP/WebSocket web server in desktop mode.
- Keep using `${NOEMA_HOME:-~/.noema}` so desktop and CLI share one local data
  home.
- Reuse the existing React onboarding and chat UI.
- Preserve GraphQL as Noema's first-party client API.
- Add native browser-open behavior for provider authentication URLs.
- Keep existing CLI and daemon web modes working.

## Non-Goals

- Do not add signing, notarization, auto-update, or release distribution.
- Do not target the Mac App Store sandbox in this slice.
- Do not replace the CLI daemon or remove the local web development path.
- Do not change Noema's default data home to `~/Library/Application Support`.
- Do not add login item, background launch, menu bar, tray, or multi-window
  behavior.
- Do not add native macOS settings/preferences UI.
- Do not fork the product API away from GraphQL.
- Do not build backwards compatibility layers for pre-V1 desktop experiments.

## Product Decisions

- The first desktop target is for non-technical users.
- The desktop app owns its runtime lifecycle invisibly.
- Desktop mode does not bind a TCP web server.
- Desktop mode loads bundled React assets through Tauri, not through
  `http://127.0.0.1`.
- Desktop and CLI share `~/.noema`.
- Existing React onboarding remains the first setup experience.
- Provider authentication opens verification URLs in the system browser through
  a Tauri command while still showing code/status fallback UI in the app.
- The first app build is unsigned and developer-focused.
- If another process already owns resources needed by the app, the first
  version fails closed with clear recovery text instead of trying to merge
  lifecycles.

## Architecture

Add a new workspace member at `crates/noema-desktop` as the Tauri app. It
depends on `noema-core` and starts a desktop runtime host inside the Tauri
process. It does not call the daemon web server entrypoint and does not bind the
daemon TCP listener.

The desktop runtime host should:

1. Resolve and initialize `NoemaPaths` from process environment.
2. Open the embedded store using `StoreConfig::from_paths`.
3. Ensure the default provider account exists.
4. Spawn the existing `CodexRuntimeHandle`.
5. Create a `ProviderAuthManager`.
6. Build the existing `GraphqlSchema` from a desktop-owned `GraphqlState`.
7. Execute GraphQL requests received over Tauri IPC.
8. Bridge GraphQL subscription events back to the webview through Tauri
   events or channels.
9. Shut down the runtime on application exit.

The current daemon path should keep serving HTTP/WebSocket GraphQL for CLI and
browser development. Shared runtime construction logic should move into
`noema-core` so daemon and desktop mode do not duplicate store/runtime/provider
setup.

## Component Boundaries

### `noema-core`

Responsibilities:

- Provide a reusable runtime construction path for daemon and desktop hosts.
- Keep product behavior inside existing runtime, repository, provider auth,
  onboarding, and GraphQL resolver modules.
- Expose enough desktop-safe APIs to execute the existing GraphQL schema
  without going through HTTP.
- Keep daemon-specific TCP and Unix-socket behavior in daemon modules.

The extracted runtime construction API should stay focused. It should not
become a broad service locator or desktop framework.

### `noema-desktop`

Responsibilities:

- Own the Tauri application shell.
- Configure app metadata, bundle settings, window, frontend asset path, and
  developer build commands.
- Initialize the desktop runtime once during startup.
- Store the runtime/schema handles in managed Tauri state.
- Provide IPC commands for GraphQL execution, subscription registration or
  event bridging, opening external URLs, and optional desktop health/status.
- Present startup and fatal initialization errors in plain language.
- Shut down the runtime cleanly on quit.

### React Web App

Responsibilities:

- Continue owning onboarding, chat, memory, shell, and route UI.
- Continue using generated GraphQL operations and types.
- Select the transport at runtime:
  - Browser/daemon mode uses `HttpLink` and `GraphQLWsLink`.
  - Tauri mode uses a desktop Apollo link backed by Tauri IPC and events.
- Call a native browser-open helper when provider auth returns a verification
  URL and the app is running in Tauri.

The operation documents and generated types should remain shared across both
transports.

## Data Flow

Browser/daemon mode remains:

```text
React UI
  -> Apollo HTTP/WebSocket links
  -> daemon web server
  -> async-graphql schema
  -> Noema store/runtime/provider paths
```

Desktop mode becomes:

```text
React UI
  -> Apollo desktop IPC link
  -> Tauri command
  -> async-graphql schema.execute(...)
  -> Noema store/runtime/provider paths
  -> GraphQL response
  -> React UI
```

Subscription flow in desktop mode:

```text
Runtime live event
  -> GraphQL subscription registry
  -> desktop subscription bridge
  -> Tauri event/channel
  -> Apollo subscription observer
  -> React transcript update
```

Provider authentication flow:

```text
React onboarding mutation
  -> GraphQL desktop IPC
  -> provider auth attempt
  -> verification URL + user code
  -> React calls Tauri open-url command
  -> system browser opens provider verification page
  -> React polling/check flow remains visible in app
```

## GraphQL IPC Contract

Desktop mode should keep the GraphQL request/response shape close to
`async_graphql::Request` and `async_graphql::Response`, serialized as JSON for
Tauri IPC. Use these commands as the first contract:

```text
graphql_execute(request_json) -> response_json
graphql_subscribe(subscription_id, request_json) -> accepted
graphql_unsubscribe(subscription_id) -> accepted
open_external_url(url) -> accepted
```

The desktop Apollo link owns conversion between Apollo operations and the JSON
request sent to the Tauri command.

Subscriptions use this lifecycle:

1. React subscribes with an operation and variables.
2. Rust creates a subscription stream against the existing GraphQL schema.
3. Rust emits each result to the webview on a `graphql_subscription_event`
   event with the subscription id.
4. React calls `graphql_unsubscribe` when Apollo tears down the observer.
5. Rust stops the stream and releases resources.

The bridge should avoid exposing arbitrary Rust commands to frontend code. Only
the specific Noema app operations needed for GraphQL transport and URL opening
belong in Tauri capabilities.

## User Experience

On launch, the app shows a minimal startup state while the desktop runtime
initializes. Once ready, it renders the normal Noema React shell. If setup is
incomplete, the existing onboarding flow appears in the app window.

Normal users should not see:

- TCP ports.
- Unix socket paths.
- `noema start`.
- GraphQL transport details.
- Raw environment variables.
- Secret credential values.

Technical details can be available behind an explicit reveal for debugging.
Those details may include paths and exact Rust error strings.

Provider auth should open verification URLs in the user's default browser. The
onboarding view should still show the user code and retry/check controls so the
flow remains usable if browser opening fails.

## Error Handling

User-facing errors should be plain language first:

- Data folder preparation failed: "Noema could not open its data folder."
- Store open failed: "Noema could not start its local memory store."
- Runtime startup failed: "Noema could not start the local assistant service."
- Provider auth failed: use existing onboarding error copy.
- Browser open failed: "Noema could not open your browser."
- GraphQL IPC failed: "Noema lost connection to its local app service."

Desktop initialization should fail closed. If the app cannot safely open the
shared store, initialize paths, start the runtime, or build the schema, it
should show a recoverable error screen instead of partially opening the UI.

The first implementation does not need automatic recovery from every lifecycle
conflict. A clear message and technical details are enough when another process
appears to own local resources.

## Build And Packaging

The first build path should support an unsigned developer macOS build. A repo
script or documented command should run the web build and Tauri build in the
right order.

The web app should remain buildable with Bun from `crates/noema-core/web`.
Tauri should consume the existing web build output as its frontend distribution
rather than requiring the daemon asset server. Do not duplicate the React source
tree for desktop.

Tauri development may use a Vite dev server for frontend hot reload. The
unsigned developer bundle must use static frontend assets and must not require a
Noema HTTP/WebSocket server.

The workspace should avoid making every normal Rust validation command require
macOS-only bundle work. Tauri-specific bundle checks can be separate from
general `cargo check --workspace` if needed.

## Security And Privacy

Desktop mode should not bind Noema's HTTP/WebSocket listener. The only app UI
transport is the Tauri webview's IPC/event channel.

Tauri capabilities should be narrow:

- Execute Noema GraphQL operations.
- Register/unregister GraphQL subscriptions.
- Open external provider auth URLs.
- Read minimal desktop health/startup state if needed.

The app should not expose filesystem APIs, shell execution, arbitrary network
fetching, or broad process management to the frontend. Provider network traffic
still happens inside the existing Noema provider/runtime paths under the same
policy assumptions as CLI/daemon mode.

## Testing

Rust tests should cover:

- Extracted runtime construction behavior where it can be tested without
  provider network calls.
- Desktop GraphQL execution against test `GraphqlState`.
- Desktop subscription bridge lifecycle using a bounded test stream or existing
  subscription registry test helper.
- Shutdown calls the runtime shutdown path.

Frontend tests should cover:

- GraphQL client chooses browser transport outside Tauri.
- GraphQL client chooses desktop transport when Tauri APIs are present.
- Provider auth URL opening calls the desktop helper in Tauri mode.
- Provider auth URL opening remains a no-op or browser-safe fallback outside
  Tauri mode.

Manual verification should cover:

- Desktop app launches into a Tauri window.
- No desktop HTTP/WebSocket listener is bound.
- Existing web daemon mode still works at the configured local web URL.
- `LocalStatus` and `OnboardingStatus` work through desktop IPC.
- Provider auth opens the system browser.
- Starting the primary conversation and sending a message work.
- Subscription streaming updates the transcript.
- Quitting the app shuts down the app-owned runtime.
- An unsigned macOS developer bundle can be produced.

Before committing implementation, run the relevant project checks:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

For frontend changes, also run from `crates/noema-core/web`:

```bash
bun run gen:types
bun run lint
bun run build
```

## Implementation Planning Decisions

- Put the Tauri app in `crates/noema-desktop`.
- Use `graphql_execute`, `graphql_subscribe`, `graphql_unsubscribe`, and
  `open_external_url` as the first Tauri commands.
- Emit desktop subscription payloads on `graphql_subscription_event`.
- Extract a focused `noema-core` runtime host object that contains the store,
  runtime handle, provider auth manager, paths, subscription registry, and
  GraphQL state inputs needed by daemon and desktop.
- Allow Vite only as a development asset server for `tauri dev`; production
  developer bundles use static frontend assets.
- Verify the no-listener property primarily by code structure and manual
  desktop launch checks: the desktop app must not call daemon web listener code,
  and a launched desktop app should not require `127.0.0.1:3737`.
