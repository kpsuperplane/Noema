# Provider Auth Onboarding Design

## Goal

Noema should support first-run onboarding through a provider-generic authentication layer. The first concrete provider is Codex, but the design must keep provider accounts, auth methods, onboarding checks, and chat runtime usage generic enough to add OpenAI Platform API keys, additional Codex subscriptions, local providers, or other model providers later.

The immediate user-facing outcome is:

- `docker compose up dev` includes a usable Codex CLI.
- The web app checks backend onboarding status before opening chat.
- If onboarding is incomplete, the first screen shows onboarding instead of chat.
- V1 onboarding contains one required step: connect the active provider account.
- The web UI can initiate Codex device-code login.
- Provider credential state persists as long as the provider allows.

## Context

Noema currently uses a provider-neutral generation contract, with Codex implemented through a daemon-owned `codex app-server --listen stdio://` subprocess. Docker development now starts Postgres, the daemon watcher, and the web asset watcher, but the dev image does not yet include the Codex CLI. As a result, the daemon can reach chat startup and then fail with:

```text
codex provider is unavailable: codex command not found
```

Codex supports ChatGPT sign-in, API-key sign-in, access-token sign-in, and device-code sign-in. For headless or containerized environments, device-code login is the best first web-initiated flow because it avoids localhost callback forwarding while still letting the user complete the auth ceremony in a browser.

Codex stores cached credentials in `CODEX_HOME`, either as a file such as `auth.json` or through a configured credential store. Noema should treat those files as provider-owned secret material.

## Definitions

### Provider

A model backend family, such as `codex`, `openai`, or a future local provider.

### Provider Account

A server-scoped configured account for a provider. V1 creates a single active account:

```text
provider_account:codex:default
```

The model should allow future accounts such as `codex:work`, `codex:personal`, or `openai:platform-main` without rewriting chat runtime code.

### Credential Home

Provider-owned secret/session state on disk:

```text
NOEMA_HOME/providers/<provider>/<account>/
```

For Codex V1:

```text
NOEMA_HOME/providers/codex/default/
```

Noema runs Codex with `CODEX_HOME` set to this account directory. Credential files and tokens must not be stored in Postgres or exposed through frontend protocols.

### Auth Attempt

A short-lived login ceremony intended to create or refresh provider credentials. For Codex device-code login, an auth attempt starts `codex login --device-auth`, exposes safe instructions to the UI, and waits for completion.

Auth attempts are not credentials. They are temporary attempts to obtain credentials. Users are expected to complete a device-code flow in roughly five minutes. If an attempt expires, is cancelled, or is interrupted, the user starts a new attempt.

### Onboarding Status

A derived backend result that tells the frontend whether Noema can enter the primary chat experience. V1 derives this from provider account readiness instead of storing a stale "completed" flag.

## Architecture

Noema adds four focused units.

### Provider Account Registry

The registry owns non-secret provider account metadata. It can ensure that the default active account exists, resolve the active account for chat, and expose safe account summaries to the web UI.

It does not read, parse, store, or return provider credential files.

### Provider Auth Adapter

Provider-specific auth behavior lives behind a generic interface. The first adapter is Codex.

The interface should support these conceptual operations:

- `status(account)`: determine whether the account is usable.
- `start_auth_attempt(account, method)`: begin a short-lived auth ceremony.
- `poll_auth_attempt(attempt_id)`: return safe progress for the UI.
- `cancel_auth_attempt(attempt_id)`: stop the live process if possible.
- later `logout(account)`: remove or invalidate credentials.

Codex V1 implements:

```text
method = oauth_device_code
command = codex login --device-auth
env CODEX_HOME = account credential home
```

### Onboarding Service

The onboarding service computes the current first-run gate.

V1 required check:

```text
active provider account exists and is authenticated/usable
```

If the check fails, onboarding returns a blocking step describing the active provider account and preferred auth method. The frontend does not decide onboarding by directly poking provider-specific details.

### Chat Runtime Provider Resolution

The chat runtime asks the provider account registry for the active authenticated account before starting provider work. For Codex, the runtime config uses the resolved account credential home as `CODEX_HOME`.

