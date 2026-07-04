# Native Tool Plane Design

## Problem Statement

Recent primary-chat replay showed a painful Notion/Dex failure loop:

- The model selected the right general capability but sent invalid Notion MCP
  payloads.
- The runtime surfaced repeated schema failures and asked the user for
  information it already had.
- The web replay loaded too much raw activity metadata, making a bad tool loop
  feel worse.

The root tool-call problem is not that the model is too weak. Noema is asking a
frontier model to generate executable tool calls inside a Noema-specific JSON
response object shaped by prose. That wastes the provider's native structured
tool-use machinery.

OpenAI Codex's open-source harness avoids this shape by deriving model-visible
tool specs and executable handlers from the same registry, sending those specs
through the provider's native tool field, then dispatching typed response tool
items. Validation still exists, but it is a correctness boundary rather than
the primary way to steer payload construction.

References:

- <https://github.com/openai/codex>
- <https://github.com/openai/codex/blob/main/codex-rs/core/src/session/turn.rs>
- <https://github.com/openai/codex/blob/main/codex-rs/core/src/client.rs>
- <https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/spec_plan.rs>
- <https://github.com/openai/codex/blob/main/codex-rs/core/src/tools/registry.rs>
- <https://github.com/openai/codex/blob/main/codex-rs/tools/src/responses_api.rs>

## Goals

- Move executable tool calls out of Noema's assistant JSON response object and
  into provider-native tool calls wherever the provider supports them.
- Keep Noema's tool model provider-neutral so OpenAI/Codex, Claude, Apple
  Foundation Models, and future providers share one canonical runtime contract.
- Derive the model-visible tool schema and executable runtime dispatch from the
  same Noema registry entry.
- Normalize MCP and builtin tool schemas before provider lowering.
- Fail closed or visibly degrade for providers that cannot support native tools.
- Keep replay cleanup as a sibling track so tool failures do not flood the
  frontend with unbounded raw payloads.

## Non-Goals

- Do not clone Codex's full harness or make Codex the runtime owner.
- Do not make a Notion-specific payload fixer.
- Do not silently keep full MCP tool use on the old envelope path for providers
  without native tools.
- Do not add backwards compatibility for pre-V1 provider contracts unless the
  user explicitly requests it.
- Do not redesign the web transcript UI in this slice.

## Current Shape

The current harness has already improved the assistant response object:

- `response_status`
- `responses[]`
- `tool_calls[]`
- `memory_proposals[]`

That is a better contract than an ordered mixed output array, but executable
tool calls are still model-authored JSON inside a Noema response object. The
Capability Gateway owns rich MCP metadata and runtime dispatch, but provider
requests do not expose the MCP input schema as native tool specs. The prompt
therefore carries tool affordances as compact text rows and rules rather than
provider-enforced tool schemas.

This design keeps the response object useful for assistant text, structured
cards, and memory proposals. It removes executable tool calls from that object
for native-tool-capable providers.

## Canonical Tool Model

Introduce provider-neutral tool types owned by Noema:

```rust
pub struct NoemaToolSpec {
    pub name: ToolName,
    pub description: String,
    pub input_schema: NoemaToolSchema,
    pub output_schema: Option<NoemaToolSchema>,
    pub kind: NoemaToolKind,
    pub execution: NoemaToolExecution,
    pub policy: ToolExposurePolicy,
}

pub struct NoemaToolCall {
    pub id: ToolCallId,
    pub name: ToolName,
    pub arguments: serde_json::Value,
    pub provider_call_id: Option<String>,
}

pub struct NoemaToolResult {
    pub call_id: ToolCallId,
    pub status: ToolResultStatus,
    pub output: serde_json::Value,
    pub summary: Option<String>,
    pub error: Option<ToolResultError>,
}
```

`NoemaToolSpec` is the single source for both:

- the provider-visible tool schema
- the executable runtime handler

Tool sources:

- local builtin tools such as `search_memory` and `update_own_name`
- calibrated MCP tools exposed through the Capability Gateway
- future first-party app tools

The registry should not have a separate "prompt summary" source that can drift
from executable schema. A compact model-facing summary may be derived for
fallback or logging, but it is not the authoritative tool contract.

## Provider Capabilities

Each provider adapter declares the tool features it supports:

```rust
pub struct ProviderToolCapabilities {
    pub native_tools: bool,
    pub parallel_tool_calls: bool,
    pub tool_choice: ProviderToolChoiceSupport,
    pub schema_dialect: ProviderToolSchemaDialect,
    pub strict_schema: bool,
    pub custom_tools: bool,
    pub native_tool_results: bool,
}
```

Examples:

- OpenAI/Codex Responses: native tools, function-style parameters, optional
  strictness, native tool-result continuation items.
- Claude: native tools, Anthropic-specific schema and continuation shape.
- Apple Foundation Models: may support a constrained local tool interface, no
  remote tool API, or no native tool calls at first.
- Fallback text providers: no native tools.

Capability declarations drive exposure. If a turn needs MCP tool use and the
selected provider cannot support native tools, Noema should choose an explicit
fallback mode instead of quietly using the old full-envelope tool path.

## Provider Request Contract

Extend the provider contract so tool specs are first-class request fields:

```rust
pub struct GenerateRequest {
    pub conversation_id: Option<ConversationId>,
    pub input: ProviderInput,
    pub instructions: String,
    pub response_format: Option<ProviderResponseFormat>,
    pub tools: Vec<NoemaToolSpec>,
    pub tool_choice: NoemaToolChoice,
    pub parallel_tool_calls: bool,
}
```

Provider adapters are responsible for:

- lowering `NoemaToolSpec` into provider-native request fields
- parsing native provider tool-call items into `NoemaToolCall`
- formatting `NoemaToolResult` back into provider-native continuation input
- reporting unsupported schema features precisely

The runtime owns:

- eligibility and policy filtering
- canonical schema validation before execution
- Capability Gateway dispatch
- transcript and audit persistence
- continuation loop limits

## Schema Normalization

Add a normalization layer between Noema/MCP schemas and provider-specific
schemas:

```rust
NoemaToolSchema -> NormalizedToolSchema -> ProviderToolSchema
```

Responsibilities:

- require object-shaped input roots
- preserve `required` and property descriptions where supported
- normalize nullable shapes
- handle or reject unsupported `oneOf`, `anyOf`, `allOf`, and recursive shapes
- prune unsupported metadata keywords
- constrain `additionalProperties` according to provider capability
- preserve enums where the provider supports them
- record lossy transformations for diagnostics

Schema normalization must be deterministic and covered by fixtures from real
MCP servers:

- Notion
- Dex
- Google Calendar
- Google Drive
- Slack-like broad schemas if available

The fixture tests should assert both sides:

- the provider-visible schema is accepted by the provider dialect rules
- Noema's canonical validator still rejects invalid payloads before execution

## Runtime Flow

Per turn:

1. Resolve provider and model for the agent.
2. Build eligible canonical tools from local builtins and calibrated MCP tools.
3. Filter tools through provider capabilities and Noema policy.
4. Lower canonical specs through the provider adapter.
5. Send native tool specs with the provider request.
6. Parse provider output into assistant response content and native tool calls.
7. Persist assistant response content.
8. Validate each `NoemaToolCall` against the canonical schema.
9. Dispatch through the local builtin executor or Capability Gateway.
10. Persist structured tool start/result activity.
11. Continue the provider turn with native tool-result items when supported.
12. Stop when the provider returns a final assistant response or the runtime
    continuation limit is reached.

Tool-call validation remains mandatory. The difference is that the model gets
the schema through the provider's tool channel before generation, so invalid
payloads should be exceptional instead of normal recovery input.

## Response Object After This Change

The Noema response object remains useful, but it no longer carries executable
tool calls for native-capable providers.

Target provider response object:

```json
{
  "response_status": "final",
  "responses": [
    {
      "kind": "text",
      "phase": "final_answer",
      "text": "Done."
    }
  ],
  "memory_proposals": []
}
```

Intermediate tool turns use provider-native tool-call items, not:

```json
{
  "tool_calls": [
    {
      "name": "notion-create-pages",
      "payload": {}
    }
  ]
}
```

For providers that require a single structured assistant output and cannot mix
native tools with a custom response schema, the adapter decides the provider
specific shape and reports the tradeoff through capabilities. Noema should not
force every provider into OpenAI's exact request format.

## Fallback Policy

Fallbacks must be explicit:

- `NoTools`: provider can answer but no model-visible tools are exposed.
- `BuiltinOnlyEnvelope`: allow a small, audited local builtin set through the
  legacy envelope path if a provider lacks native tools.
