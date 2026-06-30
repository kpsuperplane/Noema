# Third-Party MCP Control Plane Design

## Summary

Noema should support third-party MCP servers without giving agents raw,
unmediated access to their tools. Every MCP server is added through a mandatory
configuration flow, and every tool call passes through Noema's Capability
Gateway before the MCP is invoked or any result is released to the agent.

The core policy model classifies each MCP tool along three axes:

| Axis | Meaning |
| --- | --- |
| Read | The tool may bring information from the MCP destination into Noema. |
| Write | The tool may mutate state inside the MCP destination. |
| Export | The tool may share information beyond the MCP destination's trust boundary. |

Each axis has one of four effective Noema-owned classifications:

| Classification | Meaning |
| --- | --- |
| `none` | The tool does not exercise this axis. |
| `trusted` | The tool operates only on data owned by trusted identities for the active context. |
| `untrusted` | The tool operates on data owned by untrusted identities. |
| `mixed` | Trust depends on the resolved subject, resource, or destination owner. |

MCP metadata, schemas, and annotations are useful setup hints, but they are not
authoritative for third-party tools. Noema stores the effective classification
only after user or admin review, or through an explicit built-in adapter rule.

## Goals

- Let users add and configure third-party MCP servers.
- Support stdio, remote HTTP/SSE, auth-backed MCPs, and future marketplace or
  install flows in the product model.
- Require tool calibration before any MCP tool is visible to agents.
- Classify tool behavior across read, write, and export axes.
- Treat ownership as a separate, configurable trust dimension.
- Support trusted identity selectors for email, phone, and domain in V1.
- Fail closed when classification, ownership, validation, or approval is
  missing.
- Quarantine MCP read results before model-visible release.
- Always require manual approval for export operations in V1.
- Surface MCP setup, calibration, trusted identities, approvals, and audit in
  the web Settings area.

## Non-Goals

- Do not let third-party MCP self-declarations become effective policy without
  review.
- Do not run setup probe tool calls. Listing tools and reading metadata is
  allowed; invoking tools during setup is not.
- Do not require non-technical users to edit JSON schemas or JSONPath rules.
- Do not auto-save owner extractors from runtime results, even when confidence
  is high.
- Do not add V1 scoped auto-approval for exports. Leave the model open for it
  later.
- Do not build backwards compatibility or migrations for pre-V1 schema changes.

## MCP Metadata Boundary

MCP `tools/list` can provide tool names, descriptions, input schemas, optional
output schemas, and annotations. Current MCP tool results can include
unstructured content blocks and optional `structuredContent`; when an
`outputSchema` is present, structured output should conform to it.

Noema should use those shapes for validation, display, and setup suggestions.
They do not answer semantic questions such as:

- Who owns the subject of this read?
- Whether a result is private or untrusted.
- Whether a write is destructive.
- Whether a sharing action exports trusted data to an untrusted destination.
- Whether server-provided annotations are honest.

Therefore MCP metadata is input to calibration, not authority. Noema's
Capability Gateway owns the final runtime decision.

Reference MCP specifications:

- https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle

## Trust And Ownership Model

### Trusted Identity Selectors

Humans configure trusted identity selectors globally. Governable scopes such as
workspace, project, and conversation may add or narrow those selectors.

V1 selector types:

- `email`
- `phone`
- `domain`

Selectors are typed and normalized before matching. Email and domain matching
must be exact after normalization. Phone numbers should be normalized to a
canonical form before comparison. Domain selectors are first-class entries, not
hidden fuzzy matching.

Effective trusted identities are resolved for each call from:

1. The current human's base trusted selectors.
2. Active scope additions or restrictions.
3. Explicit call or approval context, when present.

### Owner Resolution

Ownership is resolved from deterministic extractors, never from free-form model
judgment as the authority.

Supported extractor sources:

- Tool arguments.
- MCP `structuredContent`.
- MCP result metadata.
- Resource URI patterns.
- Built-in adapter rules for known tools or providers.

Setup may suggest extractor candidates from input schema, optional output
schema, tool descriptions, common field names, and built-in adapter knowledge.
Normal users confirm plain-language suggestions. Advanced users may inspect raw
schemas and configure JSON Pointer or JSONPath-style extractors.

