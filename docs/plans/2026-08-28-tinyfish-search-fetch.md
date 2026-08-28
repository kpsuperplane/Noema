# Add TinyFish Search and Fetch

- **Status:** Approved execution plan
- **Mode:** Implement
- **Date:** 2026-08-28
- **Provider id:** `tinyfish`
- **Initial scope:** Search and fetch
- **Default:** Existing search and fetch providers remain the defaults

## Summary

Add `tinyfish` as one API-key provider for `web.search` and `web.fetch`.

TinyFish uses `X-API-Key` authentication. Search uses GET, and fetch uses POST.

- [TinyFish search reference](https://docs.tinyfish.ai/search-api/reference)
- [TinyFish fetch reference](https://docs.tinyfish.ai/fetch-api/reference)

Users can add TinyFish in Settings. They can select it for search, fetch, or
both.

Do not add TinyFish Agent or Browser support. Do not change default providers.

## Implementation Changes

- Add one shared, redacted TinyFish transport with separate search and fetch
  endpoints.
- Use a 10-second search timeout and a 150-second fetch timeout.
- Send `query` and optional `purpose` for search.
- Limit normalized search results to Noema's requested count.
- Send one URL, Markdown format, optional `purpose`, and `ttl: 0` for fetch.
- Validate public URLs before each fetch.
- Sanitize returned URLs before persistence or display.
- Normalize results into the existing `SearchResponse` and `FetchResponse`
  types.
- Map authentication, rate-limit, timeout, malformed response, and per-URL
  failures into existing safe errors.
- Add `tinyfish` to the account catalog, credential authority, capability
  declarations, and host resolver.
- Declare hosted search citations, direct fetch, and JavaScript rendering.
- Do not declare authenticated browser context.
- Add schema migration 66.
- Extend the stored provider-kind check without changing prior migrations.
- Preserve all existing provider accounts and bindings during migration.
- Reuse the current Settings dialog and web-provider selectors.
- Do not change the GraphQL shape or add custom UI.

Public additions include the stable `tinyfish` provider identifier. They also
include exported TinyFish search and fetch clients.

## Test Plan

- Test search method, parameters, purpose, API-key header, result limits, and
  response normalization.
- Test fetch body, live-fetch setting, API-key header, content bounds, links,
  and URL sanitization.
- Test HTTP and per-URL error mapping, including authentication and timeout
  failures.
- Test account creation, secret access, capability metadata, and Settings
  catalog visibility.
- Test schema 65 upgrade preservation and fresh schema convergence at version
  66.
- Run focused unit tests for providers, store, host, and API.
- Run `cargo fmt --all --check`.
- Run `cargo check-workspace`.
- Run `cargo gate-lint`.
- Run `cargo gate-test`.
- Run the Rust size report with a 500-line production budget.
- Use a 220-line test budget and an eight-test limit.

## Assumptions

- Use provider kind `tinyfish` and display name `TinyFish`.
- One TinyFish API key supplies both capabilities.
- Search and fetch can be selected independently.
- Fetch requests bypass stale cache entries.
- Preserve unrelated worktree changes.
- Stop if implementation needs a new public abstraction.
- Stop if either code budget grows by 50 percent.

