# Server Authentication and Public Access

- **Status:** Implemented code contract; public deployment verification remains required
- **Scope:** Browser, PWA, iOS, desktop, public ingress, recovery, and local process authority
- **Human authority:** One built-in administrator, `human:local`

This document defines Noema's server authentication and public-access contract.
[`docs/harness/security.md`](harness/security.md) remains authoritative for
information classes, model context, tool governance, and egress policy.

Each requirement applies when its feature is enabled. Public routing still
requires the deployment checks in [Public Enablement](#public-enablement).

## 1. Threat Model

Noema must protect the instance from unauthenticated network access. It must
also contain host damage after a Noema vulnerability enables process execution.
Noema does not try to remain secure after a separate actor controls the host.
Host control includes access to Noema's startup configuration and service data.

The first public release does not add:

- Multiple human identities
- OIDC or federated identity
- Selectable native-client scopes
- Third-party OAuth clients
- Per-tool operating-system users
- General process sandbox infrastructure

## 2. Principals And Authority

One instance has one built-in administrator named `human:local`.

Browser sessions and native clients use different credentials. They receive the
same application authority after successful authentication.
The server retains each platform `client_id`. Each refresh family identifies
one installation and owns its revocation, Push, WebSockets, and audit history.

The local development GraphQL socket also receives `human:local` authority.
Filesystem access to that socket is its authentication boundary.

Credential type does not change application authority. A browser session and a
native client must pass the same authorization checks.
A recent-passkey rule applies equally to browser and native clients. A native
client completes that rule through the system browser.

## 3. Startup Configuration

The target configuration is:

```yaml
web:
  host: 127.0.0.1
  rp_id: noema.example.com
  public_origin: https://noema.example.com
  dev_no_auth: false
  local_graphql_socket: false
  graphiql: false
  recovery_code: <generated-base64url-value>
mcp:
  stdio_enabled: false
```

The related environment variables are:

```text
NOEMA_WEB__DEV_NO_AUTH=true
NOEMA_WEB__LOCAL_GRAPHQL_SOCKET=true
NOEMA_WEB__GRAPHIQL=true
NOEMA_MCP__STDIO_ENABLED=true
```

The flags load only during startup. Changing one requires a server restart.
Recovery-code reads and rotations are the sole live config-file exception.
GraphQL and other remote APIs must not change these values.
`web.recovery_code` is filesystem-only. An environment override would prevent
durable one-time rotation and is therefore not supported.

The recovery design requires Noema to update `config.yaml`. The file must use
mode `0600` and belong to the Noema service account.
Recovery is the only secret permitted in the ordinary startup file. This
exception does not permit other credentials in that file.

Recovery writes must serialize and atomically replace only the file-backed
field. They must preserve unrelated file values and exclude environment values.

## 4. Required Authentication Mode

`web.dev_no_auth` defaults to `false`.

When it is false, every application request requires one of these credentials:

- An authenticated browser session
- A valid native access token
- A setup-only session for one permitted setup action

This network rule does not apply to the local development GraphQL socket.

Noema has two passkey initialization states:

- `setup_required`: no passkey exists
- `ready`: one or more passkeys exist

During `setup_required`, the server permits only:

- Static application assets
- PWA manifest and worker assets
- Authentication status
- Recovery-code entry
- Passkey registration start
- Passkey registration finish

The server rejects all other inbound operations. This rule includes GraphQL,
WebSockets, artifacts, native access, pairing, OAuth callbacks, and GraphiQL.

Passkey registration still requires a setup-only session. In normal mode, the
human gets that session by entering the recovery code.

Recovery is the only normal-mode setup authority. Noema must remove the printed
bootstrap URL and its process-local capability.
The server must apply this barrier before browser or native authentication.
Existing native credentials cannot bypass a zero-passkey state.
Unreadable or malformed passkey state must fail closed.

## 5. Development Authentication Bypass

`web.dev_no_auth` is an explicit runtime bypass. It replaces the current
compile-time `dev-no-auth` feature.

When enabled, Noema authenticates every accepted request as `human:local`.

The mode has these effects:

- Passkey login is skipped.
- Mandatory passkey onboarding is skipped.
- Existing-passkey confirmation is skipped during passkey management.
- Authentication status reports the disabled mode.

Creating a passkey still requires its WebAuthn registration ceremony.

The bypass must not change:

- Listener address
- Public origin
- Host validation
- Origin validation
- TLS behavior
- GraphiQL state
- MCP stdio state

Any client that can reach this mode has complete `human:local` authority.
A reverse proxy adds no authentication unless it enforces an access policy.
Valid external controls include VPN policy, mTLS, or proxy authentication.

Native credentials issued in this mode remain valid after the bypass is
disabled. The operator must revoke any unwanted clients.

The operator must keep the edge private while using this mode for recovery.
Before public access returns, the operator must review passkeys and native
grants, disable the mode, restart, and verify passkey login.

### Local Codex GraphQL

`web.local_graphql_socket` defaults to `false`. When enabled, Noema creates
`${NOEMA_HOME}/run/graphql.sock` for local Codex development.

The socket uses HTTP GraphQL requests over a Unix domain socket. Its parent
directory uses mode `0700`, and the socket uses mode `0600`.

Each socket request authenticates as `human:local`. It bypasses passkey login,
recent-passkey checks, browser sessions, and the zero-passkey setup barrier.

The socket exposes only GraphQL POST requests. It does not expose GraphiQL,
WebSockets, assets, artifacts, recovery, OAuth, or other HTTP routes.

A separate socket router constructs the `human:local` principal at its boundary
and calls the shared GraphQL schema. No header, path, Host value, source address,
or public-listener middleware branch can select this authority.

The normal GraphQL authorization and resource limits still apply. The stdio MCP
flag remains independent and continues to guard process creation.

Enabling the socket does not change setup state, create browser sessions, or
authorize any TCP request. Its bypass exists only for each socket request.

The public listener and reverse proxy must never route this socket. Noema does
not provide an unauthenticated TCP fallback or a local bearer token.

Local Codex can use any Unix-socket HTTP client. For example:

```sh
curl --unix-socket /path/to/noema/run/graphql.sock \
  --header 'Content-Type: application/json' \
  --data '{"query":"{ __typename }"}' \
  http://localhost/graphql
```

## 6. Passkeys

Passkeys remain the primary browser authentication method.
Noema does not add passwords, email links, or TOTP. The file-backed recovery
code is the only fallback credential.
Noema owns identity, session, setup, and recovery policy. A maintained WebAuthn
library must own ceremony validation and protocol cryptography.

Registration and authentication require exact RP ID, exact origin, and user
verification. Ceremony challenges bind to one browser session, expire after
five minutes, and succeed only once.

Noema must support multiple passkeys for `human:local`. It must not require more
than one passkey.

Each passkey record has a stable credential identifier and its independent
WebAuthn credential state.

Authentication must update only the credential that completed the ceremony.
Registration must exclude credentials already stored for the human.
Normal passkey addition and removal require recent passkey authentication.
Development bypass and recovery enrollment are explicit exceptions.

Recent authentication means passkey success in the same browser session during
the previous five minutes.

Normal web settings cannot delete the final passkey. The recovery flow can
restore access when no usable passkey remains.

The RP ID is durable instance identity. Operators must not change it without a
planned credential migration.

## 7. One-Time Recovery Code

The recovery code is a secret bearer credential. Possession grants authority to
enroll a new passkey.

The design trusts access to `config.yaml` as host-level Noema authority. The
recovery code must never enter logs, events, errors, model context, URLs, or API
responses.

### 7.1 Generation And Storage

Noema must ensure that `web.recovery_code` exists before opening the listener.

If the field is missing, Noema must:

1. Generate 32 bytes with a cryptographically secure random generator.
2. Encode them as unpadded base64url.
3. Save the resulting 43-character value to `config.yaml` atomically.
4. Re-read and verify the committed value.

A configured value must be canonical unpadded base64url that decodes to exactly
32 bytes. An empty or malformed value stops startup with a field-specific error.

Noema must fail startup if it cannot create or securely rotate the code.

Each atomic write must preserve unrelated file values and mode `0600`.

The plaintext field uses a typed secret with redacted debug output. It must not
enter the ordinary resolved `WebConfig` value.

The human reads the current code directly from `config.yaml`. Noema must not
print it to ordinary daemon output.

### 7.2 Attempt Boundary

Recovery uses `POST /auth/recovery` with a bounded body. It never accepts the
code in a URL, GraphQL operation, or browser storage.

Origin validation always applies, including when a request has an Authorization
header. Proxies, browsers, and service workers must not retry this route.

The server serializes recovery attempts. For each parsed candidate, it compares
the current code in constant time, creates and atomically saves a replacement,
then returns the captured result.

Every parsed candidate triggers replacement, whether correct, malformed, or
incorrect. Noema returns no match result before the replacement is durable.

A persistence failure creates no setup session. The server reloads the config
before another attempt or disables recovery when it cannot reload safely.

An invalid attempt returns one generic error. The UI tells the human to read the
new code from `config.yaml` before another attempt.

### 7.3 Successful Recovery

A correct code creates a browser-bound, single-success setup session. It expires
after five minutes. The rotated code remains stored for the next recovery.

The setup-only session can perform only passkey registration. It cannot access
GraphQL, native authorization, artifacts, or application data.

The UI immediately starts new-passkey enrollment. The human can retry a
cancelled WebAuthn prompt while the setup session remains valid.

Successful passkey registration creates a normal authenticated browser session.
The new passkey counts as recent authentication.

Successful registration consumes the setup session. A failed ceremony leaves
all existing passkeys and credentials unchanged.

The session permits bounded ceremony retries before expiry. A later successful
recovery invalidates all earlier setup sessions.

Existing passkeys remain visible in settings. The human can remove a lost
credential after recovery.

Recovery adds access. It does not claim a compromise or remove other access.

Recovery does not automatically revoke native clients. The human can inspect
and revoke them after entry.

### 7.4 Availability Tradeoff

An unauthenticated actor can submit invalid candidates and rotate the code.
This action cannot grant access, but it can delay legitimate recovery.

This availability tradeoff is accepted. The operator can always read the newest
code from the local startup configuration.

Recovery-code entry is unavailable while `web.dev_no_auth` is enabled.

## 8. Browser Sessions

Browser sessions use private `HttpOnly` cookies with `SameSite=Strict`.
HTTPS deployments also use `Secure` and a `__Host-` cookie name.

Every request requires the exact canonical Host. Cookie-backed mutations and
browser WebSocket upgrades require the exact configured Origin. A missing
Origin fails on those routes.

Noema does not use permissive credentialed CORS. The proxy preserves the public
Host, and Noema trusts forwarding headers only from its configured proxy.

Authentication responses use `Cache-Control: no-store`. The PWA worker must not
cache authentication routes or private API responses.

The production session store must have:

- Bounded capacity
- Active expiry cleanup
- Idle expiry
- Absolute expiry
- Session identifier rotation
- Per-session revocation

Idle expiry is 24 hours of human inactivity. Absolute expiry is 30 days.
Background refresh, push, and subscription traffic do not extend idle expiry.

When the store is full, Noema rejects new sessions and ceremonies. Attacker
traffic must not evict active authenticated or setup sessions.

Logout and expiry must close the session's GraphQL WebSockets. The UI must
support current-session logout and global browser logout.

Each session records its authenticating passkey. Removing a passkey revokes its
sessions and pending ceremonies.

Passkey changes, native grants, and equivalent sensitive actions require recent
passkey authentication.

Private HTML, GraphQL, artifact, authentication, and OAuth responses use
`Cache-Control: no-store`. Immutable public assets can use content-hash caching.

Authenticated favicon responses contain only normalized public-site image
bytes. They can use `Cache-Control: private, max-age=86400` and an ETag. The
service accepts one hostname, rechecks each outbound destination, and stores
only rebuildable image data under `${NOEMA_HOME}/system/cache/favicons/`.

Browser responses use a restrictive CSP, `frame-ancestors 'none'`,
`base-uri 'none'`, `form-action 'self'`, `nosniff`, no-referrer policy, and a
minimal Permissions Policy.

## 9. Native Client OAuth

iOS and desktop are fixed first-party public clients. They receive complete
`human:local` application authority.

OAuth authorizes native transport. It does not replace passkeys as the human
authentication method or make Noema an external identity provider.

Each platform has a separate client registration. Neither registration has a
client secret.

Noema does not add selectable OAuth scopes. Every grant has the complete
`human:local` authority.

Native authorization uses Authorization Code with PKCE:

- The system browser performs authorization.
- `S256` is required.
- Missing, `plain`, and unknown PKCE methods fail.
- Each request uses fresh PKCE verifier and `state` values.
- The client verifies returned `state` before exchanging the code.
- The browser requires recent passkey authentication.
- Browser approval actions use Origin and CSRF protection.
- Codes are opaque, short-lived, single-use, and atomically consumed.
- Codes bind the human, client, redirect, and PKCE challenge.

iOS uses an exact claimed HTTPS link when deployment association is practical.
A fixed custom scheme is an explicit fallback for other self-hosted domains.
The fallback still requires PKCE and verified `state`.

Desktop uses an ephemeral loopback IP redirect. The server must match its
scheme, address, and path exactly, while permitting only the requested port.

Access and refresh credentials use these rules:

- Access tokens are opaque and short-lived.
- Persisted codes, access tokens, and refresh tokens use only server-side digests.
- Each successful refresh rotates the refresh token in one transaction.
- Used family members remain recorded until family expiry.
- Reuse revokes the complete family and its active access tokens.
- Families have inactivity and absolute expiry.
- Refresh cannot change the human, client, or audience.

Each client serializes refresh work. A lost refresh response requires new
browser authorization because retry can trigger family replay revocation.

Token responses use `Cache-Control: no-store`. Token and revocation endpoints
must not accept browser cookies as native-client authority.

iOS stores its refresh credential in device-only Keychain storage. Desktop
stores its complete protected profile in the operating-system credential store.

Clients keep access tokens only in memory. They replace stored refresh tokens
atomically after rotation.

Native clients require HTTPS and never provide a trust-all certificate mode.
Sign-out clears protected private caches after server revocation.

The operating-system account and device lock protect retained native data.
Server revocation cannot erase private data from an offline device.

The desktop Rust host owns remote credentials and transport. The packaged
webview never receives bearer credentials or executes remote server content.
Privileged IPC validates its caller origin and command arguments.

Native sign-out revokes the family before local deletion. Noema supports
individual and global native-client revocation.

Family revocation and expiry close related WebSockets and disable related push
registrations. Every request checks current token and family state.

Noema uses a maintained Rust OAuth server library. Desktop uses a maintained
Rust OAuth client library. Noema must not implement OAuth from scratch.
Noema owns durable token state, rotation, replay, and revocation. iOS uses the
platform authentication session and networking APIs.

Noema does not add OIDC, ID tokens, UserInfo, JWKS, or OIDC discovery.

The OAuth cutover removes legacy pairing routes and revokes legacy credentials.

## 10. GraphiQL

`web.graphiql` defaults to `false`.

When disabled, Noema does not register the GraphiQL route. It returns `404`.

When enabled, GraphiQL uses normal browser authentication and a restrictive
CSP. Noema pins and serves its assets without third-party scripts.

## 11. MCP Stdio Process Authority

`mcp.stdio_enabled` defaults to `false`.

The stdio session factory must enforce the flag before constructing or starting
a process. This one check covers setup, continuation, existing rows, and tool
invocation.

When disabled:

- Every stdio launch fails closed.
- Existing stdio definitions remain stored but unavailable.
- Hosted HTTP MCP remains available.

When enabled, browser and native clients have equal stdio authority.

Stdio MCP configuration is process installation. The configured executable and
arguments run with the Noema service account.

The launcher uses an exact executable and argument vector without shell parsing.
It passes only explicit MCP environment bindings, not the daemon environment.

An enabled child can read every Noema file available to the service account.
It can also act through every credential explicitly provided to that child.

This combination is an intentional double opt-in:

```text
NOEMA_WEB__DEV_NO_AUTH=true
NOEMA_MCP__STDIO_ENABLED=true
```

When both values are true, every reachable client can execute commands with the
Noema service account. Noema permits the combination. The operator owns its
external access controls.

## 12. Service And Public Edge

The public Noema service must not run as root.

Use one dedicated service account with:

- No Linux capabilities
- `NoNewPrivileges`
- A private Noema home
- Mode `0700` data directories
- Mode `0600` databases, credentials, and startup configuration
- Explicit writable data paths

The service binary and service-manager unit remain root-owned and read-only.
The startup configuration remains service-owned because recovery rotates it.

This boundary contains a Noema process-execution defect to the service account.
It does not protect Noema data from code already running as that account.

Noema does not require per-MCP users or a general container boundary for the
first public release.

The application listener remains on loopback. A separate reverse proxy owns
public TLS and WebSocket forwarding.

The edge must apply connection, request, body, header, and pre-authentication
rate limits. It must not log cookies, authorization headers, OAuth codes, PKCE
values, recovery candidates, or secret-bearing callback data.

Noema authenticates protected requests before expensive parsing. It bounds
request bodies and concurrent HTTP and WebSocket connections.

The proxy must preserve the configured public Host. It must not forward an
untrusted client value through a header that Noema treats as authoritative.

Unauthenticated production errors are bounded and stable. They do not expose
stack traces, database details, or secret values. Authenticated administrator
errors can preserve ordinary diagnostics, but they must never expose secrets.

### Public Enablement

Before public routing, verify:

- `web.dev_no_auth` is false and at least one passkey exists.
- The public origin uses HTTPS and matches the RP ID.
- GraphiQL and stdio MCP remain disabled unless explicitly enabled.
- The listener is loopback-only.
- Canonical Host enforcement and application and edge limits are active.
- An enabled local GraphQL socket has private permissions and no proxy route.

Triage dependency advisories before public routing. Only reachable
vulnerabilities within this threat model block access.

Validate setup isolation, development bypass, recovery failure and concurrency,
session revocation, OAuth replay, and WebSocket termination through the public
edge.

### Repository Advisory Review: 2026-08-16

`cargo audit` reports `RUSTSEC-2026-0194` and `RUSTSEC-2026-0195` for
`quick-xml 0.39.4`. The dependency chain is desktop-only:
`noema-desktop -> tauri -> plist -> quick-xml`.

Tauri reads the installed macOS `Info.plist` during application restart.
Public server requests do not provide this XML. Modifying the installed file
already requires local package control, which is outside this threat model.

`cargo audit` also reports `RUSTSEC-2023-0071` for `rsa 0.9.10` through
`web-push-native`. Noema creates and loads only ES256 VAPID keys. It performs no
RSA private-key operation, so the reported timing path is not reachable.

`bun audit` reports 12 advisories in `brace-expansion`, `js-yaml`, `nanoid`,
and `postcss`. These packages run in the web build, lint, and generation tools.
They do not parse requests in the deployed server or browser application.

These advisories do not block public routing under this threat model. Repeat
the reachability review after dependency or build-pipeline changes.

## 13. PWA Offline Boundary

Installed PWA data remains protected by the browser profile and device lock.
Server logout cannot erase data from an offline device.

Installed mode provides one `Log out and erase this device` action. It deletes
local private data. When online, it first revokes the server session and Push
registration. When offline, server authority remains until expiry or later
revocation.

The detailed release and cache contract remains in
[Installed Web Application](frontend/pwa.md). GraphQL, WebSocket, authentication,
OAuth, recovery, artifacts, administration, and external origins remain
network-only. The worker never retries or navigation-falls-back to those paths.
