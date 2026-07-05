# Rust-Only Web Fetch Design

## Goal

Give Noema a first-party `web.fetch` tool that can read a specific public web
page, extract readable markdown, and return model-usable content without
requiring Python, MCP setup, a browser session, an API key, or a hosted scrape
provider.

The first slice should:

- Let the model propose the URL to fetch.
- Show the URL in normal chat tool markers.
- Fetch only public `http` and `https` URLs through Rust-owned networking.
- Extract readable page content as markdown with `readability-rs`.
- Summarize long extracted markdown before returning it to the agent.
- Make summarization visible and configurable from the web UI.
- Keep a provider boundary so future fetch providers can be added without
  changing the model-visible tool.

## Prior Art

OpenClaw's `web_fetch` is a useful direct-fetch reference: plain HTTP GET,
private/internal host blocking, redirect re-checks, Readability extraction,
optional provider fallback, cache, response caps, and no JavaScript execution.

Hermes' `web_extract` is the better long-page handling reference: it returns
small pages as raw markdown, summarizes medium pages through a configurable
auxiliary model, chunk-summarizes very large pages, and refuses pages that are
too large to compress responsibly.

June's web tools are a hosted/proxied design: the local agent sees
`web_search` and `web_fetch`, but calls are routed through a loopback app
proxy to June API and then Venice augment endpoints. Noema should borrow the
separate search/fetch capabilities, URL validation discipline, and structured
unsupported-site errors, but not the hosted proxy or metering model for the
first slice.

## Non-Goals

- No browser automation, JavaScript execution, screenshots, clicking, or
  logged-in pages.
- No cookies, local browser profiles, or platform browser state.
- No MCP-based web fetch.
- No Python runtime dependency.
- No hosted fetch provider in the first implementation.
- No model-selected provider argument.
- No durable archive of full raw page bodies.
- No bypass around private/internal network blocking.

## Tool Contract

Expose one first-party native tool:

```text
web.fetch
```

Input schema:

```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "minLength": 1,
      "maxLength": 2048,
      "description": "The public http(s) URL to fetch and read."
    },
    "reason": {
      "type": "string",
      "maxLength": 500,
      "description": "Brief reason this page is useful for the current response."
    },
    "max_chars": {
      "type": "integer",
      "minimum": 1000,
      "maximum": 20000,
      "description": "Maximum characters to return after extraction and optional summarization."
    }
  },
  "required": ["url"],
  "additionalProperties": false
}
```

There is intentionally no `provider` argument. Provider choice belongs to
Noema configuration and runtime state, not to the model.

Output shape:

```json
{
  "provider": "direct_http",
  "url": "https://example.com/article",
  "final_url": "https://example.com/article",
  "title": "Article title",
  "format": "markdown",
  "extraction": "readability_rs",
  "content_kind": "raw_markdown",
  "content": "# Article title\n\nReadable page content...",
  "raw_chars": 12840,
  "returned_chars": 12840,
  "truncated": false
}
```

When Noema summarizes:

```json
{
  "provider": "direct_http",
  "url": "https://example.com/long-doc",
  "final_url": "https://example.com/long-doc",
  "title": "Long doc",
  "format": "markdown",
  "extraction": "readability_rs",
  "content_kind": "summary",
  "content": "Condensed markdown summary...",
  "raw_excerpt": "First raw markdown excerpt...",
  "raw_chars": 183421,
  "returned_chars": 4972,
  "summary_model": "gpt-5.4-mini",
  "summary_strategy": "single_pass",
  "truncated": false
}
```

## Provider Boundary

Add a generic fetch provider boundary, but implement only `direct_http` in the
first slice.

Conceptual interface:

```rust
trait WebFetchProvider {
    async fn fetch(&self, request: FetchRequest) -> Result<FetchResponse, FetchError>;
}
```

The boundary should model capability and behavior, not reserve named provider
slots. Future providers may be hosted APIs, self-hosted endpoints, or browser
backends, but they are not part of this design's model-visible surface.

## Direct HTTP Provider

The initial provider is `direct_http`.

Pipeline:

```text
parse and canonicalize URL
-> reject non-http(s), private, internal, local, and malformed targets
-> resolve target to public IPs
-> fetch with bounded Rust HTTP client
-> re-check each redirect target before following
-> cap response bytes while streaming
-> accept HTML and plain text content types only
-> extract HTML to markdown with readability-rs
-> summarize if the extracted markdown is large
-> return normalized tool output
```