If ownership cannot be configured from metadata or schema, the tool remains
blocked for agent use. A later explicit user-initiated calibration action or
real user-requested operation may reveal candidate owner fields, but Noema must
not run that call automatically. The result stays quarantined, and Noema asks
the user or admin before saving an extractor or releasing future results based
on it.

Unresolved ownership is not an acceptable steady state for an enabled
agent-usable tool.

## Export Semantics

Export means an MCP-mediated action causes information to leave the MCP
destination's trust boundary.

Examples:

- Sending an email through a mail MCP.
- Sharing a Google Doc with another account.
- Publishing or posting content externally.
- Copying data from a trusted private destination to an untrusted destination.

The fact that Noema sends a request to the MCP server is not itself "export" in
this model. Export is about the external effect the tool performs.

V1 rule: all export operations require manual approval, regardless of whether
source and destination owners are trusted. Future scoped auto-approval grants
can reuse the same policy model, but they are out of scope for V1.

## Setup And Calibration Flow

Adding an MCP server is a mandatory setup flow:

1. User starts from Settings -> MCPs.
2. User chooses transport and connection type:
   - Local stdio command.
   - Remote HTTP/SSE endpoint.
   - Auth-backed remote server.
   - Future marketplace/install entry.
3. Noema initializes the MCP connection and reads safe metadata.
4. Noema lists tools and records discovered metadata.
5. Noema shows a calibration checklist for every tool.
6. The user or admin confirms effective policy before enabling tools.

Discovery during setup is metadata-only. Noema must not call tools during setup
because a probe call could mutate state, send messages, or otherwise cause
external side effects.

For each tool, calibration records:

- Effective read/write/export classification.
- Whether each axis is `none`, `trusted`, `untrusted`, or `mixed`.
- Whether ownership is resolved before execution, after execution, or through a
  built-in adapter.
- The owner extractor when deterministic metadata makes one configurable.
- The owner identity type returned by the extractor.
- Enabled agents and scopes.
- Whether the tool is agent-visible.
- The user/admin who reviewed the calibration.
- The metadata version or fingerprint used during review.

Noema may propose defaults, but the setup UI must clearly separate:

- MCP-provided hints.
- Noema built-in adapter rules.
- Effective reviewed policy.

## Runtime Gateway Flow

When an agent proposes an MCP tool call, the provider output is treated as a
proposal. The Capability Gateway is the only component allowed to invoke the MCP
or release results to the agent.

Runtime steps:

1. Resolve the MCP server and tool.
2. Verify the tool is calibrated and enabled for the agent and active scopes.
3. Validate arguments against the MCP `inputSchema` where available.
4. Compute effective trusted identity selectors.
5. Resolve pre-call ownership where configured.
6. Evaluate preflight policy.
7. Create approval request when required.
8. Invoke the MCP only after policy permits execution.
9. Validate the MCP result shape, including `outputSchema` where available.
10. Place read results in quarantine.
11. Resolve post-call ownership where configured.
12. Examine, sanitize, deny, or release the result.
13. Persist transcript and audit records.
14. Return only released/sanitized result content to the provider continuation.

The V1 policy decision table is:

| Axis | Untrusted | Mixed | Trusted |
| --- | --- | --- | --- |
| Read | Examine | Examine | Allow |
| Write | Examine | Examine | Allow |
| Export | Examine | Examine | Examine |

`Examine` has a fixed top-level outcome set:

- `allow`
- `allow_sanitized`
- `require_approval`
- `deny`

Operation-specific evidence is attached below the fixed outcome. For example,
a read examination may include prompt-injection scan results, while an export
examination includes recipients, payload previews, and data source summaries.

## Read Quarantine

Read results are never released directly from the MCP server to the model.

The quarantine buffer stores:

- Raw MCP result reference or redacted payload.
- Validation status.
- Resolved owner identities.
- Effective owner trust.
- Prompt-injection or hostile-instruction scan results.
- Sanitization summary.
- Release decision.

Trusted reads can be released after validation and owner resolution. Untrusted
or mixed reads are examined first. If a mixed read needs post-call ownership
resolution, the result remains quarantined until ownership is resolved. If
ownership is unresolved, Noema does not release the result to the agent. It asks
the user/admin whether to configure an extractor from the real response or deny
release.

