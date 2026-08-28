# Add TinyFish and Firecrawl Through Shared Web Provider Paths

- **Status:** Revised execution plan
- **Date:** 2026-08-28
- **Mode:** Implement in two committed units
- **Provider ids:** `tinyfish` and `firecrawl`
- **Initial scope:** Search and fetch
- **Default:** Existing search and fetch providers remain the defaults

## Observable Outcome

Users can add TinyFish and authenticated Firecrawl accounts in Settings.
They can select either account for `web.search`, `web.fetch`, or both.

Users can also select one permanent `Firecrawl Keyless` system account.
That account uses Firecrawl Cloud without a credential.

The implementation removes repeated provider account and web result policy.
It does not add a generic provider framework.

## Non-goals

- Do not add TinyFish Agent or Browser support.
- Do not add Firecrawl Agent, Browser, Crawl, Map, Parse, or Interact support.
- Do not add self-hosted Firecrawl endpoint configuration.
- Do not change model-visible tool input or GraphQL shapes.
- Do not change the default DuckDuckGo and direct HTTP selections.
- Do not add a universal enum for model, search, fetch, and browser providers.
- Do not add a provider factory trait, transport trait, compatibility layer, or client aliases.
- Do not redesign model generation, browser sessions, or provider routing outside the repeated paths below.

## Verified Provider Contracts

The request details below were checked against the official references on 2026-08-28.

