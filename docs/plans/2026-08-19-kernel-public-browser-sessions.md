# Kernel Public Browser Sessions

- **Status:** Implemented in Go on 2026-09-05
- **Mode:** Reference record
- **Date:** 2026-08-19
- **Provider id:** `kernel`
- **Initial scope:** Public interactive browser sessions
- **Default:** Obscura remains the default browser provider

This plan adds Kernel as an explicit browser provider. It does not change the
model-visible browser tools. It does not replace Obscura.

`internal/webtool/kernel_browser.go` and `docs/harness/web-browsing.md` are the
current authorities. Rust paths below record the original design only.

## 1. Outcome

A human can add a Kernel account with an API key. The human can assign that
account to `web.browse`. A conversation or Task can then use a Kernel browser
session for public pages.

The session supports the existing browser commands:

- open a public URL;
- read a bounded snapshot;
- interact with one snapshot element;
- wait for text or an element;
- move through history or reload;
- close the session.

The result keeps the current `BrowseResponse` shape. It reports `kernel` as
the provider. Obscura remains the default when no browser assignment exists.

## 2. Implementation brief

### Non-goals

- Do not add authenticated profiles, saved cookies, or Managed Auth.
- Do not add browser pools.
- Do not add file transfer, Live View, video replay, or MCP support.
- Do not change the model-visible browser tool schemas.
- Do not change the Obscura default or its worker model.
- Do not add a new generic browser-provider framework.
- Do not add private, local, or unrestricted network access.
- Do not redesign the Settings surface.

### Existing path to reuse

Reuse the provider-neutral browser facade in
`crates/noema-providers/src/web/browse.rs`.

Reuse the Obscura command, snapshot, URL-policy, revision, timeout, screenshot,
capacity, and idle-expiry behavior as the contract for Kernel.

Reuse the provider account catalog, secret-input store, capability assignment,
credential revision, and authentication-failure paths.

Reuse the existing host resolver. Add Kernel resolution to that authority.

### Expected files

Production changes are expected in:

- `crates/noema-providers/src/web/browse.rs`
- `crates/noema-providers/src/web/mod.rs`
- `crates/noema-providers/src/adapters/web/browse/mod.rs`
- new `crates/noema-providers/src/adapters/web/browse/kernel.rs`
- `crates/noema-providers/src/accounts.rs`
- `crates/noema-providers/src/capabilities.rs`
- `crates/noema-providers/src/adapters/account_service/credentials.rs`
- `crates/noema-providers/src/adapters/account_service/helpers.rs`
- `crates/noema-store/src/provider_accounts.rs`
- `crates/noema-store/src/schema.rs`
- `crates/noema-host/src/composition.rs`
- `crates/noema-runtime/src/daemon/runtime/local_tools.rs`
- `crates/noema-runtime/src/daemon/runtime/local_tools/web_actions.rs`

Tests are expected in the closest existing provider, account, runtime, and
schema test modules. No frontend source change is expected.

### Code budget

- Production net: 500–700 Rust lines.
- Test net: 150–250 Rust lines.
- New Rust tests: 6–8.
- Stop if production or test net exceeds its estimate by 50% or 500 lines,
  whichever limit is smaller.

### Stop conditions

Stop and request a scope decision if any condition occurs:

- Kernel cannot enforce the public URL policy at the required boundary.
- Kernel execution requires sending model-provided JavaScript.
- The API contract requires a new persistent browser state authority.
- The provider cache cannot preserve sessions across resolver calls safely.
- The migration cannot preserve existing provider accounts and schema shape.
- The patch needs a second browser abstraction or a broad runtime redesign.

## 3. Provider contract

Use Kernel’s HTTPS API directly. Do not route this path through Kernel’s MCP
server. MCP exposes a separate tool surface and would bypass Noema’s provider
assignment and browser policy.

The initial client uses the documented API host:

`https://api.onkernel.com`

Use a bearer API key from the protected provider credential store. Never place
the key in model context, ordinary events, logs, or error text.

### Browser lifecycle