Normal transcript views should show concise activity such as "Read result
examined" or "Tool result withheld." Owner/admin inspection can reveal exact
fields, raw payload references, policy reasons, and extractor candidates.

## Writes And Exports

Writes are allowed without further examination only when the configured write
axis and resolved owner trust are trusted. Risky writes, untrusted writes, mixed
writes, and unresolved writes require examination. Depending on severity,
examination may deny the call or require manual approval.

Exports always require manual approval in V1. Approval cards must be
decision-ready without trusting model prose. They include:

- Requested action.
- MCP server and tool.
- Destination, recipient, share target, or publish target.
- Payload preview, diff, or payload hash.
- Data source summary.
- Source and destination owner identities.
- Trusted/untrusted/mixed labels.
- Active human, agent, workspace/project/conversation scope.
- What leaves the current trust boundary.
- Approval options: approve once, deny, or cancel.

Future auto-approval can be modeled as scoped grants over the same fields, but
that is not part of V1 behavior.

## Web Dashboard And Settings

MCP governance should surface under Settings, alongside existing Providers and
Agents settings.

Recommended Settings sections:

| Section | Purpose |
| --- | --- |
| Providers | Existing provider account status. |
| Agents | Existing registered-agent metadata. |
| MCPs | MCP connections, discovered tools, calibration, health, and auth. |
| Trusted Identities | Human and scope-level trusted email, phone, and domain selectors. |
| Approvals | Pending and historical approval requests. |
| Audit | Owner/admin inspection of tool decisions and quarantined releases. |

The first implementation can make `MCPs` the primary new tab and include
identity, approval, and audit links inline if separate tabs are not ready.
The product design should still reserve those sections so the IA does not
collapse all governance into one dense page.

### MCPs Tab

The MCPs tab should support:

- List of configured MCP servers.
- Add MCP flow for stdio and remote connections.
- Connection health and last metadata sync.
- Auth status where applicable.
- Discovered tool count.
- Calibration status:
  - Not discovered.
  - Needs review.
  - Blocked by unresolved ownership.
  - Ready.
  - Disabled.
- Per-tool detail cards with read/write/export classifications.
- Plain-language owner extractor summary.
- Advanced raw metadata/schema reveal.
- Enabled agents and scopes.

The add flow should be a wizard:

1. Connection.
2. Metadata discovery.
3. Tool calibration.
4. Trusted identity context.
5. Review and enable.

The wizard should never run MCP tool calls during setup.

### Trusted Identities Tab

The trusted identity surface should show:

- Human-global trusted identities.
- Scope-specific additions/restrictions.
- Selector type: email, phone, domain.
- Normalized value.
- Issuer and created time.
- Where the selector applies.
- Linked MCP calibrations that depend on it.

Actions:

- Add trusted email.
- Add trusted phone.
- Add trusted domain.
- Revoke selector.
- Preview access impact for a tool/scope.

### Approvals Tab

The approvals surface should show:

- Pending export approvals.
- Pending risky write approvals.
- Approval history.
- Requesting agent and active scope.
- MCP server/tool.
- Destination and payload preview.
- Trust labels and owner resolution.
- Decision and comments.

Export approvals should also appear inline in chat/work context when a run is
waiting.

### Audit And Inspection

The audit surface should support owner/admin drill-in:

- Proposed tool call.
- Effective calibration.
- Preflight decision.
- Owner extraction evidence.
- Quarantine status.
- Sanitization or denial reason.
- Approval request/result.
- MCP invocation result metadata.
- Agent-visible released result.

Normal user views should avoid leaking private raw payloads. Owner/admin reveal
should be explicit, scoped, and consistent with the existing governance
inspection direction.

## Backend Components

### `McpConnectionRegistry`

Owns persisted server configuration, auth state, discovered tools, metadata
fingerprints, health, and enabled status.

### `McpClientRuntime`

Owns MCP lifecycle and transports:

- stdio command process management.
- remote HTTP/SSE clients.
- metadata discovery.
- tool invocation after gateway approval.

### `ToolCalibrationStore`

Owns effective Noema policy for discovered tools:

- read/write/export classifications.
- owner extractors.
- enabled agents/scopes.
- review state.
- metadata fingerprint reviewed.

### `TrustedIdentityStore`

Owns human-global and scope-level trusted identity selectors.

### `CapabilityGateway`

