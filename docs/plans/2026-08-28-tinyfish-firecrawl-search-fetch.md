# Add TinyFish and Firecrawl Search and Fetch

- **Status:** Approved execution plan
- **Mode:** Implement
- **Date:** 2026-08-28
- **Provider ids:** `tinyfish` and `firecrawl`
- **Initial scope:** Search and fetch
- **Default:** Existing search and fetch providers remain the defaults

## Summary

Add TinyFish and Firecrawl as providers for `web.search` and `web.fetch`.

TinyFish uses an API key in the `X-API-Key` header.

Firecrawl supports two access modes:

- Firecrawl Keyless sends no `Authorization` header.
- An authenticated Firecrawl account sends an API key as a bearer credential.

Users can select each provider independently for search and fetch. A Firecrawl
API-key account can supply both capabilities.

Firecrawl Keyless is available through one permanent system account. It uses
the same Firecrawl Cloud REST endpoints as authenticated accounts.

References:

- [TinyFish search reference](https://docs.tinyfish.ai/search-api/reference)
- [TinyFish fetch reference](https://docs.tinyfish.ai/fetch-api/reference)
- [Firecrawl Keyless launch](https://www.firecrawl.dev/blog/firecrawl-keyless-launch)
- [Firecrawl search reference](https://docs.firecrawl.dev/api-reference/endpoint/search)
- [Firecrawl scrape reference](https://docs.firecrawl.dev/api-reference/endpoint/scrape)

Do not add Agent, Browser, Crawl, Map, Parse, or interactive page support.
Do not add self-hosted Firecrawl endpoint configuration. Do not change default
providers.

## Provider Transports

### TinyFish

- Add one redacted transport with separate search and fetch endpoints.
- Use a 10-second search timeout and a 150-second fetch timeout.
- Send `query` and optional `purpose` for search.
- Limit normalized results to Noema's requested result count.
- Send one URL, Markdown format, optional `purpose`, and `ttl: 0` for fetch.
- Send the protected API key only through `X-API-Key`.

### Firecrawl

- Add one redacted transport for `https://api.firecrawl.dev/v2`.
- Let the transport hold an optional protected API key.
- Omit `Authorization` completely in Keyless mode.
- Send `Authorization: Bearer <key>` only for an authenticated account.
- Use `POST /search` with `query`, `limit`, and the `web` source.
- Normalize results from `data.web` without requesting page scraping.
- Use `POST /scrape` with Markdown, links, main-content extraction, and
  `maxAge: 0`.
- Set the Firecrawl request timeout to 60 seconds.
- Set the HTTP client timeout to 75 seconds.

### Shared behavior

- Validate public URLs before each fetch.
- Sanitize requested URLs, final URLs, and returned links.
- Bound returned content by the existing `FetchRequest.max_chars` value.
- Normalize into the existing `SearchResponse` and `FetchResponse` types.
- Do not add model-visible tool fields or change GraphQL shapes.
- Never place provider credentials in model context, logs, events, or errors.

Map HTTP status and provider response failures into existing safe errors.
Only an authenticated account can produce `AuthFailed` from a provider 401 or
403 response. Treat the same Keyless response as a provider failure. This rule
must not mark the permanent Keyless account unauthenticated.

## Accounts and Capabilities

- Add `tinyfish` and `firecrawl` to the provider account catalog.
- Let users create TinyFish and Firecrawl API-key accounts.
- Give each created account a generated account key and protected secret store.
- Add `provider_account:firecrawl:public` as a permanent Keyless system account.
- Display the system account as `Firecrawl Keyless`.
- Prevent deletion of the Keyless system account.
- Keep DuckDuckGo and direct HTTP as the existing defaults.
- Declare search citations for TinyFish and Firecrawl.
- Declare direct fetch and JavaScript rendering for both providers.
- Do not declare authenticated browser context.
- Mark each capability as a hosted-provider data flow.
- Resolve Firecrawl Keyless by its exact system account identifier.
- Load credentials for all user-created Firecrawl accounts.

Reuse the current Settings add-account dialog for authenticated accounts.
Reuse the existing web-provider selectors for all accounts. No custom frontend
layout or generated GraphQL change is required.

Public additions include stable provider identifiers. They also include
exported TinyFish and Firecrawl search and fetch clients.

## Stored State

Append schema migration 66. Do not revise any prior migration.

The migration must:

- add `tinyfish` and `firecrawl` to the provider-kind check;
- preserve all existing provider account rows and capability bindings;
- insert the permanent Firecrawl Keyless account;
- converge to the same schema for upgraded and fresh databases.

The Keyless account uses `auth_method = 'none'`. Its available status does not
claim that a human credential exists.

## Test Plan

Add no more than ten focused Rust tests.

1. Test TinyFish search requests and result normalization.
2. Test TinyFish fetch requests, content bounds, links, and URL sanitization.
3. Test TinyFish authentication, rate-limit, timeout, and parse failures.
4. Test Firecrawl authenticated and Keyless header behavior.
5. Test Firecrawl search requests, limits, and `data.web` normalization.
6. Test Firecrawl scrape requests and response normalization.
7. Test Firecrawl safe errors and the Keyless authentication distinction.
8. Test account creation, secret access, catalog entries, and capabilities.
9. Test Settings options for Keyless and authenticated Firecrawl accounts.
10. Test schema 65 upgrade preservation and fresh version 66 convergence.

Run these validation commands:

- focused provider, store, host, and API unit tests through `cargo validate`;
- `cargo fmt --all --check`;
- `cargo check-workspace`;
- `cargo gate-lint`;
- `cargo gate-test`.

Run the Rust size report with these limits:

- maximum production net: 850 lines;
- maximum test net: 350 lines;
- maximum new Rust tests: 10.

## Assumptions and Stop Conditions

- Use display names `TinyFish`, `Firecrawl`, and `Firecrawl Keyless`.
- Firecrawl Keyless means Firecrawl Cloud without an API key.
- Do not support a custom Firecrawl base URL in this unit.
- Search and fetch can use different accounts.
- Both fetch providers bypass stale cache entries.
- Preserve unrelated worktree changes.
- Stop if implementation needs a new public abstraction.
- Stop if migration 66 cannot preserve existing rows.
- Stop if either code budget grows by 50 percent.

