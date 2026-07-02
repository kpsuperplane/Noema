# System Errors Log Design

## Status

Approved design for adding a developer diagnostic system error log at
`${NOEMA_HOME:-$HOME/.noema}/errors.log`. No implementation has been done in
this spec. The next step is an implementation plan.

## Context

Noema currently resolves its local data directory through `NoemaPaths`.
`${NOEMA_HOME:-$HOME/.noema}` already owns config, embedded store files,
provider account homes, MCP setup secrets, and runtime files. Runtime and
provider failures are mostly represented as typed Rust errors and sanitized
before reaching the web UI or GraphQL clients.

That is right for product surfaces, but it makes developer diagnosis harder
when a provider, MCP server, persisted record, or Noema protocol boundary
returns data that violates Noema's assumptions. Those failures often need raw
payloads and full context to fix the code, prompt, schema, protocol adapter, or
parser that encountered them.

## Goals

- Add an append-only developer diagnostic log at `NOEMA_HOME/errors.log`.
- Capture system-level errors that likely require a Noema code, prompt, schema,
  protocol, parser, or adapter change.
- Preserve full raw payloads, raw excerpts, request/response bodies, tool
  arguments, provider responses, and parsing context without redaction or size
  caps.
- Keep existing user-facing error behavior and sanitized UI responses.
- Make diagnostic logging best-effort so logging failures do not replace the
  original runtime failure.
- Use a machine-readable format that is easy to append, grep, and parse.

## Non-Goals

- Do not add a user-facing error log UI in this slice.
- Do not persist system errors in SurrealDB.
- Do not rotate, truncate, compact, redact, or cap `errors.log`.
- Do not turn `errors.log` into a general product audit trail.
- Do not log expected user/setup conditions such as missing credentials, denied
  approvals, disabled MCP servers, ordinary network unavailability, HTTP 429s,
  or normal provider refusals.
- Do not introduce a global logging framework solely for this feature.

## Product Decisions

- The log path is `NOEMA_HOME/errors.log`.
- The file is a developer diagnostic artifact and may contain secrets, OAuth
  adjacent provider material, private conversation text, provider payloads, MCP
  tool arguments, MCP tool results, and raw transport responses.
- Events are written as JSON Lines: one JSON object per line.
- Diagnostic events are explicit. A subsystem writes an event only when it can
  classify the failure as a system-level error.
- Product behavior does not change after a diagnostic event is written. The
  original typed error, sanitized setup result, or existing recovery path still
  controls user-visible behavior.

## System-Level Error Definition

A system-level error is an unexpected condition that likely requires changing
Noema code or Noema-owned assumptions to fix. Examples include:

- Provider output cannot be parsed as the required Noema response envelope.
- Responses API JSON or SSE payloads violate the parser's expected shape.
- A provider response contains no usable `output_text` where Noema requires
  assistant output.
- MCP `tools/list` metadata is malformed.
- A calibrated MCP tool call returns data Noema cannot parse or interpret.
- Persisted enum or schema values fail to parse from the embedded store.
- Runtime state reaches an impossible path such as an agent-initiated onboarding
  response with no assistant text.
- A protocol bridge returns a structurally invalid envelope.

Excluded examples:

- Missing credentials.
- Provider authentication failure.
- User denied an approval.
- MCP server disabled or not calibrated.
- MCP server needs authentication.
- Provider rate limit.
- Provider refusal returned through a supported refusal path.
- User-entered invalid config caught by normal validation.
- Ordinary network outage.

## Architecture

Add a small Noema-owned diagnostic logger rather than a global logging
subscriber.

`NoemaPaths` adds:

```rust
pub fn errors_log_path(&self) -> PathBuf
```

A new `system_errors` module owns:

- `SystemErrorLogger`
- `SystemErrorEvent`
- append/create file behavior
- JSONL serialization
- test helpers for reading log events

`SystemErrorLogger` is cheap to clone and holds the resolved log path. Runtime
startup constructs it after Noema home initialization and passes it to runtime
components that need diagnostic instrumentation. Where passing the handle would
create unnecessary churn, code may construct a logger from `NoemaPaths` at the
boundary that already owns paths.

The first implementation should keep the logger independent from SurrealDB so
store failures can still be diagnosed.

## Event Schema

Each line in `errors.log` is a JSON object with stable top-level fields:

```json
{
  "timestamp": "2026-07-02T12:34:56.789Z",
  "severity": "error",
  "category": "provider_malformed_response",
  "message": "provider did not return a Noema structured response envelope",
  "context": {
    "conversation_id": "conversation:...",
    "turn_id": "conversation_turn:...",
    "provider_kind": "codex",
    "model": "gpt-5.5",
    "request_id": "resp_..."
  },
  "error_chain": [
    "malformed provider response: provider did not return a Noema structured response envelope"
  ],
  "raw": {
    "provider_text": "{... full raw provider payload ...}"
  }
}
```

Field meanings:

- `timestamp`: UTC timestamp at write time.
- `severity`: always `error` in the first slice.
- `category`: stable machine-readable category.
- `message`: concise human-readable diagnostic summary.
- `context`: structured context specific to the subsystem.
- `error_chain`: ordered error strings where source errors are available.
- `raw`: uncapped, unredacted payloads and excerpts needed to reproduce or
  understand the failure.

The schema should allow extra fields in `context` and `raw` without requiring a
central enum variant for every subsystem.

## Initial Instrumentation

Start with high-signal boundaries:

### Provider Response Parsing

Log when:

- Responses JSON parsing fails.
- Responses SSE JSON or UTF-8 parsing fails.
- SSE terminal output cannot be converted into `ResponsesResponse`.
- `ResponsesResponse::output_text` finds no output text or only unsupported
  content where text is required.
- `required_output_items_from_text` rejects provider output.
- Strict Noema response envelope parsing or validation fails.

Raw fields should include the full provider text, HTTP body, SSE event, SSE
chunk, or assembled output value available at the failure boundary.

### MCP Metadata And Tool Calls

Log when:

- `tools/list` result is not an object.
- `tools/list.tools` is missing or not an array.
- Tool name, schemas, annotations, output schema, or cursor shape is malformed.
- MCP SDK metadata conversion fails.
- A runtime MCP tool call fails because the transport result is malformed or
  cannot be interpreted by Noema.

Raw fields should include the raw result object, server id, transport kind, tool
name, call arguments, and returned payload where available.

### Runtime And Store Invariants

Log when:

- A runtime path reaches an impossible state that is not caused by user input.
- Agent onboarding receives a provider response with no assistant text.
- Persisted enum values or schema-shaped JSON fail to parse from the embedded
  store.
- Provider bridge protocol envelopes are invalid.

These events should include conversation id, turn id, provider kind, model,
store table/field where applicable, and any raw row or protocol payload already
available to the caller.

## Error Handling

Diagnostic logging is best-effort:

- A failed diagnostic write must not panic.
- A failed diagnostic write must not mask the original provider, MCP, store, or
  runtime error.
- Production code may ignore logging errors after attempting the write.
- Unit tests may use APIs that return logging failures so filesystem behavior
  can be verified.

The logger should create the log file on first append. Noema home initialization
already creates the root directory; if a caller reaches the logger before the
root exists, the logger may create the parent directory as a defensive measure.

## Privacy And Security

`errors.log` is developer-diagnostic only. It intentionally does not redact,
truncate, or cap raw content.

The file may contain:

- secrets or tokens included in provider/tool payloads
- OAuth-adjacent provider material
- private user conversation text
- tool arguments and tool responses
- raw HTTP/SSE/MCP protocol bodies
- filesystem paths and local environment details

This is acceptable for the first slice because the file lives inside the
user-owned Noema home and is meant to debug failures that sanitized product
surfaces cannot explain. Future UI or export features must treat this file as
sensitive by default.

## Testing

Logger tests should cover:

- `NoemaPaths::errors_log_path()` resolves to `<root>/errors.log`.
- Appending creates `errors.log` when missing.
- Multiple events append as JSONL without overwriting.
- Raw payload fields round-trip exactly, including multiline strings and large
  values.
- Logging failure does not panic or mask the original caller error.

Instrumentation tests should cover:

- Provider strict-envelope parse failure writes category, context, error string,
  and raw provider text.
- Responses SSE malformed payload writes raw SSE/HTTP material available at the
  failure boundary.
- MCP malformed `tools/list` metadata writes raw result and setup context.
- Runtime invariant failures write conversation, turn, provider, and model
  context where available.

Validation after implementation should include focused Rust tests for the
changed modules, then the repository's standard backend validation for a Rust
change:

```text
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
```

