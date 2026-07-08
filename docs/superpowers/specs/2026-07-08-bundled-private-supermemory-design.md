# Bundled Private Supermemory Design

## Summary

Noema managed memory should not require users to install or configure
Supermemory separately. Managed mode will launch a bundled `supermemory-server`
sidecar, keep its endpoint private to Noema, and remove managed-mode port and
base URL controls from the Settings UI.

This design keeps Supermemory as a separate process because the upstream
self-hosted server is distributed as a binary and currently exposes an HTTP API
configured by `PORT` or `SUPERMEMORY_PORT`. Noema will hide that transport
detail from users and use a runtime-only loopback endpoint until Supermemory
offers a true non-port transport such as Unix sockets, inherited file
descriptors, stdio, or an embeddable library.

## Goals

- Bundle the Supermemory server used by Noema managed mode.
- Remove the managed-mode requirement that `supermemory-server` already exists
  on `PATH`.
- Hide managed-mode base URL and port controls from Settings > Memory.
- Avoid a predictable fixed Supermemory port such as `6767` in managed mode.
- Keep External mode for advanced users who want to point Noema at their own
  Supermemory URL.
- Keep Noema's memory boundary as Supermemory API calls; do not mirror
  Supermemory memory truth or graph state in SQLite.
- Preserve useful diagnostics when the bundled sidecar is missing, cannot
  launch, or cannot become ready.

## Non-Goals

- No fork of Supermemory.
- No custom Supermemory protocol in this slice.
- No guarantee that managed mode uses no TCP listener at all; upstream
  Supermemory currently requires an HTTP listener.
- No user-facing graph browser, transcript memory markers, or `/remember`
  reintroduction.
- No SurrealDB or in-house graph-memory compatibility path.

## Architecture

Managed mode becomes a private sidecar lifecycle:

1. Noema resolves a `supermemory-server` executable.
2. Noema chooses a runtime-only loopback port.
3. Noema starts Supermemory with Noema-owned data and secret directories.
4. Noema waits for readiness and stores sanitized status in SQLite.
5. Noema clients and models interact with memory only through Noema APIs and
   tools, never by learning the managed Supermemory endpoint.

The managed Supermemory endpoint is process-private operational state. It is
not displayed in Settings, not accepted from the user, and not persisted as a
stable user setting.

External mode remains URL-based. When mode is External, Noema does not launch a
process, and Settings > Memory exposes the external base URL and connection
status.

## Binary Resolution

Managed mode resolves the server binary in this order:

1. `NOEMA_SUPERMEMORY_SERVER`, for developer and support overrides.
2. The bundled Noema resource path for the current platform.
3. `supermemory-server` on `PATH` as a development fallback.

Release builds should treat a missing bundled binary as a packaging error. The
`PATH` fallback exists to keep source-tree development flexible, not as the
normal product dependency.

The resolver should return structured diagnostics:

- `supermemory_server_missing` when no candidate exists.
- `supermemory_server_not_executable` when a candidate exists but cannot be
  executed.
- `supermemory_start_failed` when process spawn fails for another reason.

## Runtime Endpoint

Managed mode uses a loopback-only HTTP endpoint because that is the upstream
transport Supermemory exposes today.

Requirements:

- Bind target is loopback only, never `0.0.0.0`.
- Port is selected at runtime and is not the default fixed `6767` unless every
  private-port attempt fails and the explicit fallback path reports that fact.
- The chosen endpoint is kept in memory on the runtime host and injected into
  the Supermemory client boundary.
- SQLite stores readiness status and diagnostic errors, not the runtime
  managed endpoint.

Port selection may use a bind-to-port-zero probe followed by process launch.
Because releasing the probe before child launch has a small race, startup should
retry with a fresh loopback port when readiness fails in a way consistent with a
bind conflict.

## Data And Secrets

Managed mode continues to use:

```text
${NOEMA_HOME:-$HOME/.noema}/supermemory/data
${NOEMA_HOME:-$HOME/.noema}/supermemory/secrets
```

Noema passes `SUPERMEMORY_DATA_DIR` to the child process. If Supermemory
requires or prints an API key on first boot, Noema stores the credential only
under the Supermemory secrets directory and supplies it to the internal client.
SQLite may store whether managed auth is configured, but not the secret value.

## Settings UI

Settings > Memory should present two clear modes:

- Managed Local: Noema owns the bundled local memory service.
- External URL: user supplies and tests a Supermemory-compatible endpoint.

Managed Local shows:

- Service status.
- Managed data location, with the same technical-details treatment already used
  for local paths.
- Extraction model preference.
- Retry/start action when unavailable.

Managed Local does not show:

- Port input.
- Base URL input.
- The runtime private endpoint.

External URL keeps the base URL control and readiness check.

## GraphQL And Store Shape

The GraphQL contract should distinguish managed private configuration from
external URL configuration:

- Managed settings expose mode, status, model preference, and data-location
  metadata.
- External settings expose mode, status, base URL, and model preference.

The existing SQLite memory service table can be rewritten pre-V1. The desired
shape is that user settings do not require a managed `base_url` or `port`.
If implementation keeps those columns temporarily, managed-mode code must treat
them as internal legacy fields and not rely on them for normal startup.

## Error Handling

Managed lifecycle errors should update `memory_service_status` with sanitized
codes and messages before returning:

- Missing binary.
- Non-executable binary.
- Spawn failure.
- Readiness timeout.
- Authentication failure.
- Unexpected HTTP status.

Raw process and readiness details continue to go to `${NOEMA_HOME}/errors.log`.
User-facing Settings copy should say what action is possible: retry, check
packaging, or switch to External URL.

## Testing

Backend tests should cover:

- Binary resolver prefers `NOEMA_SUPERMEMORY_SERVER`.
- Binary resolver finds a bundled candidate before `PATH`.
- Missing binary persists `unavailable` with `supermemory_server_missing`.
- Managed startup uses a non-default runtime port when the port allocator
  succeeds.
- Managed mode client construction uses the runtime endpoint, not persisted
  settings.
- External mode still uses the configured external base URL and never spawns a
  sidecar.

Frontend tests are not required for this UI slice by current project
instructions. Frontend validation should use generated GraphQL types, lint, and
build.

## Rollout

This is a pre-V1 clean change. There is no migration path for older managed
port/base URL settings. Existing local Supermemory data remains in the same
Noema home directory so the sidecar can continue using it.

The first implementation can support macOS and Linux, matching upstream
self-hosted Supermemory support. Unsupported platforms should show managed mode
as unavailable with a clear packaging/status error while preserving External
URL mode.