- [TinyFish search reference](https://docs.tinyfish.ai/search-api/reference)
- [TinyFish fetch reference](https://docs.tinyfish.ai/fetch-api/reference)
- [Firecrawl Keyless launch](https://www.firecrawl.dev/blog/firecrawl-keyless-launch)
- [Firecrawl search reference](https://docs.firecrawl.dev/api-reference/endpoint/search)
- [Firecrawl scrape reference](https://docs.firecrawl.dev/api-reference/endpoint/scrape)

TinyFish search uses `GET https://api.search.tinyfish.ai`.
TinyFish fetch uses `POST https://api.fetch.tinyfish.ai`.
Both endpoints require `X-API-Key`.

Firecrawl search uses `POST https://api.firecrawl.dev/v2/search`.
Firecrawl fetch uses `POST https://api.firecrawl.dev/v2/scrape`.
Authenticated calls use bearer authentication.
Keyless REST calls omit the `Authorization` header.

## Audit Findings

The feature currently crosses repeated policy in five areas.

1. Provider identity appears in host, API, runtime, store, and provider modules.
2. Secret-account eligibility appears in the catalog, store, account service, and credential service.
3. Runtime constructs built-in web account facts instead of loading their stored rows.
4. Search and fetch adapters repeat URL, text, ranking, content-bound, and error normalization.
5. Runtime detects authentication failure by comparing a model-visible English error string.

The last finding is also a correctness risk.
Firecrawl Keyless must never become unauthenticated after a credential-free request fails.

## Architectural Decisions

### Keep Existing Authorities

- SQLite remains the source for every selectable provider account.
- The provider account catalog remains the source for user-addable account types.
- `capabilities_for_provider_account` remains the source for account capabilities.
- `ProviderCredential` remains the typed secret passed to concrete adapters.
- `WebSearchBackend` and `WebFetchBackend` remain the provider-neutral execution boundaries.
- Each provider keeps typed request and response structures for its external protocol.

### Consolidate Only Current Repetition

- Let `ProviderKind` return its fixed model provider account id.
- Add stable constants for built-in web account ids.
- Add one exact catalog lookup for account creation and secret eligibility.
- Add a bounded catalog field for generated or fixed account identity.
- Load default web accounts through the same stored-account path as selected accounts.
- Add private web adapter helpers for normalization and safe error mapping.
- Preserve typed backend failure until runtime completes account-status handling.

## Unit One: Net-negative Provider Path Consolidation

Complete and commit this unit before adding TinyFish or Firecrawl.
Use the size reporter with `--require-net-negative`.

### Provider Account Identity

- Add `ProviderKind::default_account_id()` for fixed model accounts.
- Replace host and adapter copies of fixed model account ids with that method.
- Export exact account-id constants for DuckDuckGo, direct HTTP, Obscura, and Firecrawl Keyless.
- Use those constants in API defaults, runtime defaults, host resolution, and deletion checks.
- Keep SQL literals inside the migration because Rust constants cannot own SQLite schema text.
- Do not replace fixture literals when they make a stored compatibility case clearer.

### Account Creation and Secret Eligibility

- Extend the existing catalog entry with its current account identity rule.
- Use `generated` for Exa, Kernel, TinyFish, and Firecrawl secret accounts.
- Use `fixed` for current singleton account types.
- Make store account creation read the catalog entry instead of matching every secret provider name.
- Keep provider-specific setup only when behavior differs, such as OpenRouter catalog validation.
- Make secret mutation and API-key access use the catalog's supported authentication methods.
- Validate the stored account kind and actual authentication method before every secret read or write.
- Remove the repeated `exa | kernel | openrouter` allowlists.
- Protect built-in accounts by exact account id, not by provider kind.
- Allow deletion of authenticated Firecrawl accounts while protecting only Firecrawl Keyless.

### Stored Default Resolution

- Resolve an absent search, fetch, or browse binding to one stable default account id.
- Load that default account from SQLite through the normal account reader.
- Derive provider kind, account key, status, capabilities, and credential revision from that row.
- Treat a missing built-in row as a stored-state invariant failure.
- Remove runtime construction of synthetic default account facts.
- Use the same stored default path after a selected credential cannot be loaded.
- Derive assignment tool names from `CapabilityId` instead of local string constants.

### Typed Authentication Failure

- Keep `WebSearchError` or `WebFetchError` available until runtime handles the result.
- Store only the current safe error payload after typed account handling completes.
- Remove `is_provider_account_unauthenticated_payload` and its English text comparison.
- Record an authentication failure only for `AuthFailed` with a positive credential revision.
- Fence the update with the observed credential revision.
- Keep Firecrawl Keyless at credential revision zero.
- Treat a Keyless `401` or `403` as a provider HTTP failure, not `AuthFailed`.

## Unit Two: Shared Hosted Web Support and New Providers

### Shared Private Web Adapter Support

Add one private support module under the existing web adapters.

The support module must:

- normalize whitespace in titles and snippets;
- remove credential-bearing URL components;
- preserve ordinary URL components;
- reject malformed, local, private, and unsupported result URLs;
- rank accepted search results after filtering;
- apply one provider-neutral search summary format;
- validate each requested fetch URL before dispatch;
- sanitize the requested and final fetch URLs;
- normalize, deduplicate, and limit returned links to 256;
- bound Markdown by `FetchRequest.max_chars` on character boundaries;
- calculate raw, returned, and truncated character facts once;
- map URL-policy failures into `WebFetchError` once;
- map request timeouts and HTTP statuses without reading provider error prose.

Apply the search helper to DuckDuckGo and Exa.
Apply the raw Markdown fetch helper to Exa, TinyFish, and Firecrawl.
Keep direct HTTP response construction separate because it owns streaming and summarization.

Add `WebFetchError::RateLimited` for hosted fetch status `429`.
Use the existing search rate-limit error for hosted search status `429`.

### One Client Per Hosted Web Provider

- Replace the two public Exa client aliases with one `ExaWebClient`.
- Add one `TinyFishWebClient` that implements both web backend traits.
- Add one `FirecrawlWebClient` that implements both web backend traits.
- Keep each provider's endpoint, headers, timeouts, and wire structures in its own module.
- Pass `ProviderCredential` into authenticated clients without converting it to an ordinary string.
- Let Firecrawl hold `Option<ProviderCredential>` for its two access modes.
- Keep client debug output redacted while preserving ordinary endpoint diagnostics.
- Do not add a configurable generic REST client.

### TinyFish Requests

- Use a 10-second search client timeout.
- Send `query` and optional `purpose` as GET query parameters.
- Ignore TinyFish's returned position and rank accepted results after URL filtering.
- Limit accepted results to `SearchRequest.max_results`.
- Use a 150-second fetch client timeout.
- Send one URL, `format: "markdown"`, `links: true`, and `ttl: 0`.
- Send optional `purpose` when the request reason exists.
- Read the first matching item from `results`.
- Map structured per-URL `timeout` to `WebFetchError::Timeout`.
- Map other structured per-URL failures to existing safe typed errors.
- Never use provider error prose as a policy or retry authority.

### Firecrawl Requests

- Use one client for `https://api.firecrawl.dev/v2`.
- Set the HTTP client timeout to 75 seconds.
- Set the provider request timeout to 60 seconds.
- Send `Authorization: Bearer <key>` only when a credential exists.
- Omit `Authorization` completely for Firecrawl Keyless.
- Send search `query`, `limit`, and `sources: ["web"]`.
- Do not send search scrape options.
- Normalize only `data.web` from a successful response.
- Send scrape formats `markdown` and `links`.
- Send `onlyMainContent: true`, `skipTlsVerification: false`, `maxAge: 0`, and `timeout: 60000`.
- Normalize `data.markdown`, `data.links`, and final metadata URL fields.
- Treat `success: false` as a safe provider failure without matching its prose.

## Accounts, Capabilities, and Settings

- Add `tinyfish` and `firecrawl` to the user-addable account catalog.
- Use generated account keys for both authenticated provider types.
- Use display names `TinyFish` and `Firecrawl` for created accounts.
- Insert `provider_account:firecrawl:public` as `Firecrawl Keyless`.
- Give the Keyless row `account_key = 'public'` and `auth_method = 'none'`.
- Set the Keyless row active, default within Firecrawl, and `status = 'authenticated'`.
- Let `auth_method = 'none'` remain the authority that no credential exists.
- Declare hosted search and fetch capabilities for Exa, TinyFish, and Firecrawl through one helper.
- Parameterize fetch JavaScript rendering instead of deriving it from `WebBrowse`.
- Keep Exa fetch JavaScript rendering false.
- Set TinyFish and Firecrawl fetch JavaScript rendering true.
- Set search citations and direct fetch true for both new providers.
- Keep authenticated browser context false.
- Reuse the current Settings dialog and provider selectors without frontend changes.

## Stored State

Append schema migration 66.
Do not revise any previous migration.

The migration must:

- rebuild `provider_accounts` with `tinyfish` and `firecrawl` in its kind check;
- copy every existing provider account row without transformation;
- preserve all capability binding foreign keys;
- insert the permanent Firecrawl Keyless row;
- produce the same schema for a version 65 upgrade and a fresh database.

Do not add a configuration field or a second system-account table.

No web application source or generated GraphQL file should change.

## Focused Test Plan

Add no more than eight new Rust tests.

1. Prove shared search filtering, ranking, text normalization, and ordinary URL preservation.
2. Prove shared fetch bounds, link limits, URL sanitization, and non-secret preservation.
3. Prove TinyFish search and fetch methods, fields, headers, timeouts, and normalized results.
4. Prove Firecrawl search and fetch bodies for authenticated and Keyless clients.
5. Prove typed status mapping, including rate limits, timeouts, and Keyless authentication distinction.
6. Prove catalog-backed creation, secret access, capability features, and exact built-in deletion protection.
7. Prove stored default resolution and typed authentication failure updates without payload text matching.
8. Prove version 65 preservation and fresh version 66 convergence, including the Keyless row.

Do not add tests for client aliases, constructors, getters, GraphQL pass-through mapping, or repeated layer behavior.

## Budgets and Milestones

Use the implementation start commit as the size-report base.

Unit One must be net-negative in production code.
Commit it after focused validation.

The complete two-unit change has these circuit breakers:

- maximum production net: 600 lines;
- maximum test net: 300 lines;
- maximum new Rust tests: 8.

Run the reporter after Unit One, after provider adapters, and before each commit.
Stop before adding another helper when the existing authority can own the behavior.
Stop if production or test code exceeds its budget by 50 percent or 500 lines.
Do not raise a budget after failure.

## Validation

Run focused unit tests through `cargo validate` for providers, store, host, runtime, and API.
Then run:

- `cargo fmt --all --check`
- `cargo check-workspace`
- `cargo gate-lint`
- `cargo gate-test`
- `git diff --check`

Run unit tests only.

## Acceptance Scenarios

1. A human adds TinyFish once and can select it independently for search and fetch.
2. A human adds Firecrawl once and can select it independently for search and fetch.
3. Firecrawl Keyless appears without appearing in the add-account catalog.
4. No Keyless request sends an `Authorization` header.
5. A Keyless provider rejection never changes its stored account status.
6. An authenticated rejection changes only the current credential revision to unauthenticated.
7. Malformed or sensitive provider URLs do not enter results, persistence, or model context.
8. Existing provider accounts, bindings, defaults, and Exa behavior survive migration and refactoring.

## Stop Conditions

- Stop if migration 66 cannot preserve every existing provider account and binding.
- Stop if the reduction unit becomes net-positive.
- Stop if implementation needs a new public trait or universal provider registry.
- Stop if shared support weakens a provider's typed protocol validation.
- Stop if account policy still depends on provider-name allowlists after Unit One.
- Stop if authentication state still depends on model-visible error text.
- Stop if the work requires frontend layout or GraphQL schema changes.