- `LegacyEnvelope`: development-only compatibility mode, visibly degraded.
- `NativeRequired`: fail fast when the requested operation requires native
  tool calls and the selected provider cannot provide them.

Default policy for calibrated third-party MCP tools should be `NativeRequired`.
That prevents Noema from advertising high-risk external tools through a weaker
text JSON contract.

## Provider-Specific Lowering

### OpenAI/Codex

- Lower canonical tools to Responses API tool specs.
- Use provider-native function/custom tool-call response items.
- Feed tool results back as native tool result items.
- Keep Noema's structured assistant response format for assistant text and
  memory proposals if the provider supports combining that with tools.

### Claude

- Lower canonical tools to Anthropic tool definitions.
- Parse Anthropic tool-use blocks into `NoemaToolCall`.
- Continue with Anthropic tool-result blocks.
- Apply Claude-specific schema normalization, especially around unsupported
  JSON Schema features.

### Apple Foundation Models

- Declare capabilities honestly.
- If the local Foundation Models bridge supports native tool callbacks, map
  them into the same canonical contract.
- If not, expose no MCP tools by default and keep chat/memory behavior separate
  from third-party side effects.

## Replay Cleanup

Replay cleanup is a sibling design track, not the tool-plane root fix.

The recent painful chat loaded hundreds of visible items and large MCP activity
metadata blobs. The web and GraphQL surfaces should change to:

- bounded initial replay
- paginated older history
- compact visible tool summaries
- full raw payload access only through explicit drill-in
- server-side caps on default activity metadata returned to chat startup

This keeps a failed tool loop inspectable without making every page load pay
for raw MCP payloads.

## Test Strategy

Unit tests:

- canonical builtin tool specs validate good and bad payloads
- MCP schema normalization fixtures for Notion, Dex, Google Calendar, Google
  Drive, and broad-schema tools
- provider capability filtering hides unsupported tools
- provider adapters lower normalized schemas as expected
- provider adapters parse native tool-call items into `NoemaToolCall`
- canonical validation rejects malformed provider tool-call arguments

Runtime tests:

- OpenAI/Codex native tool call to local `search_memory`
- OpenAI/Codex native tool call to a calibrated MCP tool
- invalid native tool arguments produce a structured tool failure, not a crash
- provider without native tools does not receive calibrated MCP tools
- continuation loop preserves assistant text, tool start, tool result, and
  final answer ordering

Replay tests:

- primary conversation startup returns bounded visible items
- large activity metadata is summarized by default
- full payload drill-in remains possible through an explicit query

## Migration Phases

### Phase 1: Canonical Types And Capability Surface

- Add `NoemaToolSpec`, `NoemaToolCall`, `NoemaToolResult`, and provider tool
  capability types.
- Convert local builtin tool definitions into canonical specs.
- Add canonical schema validation tests.
- Keep existing runtime behavior until provider adapters consume the new types.

### Phase 2: Schema Normalization And MCP Fixtures

- Convert calibrated MCP tool metadata into canonical specs.
- Add schema normalization.
- Add fixture tests for real MCP tool schemas, starting with Notion and Dex.

### Phase 3: OpenAI/Codex Native Tool Adapter

- Send canonical tools through OpenAI/Codex native tool fields.
- Parse native tool-call items into `NoemaToolCall`.
- Continue turns with native tool results.
- Remove calibrated MCP tool calls from the Noema response object for this
  provider path.

### Phase 4: Fallback Policy

- Make non-native provider behavior explicit.
- Default third-party MCP tools to `NativeRequired`.
- Allow only audited builtin envelope fallbacks where product policy approves.

### Phase 5: Replay Cleanup

- Bound initial GraphQL replay.
- Summarize default activity metadata.
- Add full-payload drill-in.

### Phase 6: Additional Providers

- Add Claude lowering/parsing against the canonical contract.
- Add Apple/Foundation capability support when the bridge can honestly expose
  native tool calls.

## Open Questions

- Can OpenAI/Codex combine Noema's strict assistant response format with native
  tools in the exact shape Noema wants, or should assistant response parsing
  become provider-specific when tools are active?
- Should Noema allow parallel MCP tool calls initially, or start with serial
  execution even when the provider supports parallel calls?
- Which builtin local tools, if any, are safe enough for `BuiltinOnlyEnvelope`
  fallback on providers without native tools?
- Should schema-normalization diagnostics be persisted in `errors.log`, exposed
  in MCP Settings, or both?
