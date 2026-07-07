# Exa Web Provider Design

## Goal

Add Exa as a Noema provider that can supply the existing provider-neutral
`web.search` and `web.fetch` tools.

The model-visible tool contracts stay unchanged. Agents still call
`web.search` with a query and `web.fetch` with a URL; Noema chooses the backend
through provider capability bindings.

## Scope

This slice includes:

- Provider kind `exa`.
- Exa capability declaration for `web.search` and `web.fetch`.
- A generic Add Provider Account flow in Settings, with Exa as one available
  provider type.
- Secret input from Settings for Exa API keys.
- Runtime Exa adapters for search and fetch.
- Web tool binding support through the existing GraphQL and Settings surfaces.
- Focused unit tests for capability declaration, provider resolution, request
  serialization, response normalization, and fallback behavior.

This slice does not include:

- Exa crawl, research, answer, Websets, monitors, or MCP support.
- New model-visible tool arguments for provider selection.
- A generalized multi-field credential framework beyond the narrow
  `secret_input` provider account path needed for Exa.
- Provider account deletion beyond whatever existing inactive/deactivation
  behavior already exists.

## External API Shape

Exa search uses `POST https://api.exa.ai/search` with `x-api-key` and a JSON
body containing `query` and `numResults`.

Exa fetch uses `POST https://api.exa.ai/contents` with `x-api-key` and a JSON
body containing `urls: [url]` and `text: true`.

Noema should request only the fields needed for the current tool contracts.
Search should normalize `title`, `url`, and the best available snippet from
`text`, `highlights`, or `summary`. Fetch should normalize the first returned
content result into markdown-like text using Exa's `text` field.

## Provider Model

Exa is a provider because it supplies useful capabilities to Noema. It is not a
model provider in this slice.

No Exa provider account should be listed until the user creates one through the
generic Add Provider Account flow.

The Add Provider Account flow should expose a catalog of provider types. Exa's
catalog entry is:

- `provider_kind`: `exa`
- `display_name`: `Exa`
- `auth_method`: `secret_input`
- capabilities: `web.search`, `web.fetch`

Creating an Exa account writes a normal durable provider account row:

- `provider_account_id`: generated from the provider kind and generated account
  key
- `provider_kind`: `exa`
- `account_key`: generated stable key, not derived from the mutable display
  name
- `display_name`: custom user-provided name or `Exa`
- `auth_method`: `secret_input`
- `is_active`: true
- `is_default`: false unless the generic add flow later supports explicit
  default selection
- `status`: `authenticated` when a saved Exa key is available, otherwise
  `unauthenticated`

Each created Exa account declares:

- `web.search`
- `web.fetch`

Both capabilities use `ReliabilityContract::HostedProvider`. Search uses
`DataFlowClass::TrustedExternalSearchQuery`. Fetch uses
`DataFlowClass::ExternalWebFetch`. Search results persist compact metadata.
Fetch results persist compact content.

## Runtime Behavior

When `web.search` resolves to Exa, Noema calls Exa search and maps the response
to the existing `SearchResponse`.

When `web.fetch` resolves to Exa, Noema calls Exa contents and maps the response
to the existing `FetchResponse`.

Exa runtime execution is available only for created Exa provider accounts. The
resolver should not consider Exa as a default or system provider.

If an Exa binding is selected but no API key is available at runtime, Noema
does not fail the user-visible tool outright. It falls back through the
existing provider resolver metadata:

- `web.search` falls back to DuckDuckGo public search.
- `web.fetch` falls back to direct HTTP fetch.
- `fallback_from` names the Exa provider account id.
- `fallback_reason` is safe and non-secret, such as `provider account
  unauthenticated`.

HTTP 429 maps to the existing rate-limit error category for search. Fetch 429
maps to a safe HTTP failure unless a dedicated fetch rate-limit category exists
by implementation time. Non-success responses and malformed payloads return
safe existing error categories without exposing Exa response bodies.

## Credential Handling

Settings can save, replace, and clear the Exa API key for a created Exa
provider account. The key is stored under that provider account home, not in
SurrealDB:

```text
${NOEMA_HOME:-$HOME/.noema}/providers/exa/<account_key>/api_key.json
```

The file should be created with the same private account-home behavior used by
other Noema-owned provider credentials. The JSON shape can stay minimal:

```json
{ "api_key": "..." }
```

