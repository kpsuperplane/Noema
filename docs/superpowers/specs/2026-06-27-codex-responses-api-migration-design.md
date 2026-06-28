# Codex Responses API Migration Design

## Goal

Move Noema's primary Codex chat runtime from an embedded `codex app-server`
subprocess to a Noema-owned direct Codex Responses API integration.

The target behavior is:

- Noema authenticates Codex accounts through its own device-code OAuth flow.
- Noema stores Codex OAuth tokens under `NOEMA_HOME`.
- Normal chat and memory extraction call
  `https://chatgpt.com/backend-api/codex/responses` directly.
- The Codex CLI is no longer required for normal daemon chat.
- `NOEMA_HOME/providers/codex/default/` is a Noema credential home, not a full
  `CODEX_HOME` with Codex CLI plugins, skills, sessions, and runtime state.

This is a pre-V1 schema/runtime change. Backwards compatibility with existing
Codex CLI-created account homes is not required.

## Context

Noema currently models Codex after the Codex CLI runtime. Authentication starts
`codex login --device-auth` with `CODEX_HOME` pointed at
`NOEMA_HOME/providers/codex/default/`, and chat starts a long-lived
`codex app-server --listen stdio://` subprocess.

That works, but it makes the provider account directory a complete Codex home.
Codex then installs or caches normal Codex home state there, including plugins,
skills, sessions, logs, sqlite state, and shell snapshots. This is surprising
for Noema because provider homes are meant to hold provider credentials and
portable account state, not an embedded agent runtime.

Hermes Agent's default OpenAI Codex integration does not use app-server. It uses
a Codex OAuth-backed Responses API provider pointed at
`https://chatgpt.com/backend-api/codex`, and keeps Codex app-server as an
optional runtime. Noema should adopt the direct-provider shape and replace
app-server immediately for the primary chat path.

## Chosen Approach

Use a shared Responses transport plus a Codex-specific provider.

The existing `OpenAiProvider` already contains useful `/responses` HTTP request
and response parsing logic. The migration should extract the reusable parts into
a provider-neutral Responses transport, then add `CodexResponsesProvider` for
Codex OAuth, token refresh, provider labeling, and Codex backend quirks.

Rejected alternatives:

- Reusing `OpenAiProvider` directly for Codex would be faster, but would blur
  OpenAI API-key and Codex OAuth behavior.
- Porting Hermes' full credential pool and model inventory system would be too
  large for this slice.
- Keeping app-server as a default or fallback would preserve the plugin/skill
  home-state problem this migration is meant to remove.

## Architecture

Noema keeps its provider-neutral `GenerateRequest` and `GenerateResponse`
contract. The daemon runtime continues to own:

- Noema conversation IDs.
- Turn creation and persistence.
- Recent transcript reconstruction.
- Structured Noema response prompting.
- Memory proposal persistence.
- Local `search_memory` tool continuation.
- GraphQL subscription events.

Only provider execution changes. The current stateful Codex app-server thread is
replaced by direct HTTPS calls to the Codex Responses API.

### Responses Transport

Create a small shared transport for OpenAI-compatible Responses calls. It should
own:

- Building `POST {base_url}/responses`.
- Sending bearer auth.
- Serializing `model`, `input`, optional `instructions`, optional generation
  controls, and `store: false`.
- Parsing response ids, model ids, text output, and token usage.
- Mapping non-success HTTP statuses into `ProviderError`.

The transport should not know where credentials come from.

### Codex Responses Provider

Add `CodexResponsesProvider` as the provider-specific layer for Codex OAuth.
It should:

- Load the active account's Noema-owned token file.
- Refresh access tokens when they are missing or near expiry.
- Write refreshed tokens back atomically.
- Call the shared Responses transport with provider id `codex`.
- Use base URL `https://chatgpt.com/backend-api/codex` by default.
- Return `GenerateResponse.provider = "codex"`.

### Configuration

Split or rename the current Codex CLI-shaped config. App-server-only settings
such as `command`, `sandbox`, `ephemeral`, `ignore_rules`,
`ignore_user_config`, and `codex_home` should not appear to govern direct
Responses behavior.

The direct Codex config should include:

- `base_url`, defaulting to `https://chatgpt.com/backend-api/codex`.
- `model`, optional default model.
- `timeout_seconds`.
- `refresh_skew_seconds`, defaulting to a small pre-expiry refresh window such
  as 120 seconds.

The existing `codex.command` field can be removed from the primary runtime
design. Docker no longer needs to install the Codex CLI for normal chat.

## Authentication

Noema should own the Codex OAuth device-code flow directly instead of shelling
out to `codex login`.

The adapter performs:

1. Request a device/user code from OpenAI auth.
2. Return only safe fields to the UI: `verification_url`, `user_code`, status,
   and fixed instruction text.
3. Poll for authorization completion inside the daemon.
4. Exchange the authorization code for OAuth tokens.
5. Persist tokens under the provider account home.
6. Mark the provider account authenticated in Postgres.

The token endpoint details and Codex OAuth client id should live in the Codex
auth adapter, not in the frontend. They should be treated as provider protocol
constants, not user secrets.

### Credential File

`NOEMA_HOME/providers/codex/default/auth.json` becomes Noema-owned token state.
It should contain only the fields Noema needs, such as:

```json
{
  "auth_mode": "chatgpt",
  "source": "device_code",
  "last_refresh": "2026-06-27T00:00:00Z",
  "tokens": {
    "access_token": "...",
    "refresh_token": "..."
  }
}
```

The provider account directory should be private. On Unix, account directories
should be `0700`, and token files should be written with private permissions.
Writes should be atomic to avoid partial token files.

### Existing Codex CLI Homes

Noema should not silently trust or import existing account homes that were
created as `CODEX_HOME` directories. A bare `auth.json` file is no longer proof
of authentication.

For this migration, existing users should re-authenticate through Noema's new
device-code flow. A future explicit import command can be designed separately if
needed.

## Runtime Data Flow

The direct Responses runtime is stateless from the provider's perspective.
Noema supplies context from its own persisted transcript.

For each turn:

1. The daemon creates the Noema conversation turn and appends the user item.
2. The daemon reads recent conversation items from Postgres.
3. The daemon builds structured turn instructions and the user input.
4. `CodexResponsesProvider` ensures a usable access token.
5. The provider sends a `/responses` request.
6. The provider parses assistant text and any Noema structured response
   envelope.
7. The daemon persists assistant text, memory proposals, local tool results, and
   follow-up continuation responses exactly as it does today.

Memory extraction should use the same direct provider path as chat. The hidden
second app-server subprocess currently used for extraction should disappear.

## Error Handling

Provider errors should be classified without leaking token contents, raw
provider output, or credential paths.

Authentication failures:

- `invalid_grant`
- `invalid_token`
- `refresh_token_reused`
- HTTP `401`
- HTTP `403`

These mark the provider account unauthenticated and cause onboarding to block
future chat until re-authentication.

Rate limit or quota failures:

- HTTP `429`

These fail the current turn with a safe message, but keep the provider account
authenticated.

Transient failures:

- network errors
- HTTP `5xx`
- provider timeouts

These fail the current turn and leave account auth status unchanged.

Malformed responses:

- invalid JSON
- missing output text
- missing required Noema structured envelope when one is required

These fail the current turn and leave account auth status unchanged.

Unsupported request options should be rejected before the HTTP call when the
provider cannot safely support them.

## Onboarding

Onboarding remains provider-account based. The required first-run step is still
connecting the active Codex provider account.

The reconciliation behavior changes:

- Existing `auth.json` file presence is not enough.
- Noema checks whether the token file is parseable and has an access token.
- If the token is expired or near expiry and a refresh token is available,
  Noema attempts a refresh before reporting the account status.
- If refresh fails with an authentication failure, account status becomes
  unauthenticated.
- If refresh fails because of rate limit or transient network failure, account
  status should not be incorrectly downgraded to unauthenticated.

## Testing

Focused tests should cover:

- Device-code auth state machine with mocked HTTP responses.
- Safe auth attempt payloads: no raw token, credential path, or provider output.
- Token file creation, permissions, parse errors, and atomic refresh writes.
- Access-token refresh before provider calls.
- Refresh auth failure marking provider account unauthenticated.
- Refresh rate limit preserving authenticated state.
- Responses request body construction and text parsing with a mocked HTTP
  client/server.
- `GenerateResponse` provider/model/id/usage mapping.
- Daemon turn persistence without provider thread ids.
- Memory extraction through the direct provider.
- Onboarding no longer treating bare `auth.json` existence as authenticated.
- Retirement or deletion of app-server-specific tests and config assertions.

Validation for the implementation should include:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

## Migration Boundaries

In scope:

- Replace app-server as the primary Codex chat runtime.
- Replace `codex login --device-auth` with Noema-owned device-code OAuth.
- Store Codex OAuth tokens under Noema account homes.
- Refactor shared Responses transport code.
- Update onboarding status and tests.
- Update docs that currently describe Codex CLI as required for normal chat.

Out of scope:

- Long-term support for app-server as an optional tool runtime.
- Silent import of existing `CODEX_HOME` token files.
- Multiple Codex accounts.
- Model inventory probing.
- Credential pool balancing.
- Compatibility migrations for existing pre-V1 credential homes.

## Open Risks

The Codex backend URL is not the same as the official OpenAI Platform API. This
is an intentional Hermes-like choice for ChatGPT/Codex OAuth, but the contract
may differ from or change independently of `api.openai.com/v1/responses`.

Refresh tokens may be single-use. Noema should avoid sharing token state with
Codex CLI or other clients and should avoid importing CLI token files silently.

If the backend emits richer Responses items beyond text, the first migration can
ignore them unless they are needed for Noema's structured response envelope. The
provider should fail clearly if required assistant text is absent.

## Success Criteria

- A fresh Noema home can authenticate Codex without installing or invoking the
  Codex CLI.
- After login, `NOEMA_HOME/providers/codex/default/` contains only
  Noema-owned credential/account state, not Codex CLI plugin and skill caches.
- Primary chat works through direct Codex Responses calls.
- Memory extraction works through the same direct provider path.
- Expired tokens refresh automatically when possible.
- Auth failures return the user to onboarding without exposing secrets.
- Existing GraphQL chat behavior and transcript persistence remain stable.
