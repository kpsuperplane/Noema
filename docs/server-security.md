# Server Authentication and Public Access

- **Status:** Accepted target contract; implementation is pending
- **Scope:** Browser, PWA, iOS, desktop, public ingress, recovery, and local process authority
- **Human authority:** One built-in administrator, `human:local`

This document defines Noema's server authentication and public-access contract.
It does not describe the current implementation as complete.
[`docs/harness/security.md`](harness/security.md) remains authoritative for
information classes, model context, tool governance, and egress policy.

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
The server retains each platform `client_id` and issues a native grant ID.
The client ID identifies the public app registration. The grant ID identifies
one installation and authorization.

The grant ID supports:

- Independent revocation
- Refresh-token ownership
- Push ownership
- WebSocket termination
- Audit attribution

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
  graphiql: false
  recovery_code: <generated-base64url-value>
mcp:
  stdio_enabled: false
```

The related environment variables are:

```text
NOEMA_WEB__DEV_NO_AUTH=true
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

Configuration writes must use a stable cross-process lock and atomic
replacement. They must change only the file-backed recovery field. They must
not serialize values resolved from environment variables.

## 4. Required Authentication Mode

`web.dev_no_auth` defaults to `false`.

When it is false, every application request requires one of these credentials:

- An authenticated browser session
- A valid native access token
- A setup-only session for one permitted setup action

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
- The UI shows a persistent authentication-disabled warning.
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

Each passkey record must have:

- Its stable credential identifier
- A human-visible name
- Creation time
- Last-use time
- Its independent WebAuthn credential state

Authentication must update only the credential that completed the ceremony.
Registration must exclude credentials already stored for the human.
Normal passkey addition and removal require recent passkey authentication.
Development bypass and recovery enrollment are explicit exceptions.

Recent authentication means passkey success in the same session and
authentication epoch during the previous five minutes.

Normal web settings cannot delete the final passkey. The recovery flow can
restore access when no usable passkey remains.
The UI should recommend another passkey when only one exists. It must not block
the human from continuing.

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

Startup must use a stable lock outside `config.yaml`. It must reject a symlink
or non-regular config file and insecure file permissions.

Each write must use a private temporary file in the same directory. Noema must
synchronize that file, replace the config atomically, and synchronize its
parent directory.

The write must preserve unrelated file values and mode `0600`. A structural
YAML rewrite can normalize comments and formatting.

The plaintext field uses a typed secret with redacted debug output. It must not
enter the ordinary resolved `WebConfig` value.

The human reads the current code directly from `config.yaml`. Noema must not
print it to ordinary daemon output.

### 7.2 Attempt Boundary

Recovery uses `POST /auth/recovery` with a bounded body. It never accepts the
code in a URL, GraphQL operation, or browser storage.

Host, Origin, method, body, and rate checks run before code validation. A
request becomes an attempt only when its candidate reaches comparison.

After admission, an empty, malformed, or incorrect candidate still rotates the
code. Envelope rejection and rate rejection do not rotate it.

Origin validation always applies, including when a request has an Authorization
header. Proxies, browsers, and service workers must not retry this route.

Every admitted candidate triggers replacement. Every completed attempt consumes
the current code, whether the candidate is correct or incorrect.

The server must process one recovery attempt at a time:

1. Acquire the stable recovery lock.
2. Re-read and capture the current file-backed code.
3. Compare the candidate in constant time.
4. Generate a new 32-byte recovery code.
5. Save and verify the replacement atomically.
6. Treat the committed file generation as current.
7. Release the lock.
8. Return the result of the captured comparison.

Atomic file replacement is the commit point. Before it, the old code remains
current. After it, the replacement is current.

Noema must not return a candidate-match result before directory synchronization.
A storage failure returns an operational error and creates no authority.

If a failure occurs before replacement, Noema denies recovery and retains the
old code. If replacement might have occurred, Noema denies recovery and
re-reads the file before another attempt.

If that re-read fails, Noema disables recovery until restart. It must never
continue with cached or memory-only recovery authority.

