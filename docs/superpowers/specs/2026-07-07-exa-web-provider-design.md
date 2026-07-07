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
- A default Exa provider account metadata path derived from `EXA_API_KEY`
  availability.
- Runtime Exa adapters for search and fetch.
- Web tool binding support through the existing GraphQL and Settings surfaces.
- Focused unit tests for capability declaration, provider resolution, request
  serialization, response normalization, and fallback behavior.

This slice does not include:

- A new secret-entry UI.
- Multiple Exa accounts.
- Exa crawl, research, answer, Websets, monitors, or MCP support.
- New model-visible tool arguments for provider selection.

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

The default account metadata is:

- `provider_account_id`: `provider_account:exa:default`
- `provider_kind`: `exa`
- `account_key`: `default`
- `display_name`: `Exa`
- `auth_method`: `secret_input`
- `status`: `authenticated` when `EXA_API_KEY` is non-empty, otherwise
  `unauthenticated`

The account declares:

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

Use `EXA_API_KEY` for this slice. The key is read from process environment at
runtime and is not persisted in SurrealDB.

The implementation must not log, persist, or surface the key. Tests may use
fake values such as `secret`.

This keeps the feature shippable while leaving first-class secret input and
provider-account credential storage for a later provider-auth slice.

## Settings And GraphQL

The existing `webToolSettings` query and `saveWebToolProviderBinding` mutation
should work without schema changes once the Exa provider account and
capabilities exist.

Settings should show Exa as a selectable option for Search and Fetch when
`EXA_API_KEY` is available. If an already-saved Exa binding later becomes
unauthenticated because the environment key is missing, the runtime should fall
back through the existing safe fallback metadata.

## Security And Privacy

`web.search` sends the search query to Exa. `web.fetch` sends the requested URL
to Exa and receives extracted page text. The provider capability metadata should
make these data flows visible.

Noema's existing `web.fetch` URL validation remains authoritative before
runtime execution. Exa fetch must not weaken scheme, private-address,
credential, or fragment restrictions.

Search and fetch normalization must avoid storing raw provider response bodies
inside transcript display metadata. Normalized payloads may include only the
existing tool result fields.

## Testing

Focused tests should cover:

- Exa capabilities include both `web.search` and `web.fetch`.
- Exa account status derives from `EXA_API_KEY` without persisting the secret.
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