If provider auth becomes invalid during chat, the provider status is refreshed and the frontend can return to onboarding.

## Data Model

### `provider_accounts`

Postgres stores non-secret metadata:

- `provider_account_id`
- `provider_kind`
- `account_key`
- `display_name`
- `auth_method`
- `is_active`
- `is_default`
- `status`
- `last_checked_at`
- `last_authenticated_at`
- `last_error_code`
- `last_error_message`
- `metadata`
- `created_at`
- `updated_at`

The initial account status values are:

- `unknown`
- `checking`
- `authenticated`
- `unauthenticated`
- `unavailable`

The V1 bootstrap creates or ensures:

```text
provider_kind = codex
account_key = default
display_name = Codex
auth_method = oauth_device_code
is_active = true
is_default = true
```

### `provider_auth_events`

This table is optional for V1 implementation, but the design reserves it for safe audit events:

- auth attempt started
- device-code instructions shown
- auth completed
- auth failed
- auth cancelled
- auth expired

Events must not include tokens, credential file contents, raw unredacted CLI output, or pasted secrets.

### Runtime Auth Attempts

V1 auth attempts can live primarily in memory because they are short-lived. The UI can start a new attempt after refresh, expiry, cancellation, or daemon restart.

The attempt state returned to the UI should include only safe fields:

- `attempt_id`
- `provider_kind`
- `provider_account_id`
- `method`
- `status`
- `verification_url`
- `user_code`
- `instructions`
- `expires_at`
- `last_status_message`
- `error_code`
- `error_message`

The initial auth attempt status values are:

- `starting`
- `waiting_for_user`
- `completed`
- `failed`
- `expired`
- `cancelled`

`expires_at` is optional because not every provider or CLI output exposes an exact expiry time.

If the daemon restarts during a login attempt, existing credentials may still be valid if Codex completed the login. The next onboarding status check should detect that. Otherwise the UI shows retry.

## Filesystem Layout

Provider credential homes live under `NOEMA_HOME`:

```text
NOEMA_HOME/
  providers/
    codex/
      default/
        config.toml
        auth.json
        ...
```

For container portability, the Codex account home uses file credential storage. Noema writes account-local Codex config:

```toml
cli_auth_credentials_store = "file"
```

The `providers/` directory is sensitive because it can contain tokens. It should remain inside the user-owned Noema home and should be excluded from source control by normal `NOEMA_HOME` placement.

## HTTP API

### `GET /api/onboarding/status`

Returns whether the current user/server is onboarded.

Example incomplete response:

```json
{
  "is_user_onboarded": false,
  "steps": [
    {
      "id": "connect_provider_account",
      "status": "blocked",
      "provider_kind": "codex",
      "provider_account_id": "provider_account:codex:default",
      "account_key": "default",
      "display_name": "Codex",
      "auth_method": "oauth_device_code"
    }
  ]
}
```

Example complete response:

```json
{
  "is_user_onboarded": true,
  "steps": [
    {
      "id": "connect_provider_account",
      "status": "complete",
      "provider_kind": "codex",
      "provider_account_id": "provider_account:codex:default"
    }
  ]
}
```

### `GET /api/provider-accounts`

Returns safe provider account summaries.

### `POST /api/provider-accounts/default`

Ensures the default provider account exists. The daemon may also do this during bootstrap so the frontend usually does not need to call it directly.

### `POST /api/provider-auth/attempts`

Starts a short-lived auth attempt.

Request:

```json
{
  "provider_kind": "codex",
  "provider_account_id": "provider_account:codex:default",
  "method": "oauth_device_code"
}
```

Response:

```json
{
  "attempt_id": "provider_auth_attempt_123",
  "status": "starting"
}
```

### `GET /api/provider-auth/attempts/<id>`

Polls an auth attempt. For Codex device-code login, the response includes safe instructions when available:

```json
{
  "attempt_id": "provider_auth_attempt_123",
  "status": "waiting_for_user",
  "verification_url": "https://...",
  "user_code": "ABCD-EFGH",
  "instructions": "Open the link and enter the code.",
  "expires_at": "2026-06-26T12:05:00Z"
}
```