Concurrent attempts run in lock order. Each committed attempt consumes the code
current when its comparison starts.

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

The server records code attempts and successful recovery as security events.
Those events contain no candidate, code, hash, or derived code fragment.
Their retention or aggregation must prevent persistent storage exhaustion.

### 7.4 Availability Tradeoff

An unauthenticated actor can submit invalid candidates and rotate the code.
This action cannot grant access, but it can delay legitimate recovery.

Global and source rate limits reduce this denial risk. Requests rejected before
comparison do not rotate the code.

This availability tradeoff is accepted. The operator can always read the newest
code from the local startup configuration.

When practical, the public edge should restrict the recovery route to an
operator network such as Tailscale. Normal login can remain public.

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

Each session records its authenticating passkey and authentication epoch.
Removing a passkey revokes its sessions and pending ceremonies.

Passkey changes, native grants, and equivalent sensitive actions require recent
passkey authentication.

Private HTML, GraphQL, artifact, authentication, and OAuth responses use
`Cache-Control: no-store`. Immutable public assets can use content-hash caching.

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

Noema does not add selectable OAuth scopes. It may use one fixed protocol scope
that represents the complete local-human authority.

Native authorization uses Authorization Code with PKCE:

- The system browser performs authorization.
- `S256` is required.
- Missing, `plain`, and unknown PKCE methods fail.
- Each request uses fresh PKCE verifier and `state` values.
- The client verifies returned `state` before exchanging the code.
- The browser requires recent passkey authentication.
- Browser approval actions use Origin and CSRF protection.
- Codes are opaque, short-lived, single-use, and atomically consumed.
- Codes bind the human, client, redirect, authority, and PKCE challenge.

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
- Refresh cannot change the human, client, grant, authority, or audience.

Authorization codes expire after five minutes. Access tokens expire after ten
minutes. Refresh families expire after 30 idle days or 180 absolute days.

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

Noema must use a maintained Rust OAuth server library for protocol handling. It
must not implement OAuth from scratch. `oxide-auth` is the first proof-of-fit
candidate, but its in-memory and signed issuers are not production authorities.
Noema retains redirect, approval, CSRF, code-use, replay, and revocation policy.
Composition tests must enforce the complete contract across library boundaries.

The existing Rust `oauth2` crate can support the desktop client. iOS uses the
platform authentication session and networking APIs.

Noema does not add OIDC, ID tokens, UserInfo, JWKS, or OIDC discovery.

## 10. GraphiQL

`web.graphiql` defaults to `false`.

When disabled, Noema does not register GraphiQL or the schema-download route.
Production GraphQL introspection also rejects requests. Disabled routes return
`404`.

When enabled, GraphiQL, schema download, and introspection require a browser
session with recent passkey authentication. Its assets are pinned and served
by Noema.

GraphiQL must not execute mutable third-party scripts inside Noema's origin.
Its route uses a dedicated CSP without inline or third-party scripts.

## 11. MCP Stdio Process Authority

`mcp.stdio_enabled` defaults to `false`.

The stdio session factory must enforce the flag before constructing or starting
a process. This one check covers setup, continuation, existing rows, and tool
invocation.

When disabled:

- Every stdio launch fails closed.
- Existing stdio definitions remain stored but unavailable.
- Hosted HTTP MCP remains available.
- The UI hides or disables local-command setup.

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

When both values are true, startup must warn that every reachable client can
execute commands with the Noema service account.

Noema permits the combination. The operator owns its external access controls.

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

The proxy must preserve the configured public Host. It must not forward an
untrusted client value through a header that Noema treats as authoritative.

Production errors are bounded and stable. They do not expose stack traces,
database details, internal paths, or secret values.

Enable HSTS only after the public domain and every covered subdomain are ready.

## 13. PWA Offline Boundary

Installed PWA data remains protected by the browser profile and device lock.
Server logout cannot erase data from an offline device.

The server has one durable installation identifier. It changes only when the
operator creates a new instance or restores data as a new instance.

The server also has a monotonic human authentication epoch. Global browser
logout and every passkey removal increment it.