Owns runtime mediation for MCP tools, current local tools, and future
connectors. It validates calls, asks policy, routes approvals, invokes adapters,
quarantines results, and releases only approved outputs.

### `OwnershipResolver`

Runs deterministic owner extraction from arguments, structured results,
metadata, URI patterns, and built-in adapter rules.

### `ExaminationRuntime`

Runs read examination, prompt-injection scanning, sanitization, denial
decisions, and approval routing.

### `ApprovalService`

Stores durable approval requests and decisions. V1 export operations always
route through this service.

### Audit And Transcript Integration

Persists compact transcript activities and owner/admin audit records. The
transcript should show understandable tool status without dumping raw private
payloads into normal chat.

## Data Model Sketch

Pre-V1 schema changes may rewrite tables directly. Exact SurrealDB record
shapes can evolve, but the semantics should stay stable.

Conceptual records:

- `mcp_servers`
  - server id, display name, transport kind, safe config, auth status, health,
    enabled state.
- `mcp_tools`
  - server id, tool name, description, input schema, output schema,
    annotations, metadata fingerprint.
- `tool_calibrations`
  - tool id, effective read/write/export classifications, owner extractors,
    enabled agents/scopes, review status, reviewer, reviewed metadata
    fingerprint.
- `trusted_identity_selectors`
  - owner human/scope, selector type, normalized value, effect, issuer,
    created/revoked timestamps.
- `tool_invocations`
  - proposed call, validation, policy decision, MCP invocation metadata,
    outcome.
- `quarantined_tool_results`
  - raw result reference, validation, ownership, scan/sanitization status,
    release status.
- `approval_requests`
  - action summary, payload preview/hash, destination, trust labels, decision,
    comments.

## Error Handling

The system fails closed.

Failure cases:

- MCP server cannot initialize.
- Tool metadata cannot be discovered.
- Tool has no completed calibration.
- MCP metadata changed since calibration and needs review.
- Arguments fail schema validation.
- Owner extraction fails.
- Ownership is unresolved.
- Result fails output schema validation.
- Read examination denies release.
- Write policy requires approval and approval is missing.
- Export approval is missing or denied.
- MCP invocation fails or times out.

Runtime failures produce structured failed tool results or approval waits. They
must not crash the turn, silently execute side effects, or release raw
quarantined content to the model.

## Testing

Tests should cover:

- Metadata discovery lists tools without invoking them.
- Tools are agent-invisible before calibration.
- Third-party annotations do not become effective policy without review.
- Metadata fingerprint changes force recalibration or disable affected tools.
- Email, phone, and domain selectors normalize and match correctly.
- Scope-level trusted identity additions and restrictions affect decisions.
- Mixed read resolves trusted and untrusted ownership correctly.
- Unresolved ownership blocks release and asks for extractor confirmation.
- Runtime extractor candidates are not auto-saved.
- Untrusted reads pass through examination before release.
- Sanitized read results are the only content sent to provider continuation.
- Trusted writes can proceed after validation and owner resolution.
- Untrusted or mixed writes require examination.
- Exports always create manual approval requests in V1.
- Approval denial prevents invocation or release.
- Audit records contain decision evidence without leaking raw private payloads
  into normal transcript views.
- Web Settings can show MCP server list, calibration status, trusted identity
  selectors, pending approvals, and audit links from backend read models.

## Acceptance Criteria

- Users can add MCP server configuration through Settings.
- Noema discovers MCP tools using metadata-only operations.
- Every agent-visible MCP tool has reviewed read/write/export calibration.
- Tools with unresolved ownership are blocked until the user/admin confirms an
  extractor from metadata/schema or a quarantined real result.
- Agent tool calls route through the Capability Gateway.
- Read results are quarantined before model-visible release.
- Export operations always require manual approval in V1.
- Web Settings exposes MCP setup, calibration, trusted identities, approvals,
  and audit paths.
- All policy decisions are inspectable through transcript activity and
  owner/admin audit records.

## Future Extensions

- Scoped auto-approval grants for low-risk export classes.
- Built-in high-quality adapters for common MCP providers.
- Marketplace install flows with signed metadata.
- Provider-native identity selectors such as workspace, group, or account ids.
- Richer prompt-injection classifiers and content transformation policies.
- Cross-MCP data-flow previews.
- Policy simulation for "what would this tool be allowed to do?"