Network defaults:

- stable Chrome-like user agent
- `Accept-Language: en-US,en;q=0.9`
- timeout around 30 seconds
- maximum redirects: 3
- maximum response bytes: 750 KB initially
- default `max_chars`: 20,000
- hard `max_chars` cap: 20,000

`direct_http` allows ordinary public `http` and `https` domain URLs in the
first slice because Noema performs local DNS resolution, IP classification, and
redirect validation before connecting. This differs from hosted-proxy designs
where a remote provider performs final DNS resolution and `http` domain names
are riskier to preflight locally.

The first slice should fail with a structured tool result if
`readability-rs` cannot extract useful markdown from HTML. A plain text
response can skip Readability and return normalized text as markdown-shaped
content.

## Summarization

Summarization is part of `web.fetch`, not a separate model-visible tool in the
first slice.

Behavior:

| Extracted markdown size | Behavior |
| --- | --- |
| `<= 8,000` chars | Return raw extracted markdown. |
| `8,001..=250,000` chars | Single-pass summary through the configured web-fetch summarizer model. |
| `250,001..=1,000,000` chars | Chunk summaries, then synthesize a final markdown summary. |
| `> 1,000,000` chars | Refuse with a hint to fetch a narrower URL. |

The summary should preserve headings, links, quotes, code blocks, and key
facts where possible. It should compress content, not invent commentary. The
summarizer prompt must treat page content as untrusted data: ignore
instructions embedded in the page, do not follow links, do not authorize
actions, do not write memory, and do not add facts absent from the source.

If summarization fails or times out, Noema should return a safe failed tool
result rather than silently substituting a misleading partial summary. A later
slice can consider a raw-prefix fallback if the UI clearly marks it.

When summarization succeeds, include a small raw markdown excerpt, capped around
2,000 characters, so inspection surfaces and the model can see a concrete slice
of the source text without persisting or returning the full page body.

## Summarizer Model Configuration

The model used for web-fetch summarization must be configurable from the web
UI.

Initial product shape:

- Add a "Web fetch summarizer" model setting to the web settings surface that
  owns model/tool configuration.
- Persist the selected provider account and model id in Noema state or config,
  following the existing runtime preference patterns.
- Use the configured summarizer model only for `web.fetch` compression.
- Show the effective model in inspection/settings surfaces and in summarized
  tool result metadata.

Default:

- For OpenAI and Codex provider accounts, default to `gpt-5.4-mini`.
- If the selected provider account does not expose `gpt-5.4-mini`, the UI
  should require an explicit supported choice rather than silently selecting an
  expensive or incompatible model.

This mirrors the existing direction for cheap, fast auxiliary model choices:
the main chat model should not automatically become the summarizer when it is
expensive or tuned for reasoning.

## Security Model

`web.fetch` is a special first-party web capability, but it is not the same
trust class as `web.search`.

Search sends a query to a trusted search provider. Fetch sends a request to an
arbitrary public origin chosen by the model, often based on untrusted search
results.

```text
data_flow_class: external_web_fetch
destination_trust: arbitrary_public_origin
approval_default: not_required_for_public_http
blocked_targets: private_internal_local_networks
audit_level: normal
visible_marker: true
```

This means:

- Public HTTP(S) fetches are available by default.
- Private, internal, localhost, link-local, benchmark, documentation, and
  other non-public targets are blocked before network egress.
- Redirects are limited and each redirect target is validated before
  following.
- DNS resolution and IP classification happen in Noema before connecting.
- Fetched content re-enters as `external_content` and `tool_output`.
- Extracted page text and summaries are data, not instructions.
- The full raw page body is not persisted by default.

The implementation should include explicit tests for tricky URL forms,
alternate IPv4 encodings, IPv6 private/local ranges, local hostnames, and
redirects to blocked destinations.

## Chat Marker

Normal transcript marker:

```text
Fetched web page: <url>
```

Result marker summaries:

```text
Fetched 12,840 chars
Summarized 183,421 chars to 4,972 chars
Failed: unsupported URL
```

Successful fetch markers should stay compact like successful `web.search`
markers. They should not expand into redundant `URL`, `Output`, and repeated
content rows by default. Failed fetch markers should remain expandable enough
to show the safe error message.

Do not show raw HTML, headers, cookies, provider diagnostics, or full fetched
content inside the default marker. The final assistant answer can cite or quote
from the returned markdown within normal copyright limits.