Runtime credential resolution reads only the saved provider-account key.
Noema should not read `EXA_API_KEY` for Exa. SurrealDB stores only non-secret
account metadata and status.

The implementation must not log, persist, or surface the key. Tests may use
fake values such as `secret`.

Saving a non-empty key writes it and marks the account `authenticated` as
credential-present. Noema should not perform a save-time Exa probe because the
current Exa docs do not expose a free status endpoint, and probing search or
contents could spend credits. Runtime `401` or `403` responses should update
the account to `unauthenticated` with a safe error code/message. Clearing the
key should remove the secret file and mark the account unauthenticated.

## Settings And GraphQL

Add GraphQL support for the generic add flow:

- `providerAccountCatalog`: list provider types that can be added. Exa is one
  entry.
- `createProviderAccount(providerKind, displayName, secret)`: create a durable
  provider account. `displayName` is optional for Exa and defaults to `Exa`.
  `secret` is required for Exa in this slice.

Add small GraphQL mutations for existing `secret_input` provider accounts,
scoped to the selected provider account:

- `saveProviderSecretInput(providerAccountId, secret)`
- `clearProviderSecret(providerAccountId)`

The create and secret mutations must validate provider kind, account state, and
auth method. For this slice, secret entry is supported only for
`provider_kind = "exa"`. The save resolver rejects blank secrets, writes the
secret file, updates safe provider status metadata, and returns the refreshed
`ProviderAccount`.

`providerAccounts` should list only created provider accounts. It must not
manufacture an Exa account from the provider catalog. The Providers settings
pane should show an Add Provider Account control. Choosing
Exa opens a form with optional account display name and required API key. After
creation, the account card should render write-only secret replacement and clear
actions. The UI must never echo the saved secret. It can show only status, last
checked time, and safe provider error text.

The existing `webToolSettings` query and `saveWebToolProviderBinding` mutation
should continue to own Search and Fetch provider selection. Exa should be a
selectable Search and Fetch provider when a created Exa account has available
capabilities. Provider option construction must include active created provider
accounts, not only provider accounts marked default, so custom-named Exa
accounts can be bound. If Exa is unauthenticated, it can still appear in
provider settings for setup, but web tool provider selection should disable it
using the existing disabled option model and a safe disabled reason.

## Security And Privacy

`web.search` sends the search query to Exa. `web.fetch` sends the requested
URL to Exa and receives extracted page text. The provider capability metadata
should make these data flows visible.

Noema's existing `web.fetch` URL validation remains authoritative before
runtime execution. Exa fetch must not weaken scheme, private-address,
credential, or fragment restrictions.

Search and fetch normalization must avoid storing raw provider response bodies
inside transcript display metadata. Normalized payloads may include only the
existing tool result fields.

Secret-input handling must treat API keys as write-only. GraphQL responses,
frontend state persisted outside React memory, logs, transcript metadata,
provider metadata, and test snapshots must not contain the key.

## Testing

Focused tests should cover:

- Exa capabilities include both `web.search` and `web.fetch`.
- Exa appears in `providerAccountCatalog`.
- Exa is not returned by `providerAccounts` until the user creates an account.
- Creating an Exa account accepts an optional display name, generates a stable
  account key, writes the secret file, and returns a safe provider account.
- Exa account status derives only from the saved provider-account secret
  without persisting the secret in SurrealDB.
- Settings secret save writes the provider-account secret file, rejects blank
  input, updates safe status metadata, and does not return the secret.
- Settings secret clear removes the provider-account secret file and updates
  safe status metadata.
- `webToolSettings` exposes Exa as a provider option when authenticated.
- Exa search request serialization uses `/search`, `x-api-key`, query, and
  `numResults`.
- Exa search normalization handles titles, URLs, highlights/text snippets, and
  max-result truncation.
- Exa contents request serialization uses `/contents`, `x-api-key`, `urls`,
  and `text`.
- Exa fetch normalization maps title, URL/final URL, content, raw excerpt,
  character counts, truncation, and extraction labels.
- Bound unauthenticated Exa accounts fall back to DuckDuckGo/direct HTTP with
  safe fallback metadata.

Validation should include `cargo fmt --all --check`, focused Rust tests, and
`cargo check --workspace`. Broader workspace clippy/tests can run if the slice
touches shared contracts enough to justify the extra time.
