# Provider Capabilities Design

## Goal

Make Noema's provider model broad enough to represent any third-party or
local service that supplies useful system capabilities, without changing the
stable model-visible `web.search` and `web.fetch` tool contracts.

The first conceptual slice should:

- Define a provider as a service integration, not only an LLM vendor.
- Let one provider account supply multiple capability kinds.
- Let one capability kind be supplied by multiple provider accounts.
- Keep model-visible tools provider-neutral.
- Add OpenAI hosted web search as a future `web.search` provider option.
- Treat Codex native web search as unavailable until there is a documented
  callable provider API.
- Keep Firecrawl as an example only; do not build Firecrawl support in this
  slice.

## Problem

Noema currently uses "provider" mostly to mean a model provider account such
as Codex, OpenAI, or Foundation Local. That was enough while provider choice
mostly answered "who generates the assistant response?"

The web tool layer makes the word too narrow. A service can supply useful
capabilities without supplying chat models:

- OpenAI can supply model generation and hosted web search.
- Codex can supply model generation through Noema-owned OAuth credentials, but
  Codex product-native web search is not currently a documented backend API
  Noema should call directly.
- DuckDuckGo public search supplies `web.search` but no models.
- Noema direct HTTP supplies `web.fetch` but no external account.
- A future crawler service such as Firecrawl could supply `web.search`,
  `web.fetch`, or `web.crawl` without being a model provider.

If Noema keeps "provider" synonymous with "model provider," web settings and
runtime selection will grow parallel concepts such as model providers, search
providers, fetch providers, crawl providers, and tool providers. That splits
credentials, status, policies, and user-facing settings around implementation
details instead of around real-world service accounts.

## Chosen Approach

Use capability-supplying providers.

A provider is any service integration, local or external, that can supply one
or more typed Noema capabilities. A provider account is one configured instance
of that integration. Capabilities describe what that account can do.

This makes OpenAI one provider account that may supply:

```text
model.generate
model.classify
web.search
```

It also makes a future crawler service one provider account that may supply:

```text
web.search
web.fetch
web.crawl
```

The model-visible Noema tools remain stable:

```text
web.search
web.fetch
```

The selected backend provider is runtime configuration, policy, and account
state. It is not a model-authored tool argument.

## Vocabulary

### Provider

A provider is a named integration implementation that can supply one or more
Noema capabilities.

Examples:

| Provider | Example capabilities |
| --- | --- |
| `openai` | `model.generate`, `model.classify`, `web.search` |
| `codex` | `model.generate`, `model.classify` |
| `foundation_local` | `model.generate`, `model.classify` |
| `duckduckgo_public` | `web.search` |
| `direct_http` | `web.fetch` |
| `firecrawl` | Illustrative future example only: `web.search`, `web.fetch`, `web.crawl` |

`duckduckgo_public` and `direct_http` may be system-owned providers with no
human credential setup. They should still fit the same capability model so
Settings, audit, policy, and runtime selection do not need special vocabulary.

### Provider Account

A provider account is one configured and authenticated instance of a provider.

Examples:

```text
provider_kind: openai
account_key: default
status: authenticated
```

```text
provider_kind: direct_http
account_key: system
status: available
```

Provider credential material continues to live under:

```text
${NOEMA_HOME:-$HOME/.noema}/providers/<provider>/<account>/
```

The structured store keeps only non-secret provider account metadata.

### Capability

A capability is a typed service surface that a provider account can supply.

Initial capability ids:

```text
model.generate
model.classify
web.search
web.fetch
web.crawl
```

`web.crawl` is reserved as a future capability. It is not part of the first
implementation plan.

Capability records should describe behavior relevant to policy and UI:

```text
capability_id
provider_kind
provider_account_id
status
reliability_contract
data_flow_class
supports_streaming
supports_citations
supports_direct_url_fetch
supports_js_rendering
supports_authenticated_context
supports_result_persistence
```

These fields are conceptual. The implementation can start smaller, but the
contract must preserve the distinction between "service account" and "things
that account can do."

### Tool

A tool is the stable model-visible Noema affordance. Tools are what the model
sees and calls.

Examples:

```text
web.search
web.fetch
search_memory
update_own_name
```

Tool contracts should not change when Noema swaps provider accounts behind
them. The model calls `web.search` with a query. Noema decides whether the
active backend is DuckDuckGo public search, OpenAI hosted web search, or a
future configured search provider.

### Binding

A binding maps a Noema capability need to a provider account capability.

Examples:

```text
tool: web.search
scope: system
provider_account_id: provider-account:openai-default
capability_id: web.search
```

```text
tool: web.fetch
scope: system
provider_account_id: provider-account:direct-http-system
capability_id: web.fetch
```

Bindings belong to Noema configuration and policy. They are not model-selected
tool arguments.

## Web Search Provider Options

`web.search` remains one Noema tool.

The initial provider remains:

| Provider account | Capability | Notes |
| --- | --- | --- |
| `duckduckgo_public:system` | `web.search` | Rust-only, no API key, best-effort public HTML adapter. |

Future provider option:

| Provider account | Capability | Notes |
| --- | --- | --- |
| `openai:<account>` | `web.search` | Uses OpenAI hosted web search through the Responses API when the selected account and model support it. |

OpenAI hosted web search should be modeled as `web.search`, not `web.fetch`.
The hosted OpenAI tool can search and may internally open pages for reasoning,
but it is model-mediated provider behavior, not Noema's deterministic direct
URL fetch contract. If OpenAI returns citations or source annotations, Noema
should preserve them in normalized result metadata and transcript inspection
surfaces.

The normalized Noema output should include provider behavior flags:

```json
{
  "provider": "openai",
  "provider_account_id": "provider-account:openai-default",
  "capability": "web.search",
  "provider_contract": "hosted_search",
  "query": "rust reqwest current version",
  "results": [],
  "citations": [],
  "summary": "Provider-hosted web search completed"
}
```

The exact result shape can be refined during implementation, but the model
must receive a normalized result that marks provider source, contract, and
citation support.

## Web Fetch Provider Options

`web.fetch` remains one Noema tool.

The initial provider remains:

| Provider account | Capability | Notes |
| --- | --- | --- |
| `direct_http:system` | `web.fetch` | Rust HTTP, public URL policy, redirect checks, readability extraction, summarization. |

OpenAI hosted web search should not be treated as a `web.fetch` provider unless
OpenAI exposes a documented direct URL-fetch API with semantics close enough to
Noema's `web.fetch(url)` contract.

Codex product web search should not be treated as a `web.search` or
`web.fetch` backend unless Codex exposes a documented callable provider API.
Codex CLI/app web features are product capabilities, not automatically a
Noema backend integration point.

Future fetch providers may include:

| Provider account | Capability | Notes |
| --- | --- | --- |
| `browser:<account-or-system>` | `web.fetch` | Future rendered fetch with JavaScript and maybe authenticated context. |
| `firecrawl:<account>` | `web.fetch`, `web.crawl` | Illustrative future service only; not implemented by this spec. |

## Runtime Selection

Noema should resolve provider capabilities before building model-visible tool
specs for a turn.

Conceptual flow:

```text
load provider accounts
-> list available capabilities for each account
-> apply health/auth/status filters
-> apply scope policy and user settings
-> select capability binding for web.search and web.fetch
-> expose stable Noema tool specs to the model
-> execute selected provider capability when the model calls the tool
-> normalize provider-specific result into Noema tool output
-> persist compact result metadata and feed output back to provider continuation
```

The model should not receive a `provider` argument for `web.search` or
`web.fetch`. Provider selection is a Noema governance decision.

If the configured provider capability is unavailable, Noema should either:

- fall back to the default system provider when policy allows, or
- return a structured failed tool result that says the configured provider
  capability is unavailable.

Fallback behavior must be visible in tool result metadata:

```json
{
  "provider": "duckduckgo_public",
  "fallback_from": "openai",
  "fallback_reason": "provider account unauthenticated"
}
```

Silent fallback is not acceptable because it breaks auditability and makes
answers hard to interpret.

## Provider Capability Registry

The runtime should move toward a provider capability registry. It can start as
static declarations in Rust and later become persisted account metadata.

Conceptual Rust shape:

```rust
pub struct ProviderCapability {
    pub provider_kind: String,
    pub account_key: String,
    pub capability_id: CapabilityId,
    pub status: CapabilityStatus,
    pub reliability_contract: ReliabilityContract,
    pub data_flow_class: DataFlowClass,
    pub features: CapabilityFeatures,
}

pub struct CapabilityFeatures {
    pub citations: bool,
    pub direct_url_fetch: bool,
    pub js_rendering: bool,
    pub authenticated_context: bool,
    pub result_persistence: ResultPersistencePolicy,
}
```

Static defaults:

| Provider | Account | Capability | Status |
| --- | --- | --- | --- |
| `duckduckgo_public` | `system` | `web.search` | available |
| `direct_http` | `system` | `web.fetch` | available |
| `openai` | configured account | `model.generate` | account dependent |
| `openai` | configured account | `model.classify` | account dependent |
| `openai` | configured account | `web.search` | account and model dependent |
| `codex` | configured account | `model.generate` | account dependent |
| `codex` | configured account | `model.classify` | account dependent |
| `foundation_local` | configured account | `model.generate` | platform and bridge dependent |
| `foundation_local` | configured account | `model.classify` | platform and bridge dependent |

No Firecrawl default should be added in this slice.

## Settings IA

Settings should continue to group visible pages as:

```text
Agents
Tools
  Web
  MCPs
Safety
  Approvals
  Identities
  Usage
System
  Providers
```