Each offline snapshot stores both values after authenticated reconciliation.
An offline client can trust only its last successful comparison.

After network return, the PWA checks both values before showing private data.
A mismatch deletes the old snapshot, drafts, sentinels, push state, and private
caches before new reconciliation.

Development bypass disables private snapshot writes. It also clears or
distrusts any existing authenticated offline sentinel.

Installed mode provides one `Log out and erase this device` action. When online,
it revokes the server session and push registration before local deletion.

When offline, it deletes local data immediately. It then reports that remote
revocation was incomplete and recommends global logout from another client.

The detailed release and cache contract remains in
[Installed Web Application](frontend/pwa.md). GraphQL, WebSocket, authentication,
OAuth, recovery, artifacts, administration, and external origins remain
network-only. The worker never retries or navigation-falls-back to those paths.

## 14. Remaining Security Work Before Public Access

Feature work blocks public access only when its feature is enabled. Otherwise,
Noema must disable the feature and its routes.

### Authentication And Sessions

- Enforce the setup barrier across every browser, native, GraphQL, WebSocket,
  artifact, OAuth, and GraphiQL route.
- Remove the printed bootstrap URL and process-local setup authority.
- Implement runtime `dev_no_auth`, default it to false, and preserve every other
  security boundary.
- Implement secure recovery generation, locked config updates, atomic rotation,
  and restricted setup sessions.
- Apply recovery body, Host, Origin, rate, and concurrency checks before
  comparison. Never record recovery candidates.
- Use maintained WebAuthn code with exact RP ID, origin, user verification, and
  expiring single-use challenges.
- Bound sessions and ceremonies. Add expiry, identifier rotation, revocation,
  and eviction resistance.
- Bind sessions to passkeys and the authentication epoch. Terminate affected
  WebSockets after logout, expiry, removal, or revocation.

### Native Clients

- If native access launches, replace legacy pairing with system-browser
  Authorization Code, verified `state`, S256 PKCE, and exact redirects.
- Use a maintained Rust OAuth library. Disable every legacy native authorization
  route during cutover.
- Issue revocable access tokens and rotating refresh families. Detect reuse and
  revoke every legacy credential.
- Store refresh credentials in protected operating-system storage. Keep bearer
  credentials outside webviews.

### API, PWA, And Notifications

- Bound HTTP, GraphQL, and WebSocket bodies, cost, execution time, connections,
  operations, subscriptions, and concurrency.
- Keep authentication and private runtime routes outside service-worker
  handling. Apply `no-store` to every private response.
- Disable private offline snapshots until logout-and-erase, instance binding,
  and authentication-epoch checks exist.
- If Push launches, validate current authorization before each delivery.
  Disable private notification previews by default.
- Provide accessible logout and global revocation. Terminate affected
  WebSockets and Push registrations.

### Debug And Process Boundaries

- Disable GraphiQL, schema download, and introspection by default. Require recent
  authentication and local assets when enabled.
- Enforce the stdio MCP flag before process construction. When enabled, use
  exact arguments, no shell, and an environment allowlist.
- Run Noema with an unprivileged service account, private file modes, and no
  capabilities. Keep binaries and units root-owned.

### Public Edge And Validation

- Bind Noema to loopback behind HTTPS. Enforce canonical Host and trust
  forwarding headers only from the proxy.
- Enforce cookie, Origin, CORS, cache, CSP, and browser security-header rules.
- Keep access logs, callback logs, errors, and diagnostics bounded and free of
  secrets.
- Triage dependency advisories. Block launch only for reachable vulnerabilities
  within the stated threat model.
- Test setup bypasses, recovery failures, concurrent rotation, session
  revocation, OAuth replay, and WebSocket termination.

Before public routing, verify:

- `web.dev_no_auth` is false and at least one passkey exists.
- The public origin uses HTTPS and matches the RP ID.
- GraphiQL and stdio MCP remain disabled unless explicitly reviewed.
- The listener is loopback-only. Host enforcement and resource limits are active.