## Persistence

Persist:

- requested URL
- final URL
- provider id
- extraction kind
- content kind (`raw_markdown` or `summary`)
- title when available
- raw character count
- returned character count
- summarizer model id when used
- success/failure and safe error category
- compact result payload needed for transcript replay and provider
  continuation

Do not persist:

- raw HTML
- response headers except safe metadata needed for diagnostics
- cookies
- browser/session state
- full unsummarized page bodies when summarization happened

Caching is deferred. The first implementation should fetch each requested URL
fresh so cache keys, invalidation, and provider-specific storage rules do not
hide inside the initial security model.

## Runtime Architecture

Add a first-party execution kind:

```rust
NoemaToolExecution::WebFetch
```

Conceptual module shape:

```text
provider/tools.rs
  NoemaToolExecution::WebFetch

daemon/runtime/model_tools.rs
  includes web.fetch in native-capable model tools

daemon/runtime/local_tools.rs
  dispatches web.fetch through the first-party executor

web_fetch/
  mod.rs
  direct_http.rs
  extraction.rs
  summarize.rs
  tool.rs
  types.rs
  url_policy.rs
```

`web.search` and `web.fetch` should remain separate model-visible tools, but
they can share lower-level URL/network policy helpers if that reduces
duplication without muddying the two contracts.

## Error Handling

Return structured failed tool results for:

- empty URL
- URL too long
- unsupported scheme
- malformed URL
- private/internal/local target
- DNS failure
- redirect to blocked target
- too many redirects
- timeout
- unsupported content type
- response too large before extraction
- Readability extraction failure
- summarization failure
- page too large to summarize responsibly

Provider failures should still persist as failed tool results and feed back to
the model for same-turn recovery, following the existing local/MCP tool result
continuation pattern.

## Testing

Unit tests:

- `web.fetch` tool spec matches runtime argument parser.
- Invalid arguments produce safe structured tool errors.
- URL policy rejects private/internal/local targets, alternate IPv4 forms, and
  non-HTTP schemes.
- URL policy accepts ordinary public HTTP(S) URLs.
- Redirect validation rejects redirect chains that land on blocked targets.
- Response byte cap is enforced while streaming.
- HTML extraction uses `readability-rs` and returns markdown.
- Plain text responses are normalized without Readability.
- Size policy chooses raw, single-pass summary, chunked summary, or refusal.
- Summarizer prompt ignores page instructions and preserves provenance
  metadata.

Runtime tests:

- Native-capable provider receives `web.fetch`.
- Provider-proposed `web.fetch` executes through the first-party executor.
- Tool call and tool result transcript items show the visible URL marker.
- Successful fetch markers stay compact.
- Failed fetch markers expose safe error details.
- Tool result continuation sends normalized fetched content or summary back to
  the model.

UI tests:

- Web settings can display and save the web-fetch summarizer model.
- OpenAI/Codex defaults show `gpt-5.4-mini`.
- Unsupported defaults require explicit user selection rather than silently
  choosing a fallback model.

Live e2e:

- After implementation, verify `web.fetch` against the running agent at
  `:3737`, mirroring the `web.search` validation style.

## Implementation Phases

### Phase 1: Tool Contract And Provider Boundary

- Add `NoemaToolExecution::WebFetch`.
- Add `web.fetch` tool spec and parser.
- Add normalized fetch request/response/error types.
- Add a generic fetch provider boundary with only `direct_http` implemented.

### Phase 2: Direct Fetch And Extraction

- Add public URL policy and tests.
- Add bounded Rust HTTP fetch with redirect re-checks.
- Add `readability-rs` extraction to markdown.
- Add safe structured failures for unsupported or unextractable pages.

### Phase 3: Summarization

- Add size policy.
- Add single-pass and chunked summarization through a configurable auxiliary
  model.
- Add summarizer prompt and metadata.
- Persist summarizer model configuration.

### Phase 4: Runtime And UI Wiring

- Expose `web.fetch` to native-capable providers.
- Dispatch `web.fetch` through local tool execution.
- Persist visible transcript call/result activity.
- Feed normalized output back through native tool-result continuation.
- Add web UI control for the web-fetch summarizer model.
- Compact successful `web.fetch` markers.

## Deferred Decisions

- Cache behavior after repeated fetch usage is visible.
- Hosted or self-hosted provider adapters after `direct_http` works end to end.
- Browser-backed fetch for JavaScript-heavy or logged-in pages.