`System > Providers` should represent configured service accounts and their
available capabilities. `Tools > Web` should represent Noema's stable web
tools and the active provider binding for each.

Recommended ownership:

- `System > Providers`: authentication, health, account metadata, available
  capabilities, model catalog, and provider account lifecycle.
- `Tools > Web`: active `web.search` binding, active `web.fetch` binding,
  fetch summarizer model preference, provider behavior labels, and safety
  posture.
- `Safety > Usage`: auxiliary model preferences that are not web-specific,
  such as tool progress audit model selection.

`Tools > Web` may later expose a provider picker for search and fetch, but it
should only list provider accounts that actually advertise the matching
capability.

## Policy And Audit

Capability type must influence policy. Provider identity alone is not enough.

Examples:

| Capability | Data flow | Approval default |
| --- | --- | --- |
| `model.generate` | prompt and context to model provider | configured provider account policy |
| `model.classify` | bounded auxiliary prompt to model provider | configured provider account policy |
| `web.search` | query to search provider | not required for routine public search |
| `web.fetch` | request to arbitrary public origin | not required for public HTTP(S), with strict blocked targets |
| `web.crawl` | potentially broad traversal/export | future policy; likely tighter than fetch |

Audit records should include:

- stable Noema tool name
- selected provider account id
- capability id
- provider contract label
- fallback source and reason when applicable
- query or sanitized URL
- result count or content size
- citation support and citation count when applicable
- success or safe failure category

Provider diagnostics and raw payloads should stay in system diagnostics rather
than normal chat transcript rows.

## Storage

Provider accounts remain canonical structured records. Capability availability
can start as derived runtime metadata, then become persisted non-secret account
metadata when the UI needs inspection and filtering.

Do not persist:

- OpenAI hosted web-search raw provider payloads by default.
- Full raw fetched page bodies.
- Firecrawl-specific configuration, because Firecrawl is not implemented in
  this slice.

Persist compact normalized tool result metadata needed for transcript replay,
inspection, continuation, and audit.

## Error Handling

Provider capability resolution should return structured errors:

| Error | User-facing meaning |
| --- | --- |
| `capability_not_configured` | No provider is configured for the requested capability. |
| `capability_unavailable` | The selected provider account cannot currently supply the capability. |
| `capability_unsupported` | The provider does not support this capability. |
| `capability_auth_required` | The provider account must be authenticated. |
| `capability_policy_blocked` | Policy prevents using this provider capability in the current scope. |
| `capability_provider_failed` | The provider call failed after selection. |

Tool failures should feed back to the model like current web tool failures so
the model can recover in the same turn when appropriate.

## OpenAI And Codex Boundaries

OpenAI hosted web search is permissible as a future provider-backed
`web.search` implementation because it is documented as a Responses API hosted
tool.

OpenAI hosted web search should not be a `web.fetch` provider without a
documented direct URL-fetch contract. Provider-internal page opening is useful
for research answers, but it is not equivalent to Noema returning deterministic
readable markdown for a specific URL.

Codex native web search should remain out of scope until Codex exposes a
documented callable backend API suitable for Noema. Codex product settings such
as cached/live/disabled search modes are product behavior, not automatically a
Noema provider capability.

## Non-Goals

- No Firecrawl implementation.
- No `web.crawl` implementation.
- No browser-backed fetch.
- No hosted fetch provider.
- No model-selected `provider` argument for web tools.
- No Settings redesign beyond the conceptual ownership described here.
- No migration or backwards compatibility layer for pre-V1 provider schema.
- No attempt to wrap Codex CLI/app product web search as a backend provider.

## Implementation Shape

This spec should lead to a later implementation plan with these slices:

1. Add provider capability vocabulary and static declarations.
2. Teach provider account metadata/settings to expose capability lists.
3. Introduce capability bindings for `web.search` and `web.fetch`.
4. Keep existing DuckDuckGo and direct HTTP behavior as default bindings.
5. Add OpenAI hosted web search as an optional `web.search` capability behind
   explicit provider support checks.
6. Normalize OpenAI hosted web-search results into Noema `web.search` output.
7. Update Web Settings to show active bindings and provider behavior labels.

Each slice should preserve the existing model-visible tool schemas.

## Validation

Spec-driven implementation should include:

- Unit tests for provider capability declarations.
- Unit tests that provider capability resolution selects system defaults when
  no explicit binding exists.
- Unit tests that unavailable configured capabilities either visibly fall back
  or return structured failures according to policy.
- Unit tests proving `web.search` and `web.fetch` schemas do not gain a
  model-visible `provider` argument.
- Provider adapter tests for OpenAI hosted web search request lowering and
  result normalization when that implementation slice is built.
- Settings/API tests showing only provider accounts with `web.search` appear
  as search provider choices, and only provider accounts with `web.fetch`
  appear as fetch provider choices.

No network tests should be required for ordinary unit validation.
