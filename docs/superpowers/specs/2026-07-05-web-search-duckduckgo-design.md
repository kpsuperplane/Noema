# Rust-Only Web Search Design

## Goal

Give Noema a working `web.search` tool out of the box, without requiring an
API key, Python service, MCP setup, browser session, or page-fetching feature.

The first slice is pure internet search:

- The model proposes a search query.
- Noema shows the query in normal chat tool markers.
- Noema executes a Rust-owned best-effort DuckDuckGo public search adapter.
- Noema returns normalized result metadata only: title, URL, snippet, and rank.
- Noema does not fetch or read result pages.

## Non-Goals

- No browser automation, clicking, cookies, screenshots, or logged-in sessions.
- No page fetch, scrape, readability extraction, crawling, or content indexing.
- No MCP-based web search.
- No Python runtime dependency.
- No durable raw search result archive in the first slice.
- No approval gate for ordinary search queries.

## Provider Ladder

Noema should separate the stable `web.search` capability from the concrete
search provider.

Initial providers:

| Provider | Role | Notes |
| --- | --- | --- |
| `duckduckgo_public` | Default out-of-box provider | Rust-only, no API key, best-effort, unofficial public search path. |
| `brave_search` | Future recommended hosted provider | API key, provider contract, better reliability, simple pricing. |
| `websurfx` | Future free Rust self-hosted candidate | Rust service, optional endpoint adapter after evaluation. |
| `searxng` | Future free self-hosted candidate | Mature, but Python service; optional endpoint adapter only. |

If no configured provider exists, Noema uses `duckduckgo_public`. A configured
provider can override it later without changing the model-visible tool.

## Tool Contract

Expose one first-party native tool:

```text
web.search
```

Input schema:

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "minLength": 1,
      "maxLength": 500,
      "description": "The exact internet search query to send to the configured search provider."
    },
    "reason": {
      "type": "string",
      "maxLength": 500,
      "description": "Brief reason this search is useful for the current response."
    },
    "max_results": {
      "type": "integer",
      "minimum": 1,
      "maximum": 10,
      "description": "Maximum number of search results to return."
    }
  },
  "required": ["query"],
  "additionalProperties": false
}
```

Output shape:

```json
{
  "provider": "duckduckgo_public",
  "query": "rust reqwest current version",
  "results": [
    {
      "rank": 1,
      "title": "reqwest - crates.io",
      "url": "https://crates.io/crates/reqwest",
      "snippet": "..."
    }
  ]
}
```

The model receives enough data to cite or reason about search results, but not
raw provider payloads or fetched page bodies.

## Security Model

`web.search` is a special first-party capability, distinct from ordinary MCPs.

The query is a trusted-provider outbound disclosure, not normal untrusted
export:

```text
data_flow_class: trusted_external_search_query
destination_trust: trusted_search_provider
approval_default: not_required
audit_level: normal
visible_marker: true
```

This means:

- Search is available by default and does not ask for approval.
- The exact query is visible in normal chat tool markers.
- Noema records provider, query, result count, and timestamp for inspection.
- Search results re-enter as `external_content` and `tool_output`.
- Search result text is data, not instructions; it cannot expand scopes,
  authorize actions, write memory, or alter policy.

The default DuckDuckGo public adapter should also carry a reliability label:

```text
provider_contract: best_effort_public
```

That label is visible in developer/audit surfaces and can be summarized in
Settings, but it should not make normal search feel broken or gated.

## Runtime Architecture

Add a first-party execution kind to the native tool plane, for example:

```rust
NoemaToolExecution::WebSearch
```

The model-facing tool is built alongside local builtins and calibrated MCP
tools, but dispatch should go through a first-party web search executor rather
than the MCP gateway.

Conceptual module shape:

```text
provider/tools.rs
  NoemaToolExecution::WebSearch

daemon/runtime/model_tools.rs
  includes web.search in native-capable model tools

search/
  mod.rs
  duckduckgo.rs
  types.rs
```

Core trait:

```rust
trait SearchProvider {
    async fn search(&self, request: SearchRequest) -> Result<SearchResponse, SearchError>;
}
```

The initial `DuckDuckGoPublicSearchProvider` should use Noema's Rust HTTP stack,
bounded timeouts, bounded response size, stable user agent, and deterministic
HTML/result parsing. If the public endpoint changes or rate-limits Noema, the
tool returns a structured failed tool result that the model can explain.

## Chat Marker

Normal transcript marker:

```text
Searched the web for: <query>
```

Expanded marker fields:

```text
Provider: DuckDuckGo public search
Reliability: best effort
Results: N
```

Do not show raw HTML, raw provider response payloads, request headers, or
provider diagnostics in the default chat marker. System-level diagnostics can
go to the existing errors log if the adapter encounters parser or protocol
failures that suggest Noema code needs attention.

## Persistence

Persist:

- query
- provider id
- provider reliability label
- result count
- success/failure
- compact normalized result list if needed for transcript replay

Do not persist:

- raw HTML
- raw provider JSON
- fetched page bodies
- cookies or browser/session state

If future provider terms restrict result storage, the provider adapter can mark
results as transient. The first DuckDuckGo public adapter should keep storage
minimal because the integration is best-effort and unofficial.

## Error Handling

Return structured tool failures for:

- empty query after trimming
- query too long
- unsupported arguments
- HTTP timeout
- rate limit or blocking response
- parse failure

No-result searches should return a successful empty result set with a stable
`no_results` summary. They are not runtime failures.

Provider failures should still persist as failed tool results and feed back to
the model for same-turn recovery, following the existing local/MCP tool result
continuation pattern.

## Testing

Unit tests:

- `web.search` tool spec matches runtime argument parser.
- Invalid arguments produce safe structured tool errors.
- DuckDuckGo parser extracts title, URL, and snippet from saved fixtures.
- Parser ignores scripts, forms, hidden controls, and non-result links.
- Result count is bounded by `max_results`.
- Empty/no-result fixtures produce successful empty results or a stable
  `no_results` payload, not a crash.

Runtime tests:

- Native-capable provider receives `web.search`.
- Provider-proposed `web.search` call executes through the web search executor.
- Tool call and tool result transcript items show the visible query marker.
- Tool result continuation sends normalized search results back to the model.
- Search result text is treated as external tool output, not as local policy.

Network tests should not be required for ordinary unit validation. Live
DuckDuckGo checks can be developer-only or ignored by default because the
public endpoint is best-effort.

## Implementation Phases

### Phase 1: Canonical Tool And Executor

- Add `NoemaToolExecution::WebSearch`.
- Add `web.search` tool spec and parser.
- Add search provider trait and normalized result types.
- Add DuckDuckGo public provider with fixture-backed parser tests.

### Phase 2: Runtime Wiring

- Include `web.search` in model tools for native-capable providers.
- Dispatch `web.search` from local tool execution.
- Persist visible tool call/result activity with query and compact result
  summaries.
- Feed normalized results back through native tool-result continuation.

### Phase 3: Settings And Future Providers

- Expose provider status in Settings after the first backend slice works.
- Add Brave Search API as a configured hosted provider.
- Evaluate Websurfx as the preferred free Rust self-hosted provider.
- Add SearXNG only as an optional endpoint adapter, not a default dependency.

## Open Questions

- Should `reason` be required after the first slice, or remain optional to keep
  the tool easy for models to call?
- Should Noema expose a user-visible reliability badge in every web search
  marker, or only when using `best_effort_public` providers?
- Should `duckduckgo_public` use one endpoint strategy initially, or implement
  a small ordered fallback between public HTML endpoints?
