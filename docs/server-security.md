# Server authentication and public access

This document describes the current server controls and deployment requirements.
[Harness security](harness/security.md) covers model context, tool calls, and information handling.
Authentication does not provide separate permissions for individual Tasks or native clients.

## Authority and limits

One instance has one administrator, `human:local`.
Authenticated browsers, native clients, and the private local socket receive that administrator's application authority.
Some operations also require browser-session context or recent passkey authentication.
There are no additional human accounts, selectable native scopes, or third-party OAuth registrations.

The boundary protects against unauthenticated network access.
It does not protect Noema data from a process that already controls the service account or host.
MCP subprocesses run as the service account, without a separate operating-system sandbox.

## Configuration

For public access, use an exact HTTPS origin and stable passkey domain:

```yaml
web:
  host: 127.0.0.1
  port: 3737
  rp_id: noema.example.com
  public_origin: https://noema.example.com
  dev_no_auth: false
  local_graphql_socket: true
  graphiql: false
mcp:
  stdio_enabled: false
```

Configuration loads at startup. Environment overrides use names such as `NOEMA_WEB__PUBLIC_ORIGIN` and `NOEMA_MCP__STDIO_ENABLED`.
Changing these settings requires a restart.
The local socket defaults to enabled on Linux and macOS. Windows does not support it.

Noema generates `web.recovery_code` in the service-owned `config.yaml` with mode `0600`.
It is a credential, not an ordinary setting to copy into documentation or diagnostics.
There is no environment override because recovery must rotate the saved value.
Address setup and recovery preserve unrelated file-backed settings.

Sources: [configuration](../internal/auth/config.go), [address setup](../internal/auth/domain_setup.go).

## First claim and passkeys

Without a configured public origin or passkeys, the browser first confirms its address.
The submitted origin must match the request Origin and Host.
HTTPS domain names and HTTP localhost are accepted.
An explicit environment origin, desktop launch, or development bypass skips address confirmation.

When no passkey exists, only setup and public application assets are available on the network listener.
GraphQL, artifacts, and native credentials cannot bypass this state.
The first visitor can register the initial passkey and claim the instance.
Only the first completed registration succeeds.
Keep an unclaimed public instance inaccessible to other visitors.

Subsequent browser access uses passkeys. Noema supports multiple passkeys and prevents normal removal of the final one.
Adding or removing a passkey normally requires passkey authentication within the same session during the previous five minutes.
Initial claim, recovery enrollment, and development bypass are exceptions.
WebAuthn validates the configured domain, origin, and user verification.
Ceremonies expire after five minutes and cannot complete twice.
Changing `web.rp_id` can make existing passkeys unusable.

Sources: [admission and ceremonies](../internal/auth/handler.go), [WebAuthn](../internal/auth/webauthn.go), [saved authentication state](../internal/store/auth.go).

## Recovery

The human reads the current recovery code directly from `config.yaml`.
`POST /auth/recovery` accepts it in a bounded request body with Origin validation.
Noema compares the candidate in constant time and saves a replacement before returning the result.
Every parsed candidate rotates the code, including an incorrect candidate.
Malformed request bodies are rejected before that comparison.
A storage failure cannot create a recovery session.

A correct code creates a browser-bound setup session lasting five minutes.
That session can register a passkey but cannot read application data.
Registration consumes it and creates a normal authenticated browser session.
A later recovery invalidates earlier setup sessions.
Recovery preserves existing passkeys and native clients; it does not revoke possibly compromised access automatically.

Invalid submissions can force repeated code rotation and delay legitimate recovery.
Read the latest file value before another attempt.
Recovery entry is unavailable during development authentication bypass.

Sources: [rotation](../internal/auth/config.go), [recovery route](../internal/auth/handler.go), [setup sessions](../internal/auth/session.go).

## Browser sessions and requests

Browser cookies use `HttpOnly` and `SameSite=Strict`.
HTTPS configurations also use `Secure` and the `__Host-` prefix.
SQLite stores session digests and state. A protected file stores the cookie key.
Both survive server restarts.

Sessions have a 24-hour idle limit and a 30-day absolute limit.
The default store capacity is 1,024 sessions; full capacity rejects new sessions instead of evicting active ones.
Logout, expiry, and passkey removal revoke affected sessions and close their WebSockets.

The server compares the incoming Host directly with its configured authority.
It does not select authority from forwarded headers.
Cookie-backed protected POST requests and browser WebSocket upgrades require the configured Origin.
Native bearer authentication follows its separate admission path.