Terminal fallback can be included as a safe command template, using the same credential home.

### `POST /api/provider-auth/attempts/<id>/cancel`

Cancels the live auth process if it is still running and marks the attempt cancelled.

## Frontend Flow

The first screen is onboarding-aware.

1. Load `/api/onboarding/status`.
2. If `is_user_onboarded` is true, open `/api/chat/ws` and start the primary conversation.
3. If false, render onboarding in the main content area and do not open the chat WebSocket.
4. Show the provider connection step.
5. User clicks "Connect Codex."
6. Start an auth attempt and poll until complete, expired, failed, or cancelled.
7. When complete, refresh onboarding status.
8. If onboarding is now complete, enter chat.

The top status cluster can continue showing local service and memory status, and should add provider status when available.

## UI States

### Needs Provider Login

Show:

- provider name
- account display name
- current status
- primary "Connect Codex" action
- secondary terminal fallback

### Waiting For User

Show:

- verification URL
- device code
- concise instructions
- time remaining when known
- cancel action
- retry action only after expiry/failure

### Completed

Show brief success state, refresh onboarding, then transition to chat.

### Failed Or Expired

Show the safe error message and a retry action.

### Command Not Found

Show that the Codex CLI is unavailable in the current environment. Docker/dev should prevent this by installing the CLI in the image. Host-side docs should tell users how to install Codex when running outside Docker.

## Docker And Development

The dev image should include:

- Rust
- Bun
- cargo-watch
- Codex CLI

Compose should persist `NOEMA_HOME` so provider credential homes survive container restarts. Noema should not require a host-level Codex install for `docker compose up dev`.

Documentation should include:

- preferred web onboarding flow
- terminal fallback for the same account home
- explanation that provider credential material lives under `NOEMA_HOME/providers/...`
- warning not to commit or share provider credential files

## Error Handling

### Auth Attempt Expired

The UI offers retry. Expiry is normal for device-code auth.

### Auth Attempt Cancelled

The UI returns to the connect-provider state.

### Device Auth Unsupported

Show a fallback path. In V1 this can be a terminal fallback. Later it can expose access-token or API-key auth methods.

### Daemon Restart During Auth

The live auth process is gone. The UI starts from onboarding status again. If credentials were written, onboarding completes. If not, the user starts another attempt.

### Provider Auth Fails During Chat

Refresh provider status and return to onboarding if the active provider account is no longer usable.

## Security

- Do not store tokens in Postgres.
- Do not return raw credential file paths or credential file contents through the API.
- Do not log raw tokens, raw secrets, auth cache contents, or unredacted CLI output.
- Redact auth process output before storing or returning it.
- Keep secret-entry auth methods in the generic protocol shape, but do not enable web-pasted secrets until masking, logging, transport, validation, and storage behavior are explicitly implemented.
- Treat provider credentials as server-admin managed state in V1.

## Validation

Unit tests:

- onboarding status derivation
- default provider account bootstrap
- provider account metadata repository behavior
- Codex credential home path resolution
- auth attempt state transitions
- auth output redaction

Adapter tests with a fake Codex command:

- emits device-code instructions
- completes successfully
- fails
- expires
- cancels
- reports command not found

Frontend/protocol tests:

- generated TypeScript is current
- onboarding incomplete blocks chat startup
- onboarding complete allows chat startup
- provider login panel renders the active blocking step

Docker smoke:

- `docker compose up dev` starts without `codex command not found`
- unauthenticated first page shows onboarding
- after a valid provider credential home exists, first page enters chat

## Non-Goals

- Multiple active provider accounts.
- Full provider account management screens.
- Web-pasted API keys or access tokens.
- OS keychain integration.
- Full logout or revoke UX.
- Production remote-access hardening beyond the current local web assumptions.
- Expanding the Codex app-server protocol beyond current chat behavior.
- Backwards-compatible migrations for pre-V1 development databases.

## Open Follow-Ups

- Decide when to add account switching and routing rules.
- Decide when to support secret-entry auth methods.
- Decide whether provider auth events become required in V1 or wait for the account-management surface.
- Decide how provider status should be reflected in future settings and inspection views.