Create one remote browser when an owner has no active session:

```text
POST /browsers
Authorization: Bearer <protected Kernel API key>
```

Use a headless Chromium session. Set its remote timeout to 30 minutes.
Use the existing Noema session limit and session owner identity.

Do not pass a profile, cookie jar, start URL, or authentication state.

Drive the browser through fixed Playwright code sent to:

```text
POST /browsers/{session_id}/playwright/execute
Authorization: Bearer <protected Kernel API key>
```

Set the remote execution timeout to the existing 30-second browser command
deadline. Use only generated scripts for navigation, snapshots, interaction,
wait, history, and screenshots.

Delete the remote browser on explicit close:

```text
DELETE /browsers/{session_id}
Authorization: Bearer <protected Kernel API key>
```

Treat deletion as best effort after local session removal. The remote timeout
remains the final cleanup boundary.

References:

- [Kernel browser creation](https://www.kernel.sh/docs/introduction/create)
- [Kernel Playwright execution](https://www.kernel.sh/docs/browsers/playwright-execution)
- [Kernel browser deletion](https://www.kernel.sh/docs/api-reference/browsers/delete-a-browser-session-by-id)
- [Kernel MCP server](https://www.kernel.sh/docs/reference/mcp-server)

## 4. Security and data handling

### URL policy

Apply the same public URL policy as Obscura.

1. Validate the requested URL before browser creation or navigation.
2. Validate the resulting page URL after navigation.
3. Validate the resulting page URL after interaction, wait, and history.
4. Reject private, local, unsupported, and rebinding targets.
5. Never treat page text, attributes, or links as trusted instructions.

The implementation must prove that remote page navigation cannot bypass the
Noema public-target policy. Use Kernel request interception when the API allows
it. At minimum, combine generated request guards with Noema’s host-side URL
and DNS checks. Add a focused hostile-target test.

If Kernel cannot provide an equivalent network boundary, do not ship this
provider. A post-navigation check alone is not sufficient for this scope.

### Script safety

Kernel’s Playwright endpoint can execute arbitrary code. Noema must not expose
that ability to the model.

- Keep every script as a Rust constant or a bounded generated fragment.
- Encode URLs, references, text, and values as JSON string literals.
- Do not concatenate raw model text into JavaScript.
- Do not return stdout, stderr, raw provider errors, or generated code.
- Bound every parsed result before building `BrowseResponse`.

### Credential failures

Map Kernel HTTP 401 and 403 responses to a safe browser authentication error.
Use the existing credential-revision fence to mark the account unauthenticated.

Keep normal session errors separate from credential errors. Do not silently
retry a failed remote action with a new browser session.

## 5. Provider account and capability changes

Add this catalog entry in `provider_account_catalog()`:

| Field | Value |
| --- | --- |
| Provider kind | `kernel` |
| Display name | `Kernel` |
| Preferred auth method | `SecretInput` |
| Supported auth methods | `SecretInput` |
| Capability | `web.browse` |

Create Kernel accounts as user-managed accounts with generated account keys.
Do not create a system Kernel account. Do not make Kernel the default.

Allow `kernel` in these existing credential and account paths:

- secret-input account validation;
- protected API-key reads;
- secret save and clear operations;
- account reconciliation;
- fenced authentication-failure recording;
- account deletion when no capability binding references the account.

Declare Kernel’s browser capability as an external hosted provider. Derive
availability from the account status. Keep the existing `web.browse` feature
contract. Do not interpret that contract as persisted authenticated profiles.

The existing Settings path reads the provider catalog dynamically. Verify that
Kernel appears through that path. Do not add a provider-specific Settings card.

## 6. Browser backend design

### Facade

Add `KERNEL_BROWSER_PROVIDER_ID` beside the Obscura provider id.

Add a Kernel variant to the existing private backend enum. Add one constructor
that receives the protected API key and the configured maximum session count.

Keep `WebBrowseBackendHandle` as the only runtime-facing browser handle.
Do not expose Kernel request or session types outside the adapter boundary.

### Session state

Mirror the Obscura lifecycle with remote session state:

- owner key;
- Kernel session id;
- local snapshot revision;
- idle deadline;
- capacity permit;
- per-session command serialization.

Use a background expiry task. It must release local capacity and attempt remote
deletion when a session reaches its idle deadline.

Remove a session after an unavailable worker, blocked target, or uncertain
outcome. Mark interaction and history failures as outcome-uncertain when the
remote command may have changed page state.

Cache the constructed Kernel backend by provider account id and credential
revision in the host resolver. A new backend for every browser command would
lose the remote session. A credential revision change must select a new backend.

The old remote session may remain until its Kernel timeout after key rotation.
Record this bounded cleanup behavior. Do not add credential migration.

### Command mapping

| Noema command | Generated Kernel operation | Required result |
| --- | --- | --- |
| `Open` | `page.goto` with the requested wait mode | Validate URL, settle, return snapshot and screenshot |
| `Snapshot` | DOM extraction and screenshot | Increment revision and return bounded content |
| `Interact` | Ref lookup followed by click, fill, type, key press, or select | Enforce revision, validate URL, return fresh snapshot |
| `Wait` | Bounded text or ref wait | Enforce timeout, validate URL, return fresh snapshot |
| `History` | Back, forward, or reload | Enforce revision, validate URL, return fresh snapshot |
| `Close` | Local removal and remote DELETE | Return `state: closed` |

Use the Obscura snapshot rules:

- assign stable `data-noema-ref` values to interactive elements;
- return role, name, href, and disabled state;
- cap elements at 200;
- cap text at the requested limit and 20,000 characters;
- cap names and URLs to the existing contract;
- include a PNG screenshot only within the existing 900,000-byte limit;
- hide the screenshot from model-visible output through the current runtime path.

Parse only the fields needed for `BrowseResponse`. Treat missing, malformed,
failed, or oversized provider results as safe provider errors.

If `WebBrowseError` gains an authentication variant, update the existing
Obscura worker error codec with a stable token. Do not change Obscura behavior.

## 7. Host and runtime integration

### Host resolver

Extend `HostWebBackendResolver` in
`crates/noema-host/src/composition.rs`.

When the request provider kind is `kernel`:

1. Read `credentials.api_key("kernel", account_id)`.
2. Convert the protected credential to the backend constructor input.
3. Resolve or create the cached handle for `(account_id, credential_revision)`.
4. Return the cached handle.

Return `Unauthenticated` when the protected credential is missing. Return
`Unavailable` when construction fails.

Keep `obscura` mapped to the existing default handle. Keep all search and fetch
resolver behavior unchanged.

### Runtime selection

Use the existing stored `web.browse` binding and capability availability checks.
When the binding is absent, continue to select Obscura.

When a bound Kernel account is unavailable or unauthenticated, preserve the
existing browser fallback behavior to Obscura and its safe fallback metadata.

When a Kernel command returns an authentication error, record the failure with
the resolved account id and credential revision. Do not record a failure for a
navigation, snapshot, timeout, or page-content error.

Keep browser owner keys scoped to the current conversation or Task generation.
Do not share a Kernel session across owners.

## 8. Schema migration

Advance `STORE_SCHEMA_VERSION` from 57 to 58.

Append one immutable migration after the current final migration. Do not edit
`LEGACY_V9_SCHEMA_SQL`, `SYSTEM_PROVIDER_ACCOUNTS_SQL`, or any earlier
migration.

The migration must expand the `provider_accounts.provider_kind` check to include
`kernel`. Preserve every existing row and every current column. Do not insert a
system Kernel account.

Prefer the existing table-rebuild pattern used for closed SQLite checks. The
new table definition must retain the current provider kinds, auth methods,
status checks, defaults, timestamps, and uniqueness constraint.

Add a schema test that proves both paths:

1. Build a database at version 57, apply version 58, and insert a Kernel
   account.
2. Build a fresh database at the latest version and insert the same account.
3. Confirm the upgraded and fresh schema objects match.
4. Confirm existing provider accounts survive the upgrade.
5. Confirm the Kernel provider kind is accepted after migration 58.

## 9. Ordered implementation units

### Unit 1: Provider account contract

Change the provider catalog, capability declaration, store account allowlist,
credential allowlist, and secret-input helpers. Add focused account tests.

Commit this unit after focused validation.

### Unit 2: Schema migration

Add migration 58 and the upgrade/fresh-convergence test. Confirm no earlier
migration changed.

Commit this unit after focused validation.

### Unit 3: Kernel backend

Add the facade variant and concrete HTTP backend. Implement lifecycle, fixed
scripts, URL checks, snapshots, screenshots, revisions, limits, expiry, and
safe errors. Use a local HTTP server, never the live Kernel service.

Commit this unit after focused provider validation.

### Unit 4: Host and runtime selection

Add credential-backed resolution and revision-keyed caching. Wire browser
authentication failures to the existing fenced status path. Verify the
Obscura default and explicit Kernel assignment.

Commit this unit after focused host and runtime validation.

### Unit 5: Review and repository validation

Run the deletion pass and one read-only adversarial review. Then run repository
validation. Stage only planned files.

## 10. Tests and unique risks

| Test | Layer | Unique risk |
| --- | --- | --- |
| Kernel catalog and capability status | Providers | Kernel is visible with the right account method and availability state |
| Kernel secret read, clear, and auth-failure fence | Account service | API keys stay protected and stale failures cannot change newer credentials |
| Version 57 upgrade and fresh schema convergence | Store | Existing databases and new databases accept the same Kernel provider kind |
| Kernel create, execute, parse, and delete wire flow | Provider adapter | Endpoint paths, bearer auth, bounded request fields, and response parsing drift |
| Public-target rejection and redirect validation | Provider adapter | Remote navigation bypasses the Noema URL and DNS policy |
| Snapshot, revision, interaction, wait, history, and screenshot limits | Provider adapter | Kernel behavior diverges from the existing browser contract |
| Session reuse, capacity, expiry, and close | Provider adapter | Resolver calls lose sessions or remote sessions leak capacity |
| Explicit Kernel binding and Obscura default | Runtime | Provider selection changes default behavior or crosses owner boundaries |

Do not add live integration tests. A live test would require a user secret and
would make unit validation depend on external service state.

## 11. Acceptance scenarios

The implementation is complete when these scenarios pass:

1. Kernel appears in the generic account catalog.
2. A saved API key stays outside account metadata and model context.
3. An assigned Kernel account opens a public page.
4. Later commands reuse the same remote session.
5. Stale snapshots and private redirects fail safely.
6. Page content cannot execute arbitrary Noema or JavaScript commands.
7. Close and expiry release local capacity and clean up remote sessions.
8. Kernel authentication failures update only the matching credential revision.
9. An unassigned browser still uses Obscura.
10. A version 57 database upgrades without losing provider accounts.

## 12. Validation

Run focused validation after each unit:

```text
cargo validate test -p noema-providers --features adapters --lib
cargo validate test -p noema-store --lib schema
cargo validate test -p noema-runtime --lib local_tools
```

Before each commit, run:

```text
git diff --check
bun run scripts/report-rust-size.ts --base <unit-base>
```

Before the final commit, run the repository defaults:

```text
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
```

Use unit-base Git references for the size report. Apply the production and test
budgets from this plan. Run unit tests only.

## 13. Rollback point

The feature can be disabled by removing Kernel capability assignments and
leaving Obscura as the default. The migration remains forward-only.

Do not remove the `kernel` schema value after migration 58 ships. If the
backend is not ready, leave the account type unavailable and add a later
forward migration only when a correction is required.

## 14. Deferred work

Consider these only after public sessions operate reliably:

- Managed Auth or persisted browser profiles;
- authenticated session grants and secret input policy;
- Browser pools;
- live human view and replay links;
- file transfer;
- reconnect support for long-lived CDP or WebDriver connections;
- provider health checks and live usage metrics;
- a live Kernel integration test in a separately authorized test environment.