Public GraphQL POST bodies are limited to 512 KiB. The private local socket and CLI retain a 64 KiB limit.
Authentication admission also bounds concurrent HTTP and WebSocket requests.
Private API and authentication responses use `Cache-Control: no-store`.
Public assets can use content-hash caching; authenticated favicons have a private cache policy.
Browser headers restrict script sources, framing, base URLs, referrers, and browser features.

Sources: [request admission and headers](../internal/auth/handler.go), [session storage](../internal/store/auth.go), [cookie handling](../internal/auth/session.go).

## Native clients

The fixed iOS and desktop clients use Authorization Code with PKCE `S256`.
The system browser authenticates the human and requires recent passkey approval.
Codes bind the client, redirect, and PKCE challenge, and are consumed once.
The server accepts `noema://oauth/callback` for iOS.
Desktop uses an HTTP loopback callback with an explicit port and exact callback path.
The server accepts `127.0.0.1` and `::1`; the desktop client currently binds `127.0.0.1`.
These callback addresses do not relax the clients' HTTPS requirement for the server origin.

Native access tokens expire. Refresh credentials rotate on successful use.
SQLite retains credential digests and refresh-family state.
Reuse outside the supported retry path revokes the family.
Revocation closes related WebSockets and disables associated push registrations.

The iOS client saves a refresh request identifier before transmission.
A retry with that identifier and credential can recover the exact response while its direct successor remains active.
The response is retained in a protected transient-auth file, not ordinary SQLite state.
Desktop currently omits this identifier and uses the older 60-second retry path.
Remove that compatibility path after desktop adopts request-bound refresh and its supported transition release ends.

Native clients keep access tokens in memory and refresh credentials in operating-system credential storage.
iOS uses device-only Keychain storage. Desktop keeps credentials outside the webview.
Normal disconnect revokes server access before deleting the local profile.
An offline device can retain private data after server revocation.

Sources: [OAuth protocol](../internal/auth/native_protocol.go), [native admission](../internal/auth/native_oauth.go), [retry storage](../internal/auth/native_retry.go), [desktop refresh](../crates/noema-desktop/src/remote_graphql.rs).
See the [iOS client](../apps/ios/README.md) for cache and connection behavior.

## Development bypass and local access

`web.dev_no_auth: true` grants administrator authority to accepted network requests without passkey login.
It preserves Host and Origin checks, but those checks do not replace authentication.
The setting does not change the listener address or enable GraphiQL or MCP stdio.
Credentials issued during bypass can remain valid after it is disabled.
Keep the listener private and inspect passkeys and native grants before restoring public access.

The private Unix socket uses a `0700` parent directory and `0600` socket.
Filesystem access grants administrator authority even before the first passkey exists.
It does not create a browser session; operations requiring that session can remain unavailable.
It serves GraphQL, subscriptions, app assets, schema, and authenticated status.
It does not expose recovery, OAuth, artifacts, or GraphiQL.
No public header or TCP route selects this authority.

See [CLI access](cli.md), [development permissions](development/codex-permissions.md), and [browser inspection](frontend/browser-inspection.md).
Source: [local socket](../internal/web/local_graphql_unix.go).

## GraphiQL and MCP processes

GraphiQL is disabled by default. When enabled, it uses normal authentication and bundled assets.
MCP stdio is also disabled by default. Disabled stdio connections remain stored but cannot start a process.
Hosted HTTP MCP does not need the stdio setting.

An enabled stdio connection starts the exact executable and argument vector without shell parsing.
Only explicit environment bindings enter that subprocess.
The subprocess can still read files and use authority available to the Noema service account.
Enabling both stdio and development authentication bypass lets reachable clients configure service-account processes.

Source: [stdio process launcher](../internal/mcp/stdio.go).

## Public deployment

Run release binaries under a dedicated service account. Unix release binaries reject root.
Use a private Noema home and restricted file permissions.
The [systemd example](../deploy/systemd/noema.service) removes Linux capabilities and limits writable paths.
This separation limits host access; it does not isolate Noema data from its own process.

Keep the application listener on loopback behind a TLS reverse proxy.
The [nginx example](../deploy/nginx/noema.conf.example) supplies request, connection, body, and header limits.
Preserve the configured public Host. Keep credentials and secret-bearing callback data out of proxy logs.
Do not expose the private local socket through public routing.

Before exposing an instance, verify passkey setup, exact origin, disabled development bypass, and the actual proxy limits.
Check recovery, revocation, OAuth retries, and WebSocket termination through that deployment.
Repository tests and sample configuration do not prove a particular public deployment is secure.
Review dependency advisories against the deployed version; dated audit results are not a standing assurance.

Installed web data is also subject to the [PWA cache and logout boundary](frontend/pwa.md).
