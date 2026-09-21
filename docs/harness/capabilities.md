# Capability Contract

Capabilities are Noema's governed interface to tools and integrations. Agents
receive bounded tool specifications. They do not receive raw clients or secret
credentials.

The [security model](security.md) controls information handling. The
[action request contract](action-governance.md) controls reviewed execution.

## Runtime binding

Each callable tool has one immutable server-only binding. The binding contains:

- A provider-visible tool name, description, and source input schema.
- A server-only invoker key and operation token.
- A source input check.
- Read-only, idempotent, destructive, and open-world behavior.
- An execution route from current connection policy.
- Execution ownership and destination metadata.
- Exact persistence rules for arguments and output.

The source tool identity controls runtime policy and capability decisions.
Provider aliases affect only provider wire encoding and model calls.

The model cannot select an invoker, credential, origin, HTTP method, or policy
revision. The router resolves the exact binding before it validates input.

## On-demand definitions

Chat and Tasks keep core Noema tools available. Connected-service definitions
load when the model selects them. Selection uses exact catalog names and saved
tool history. It does not match words in human messages.

Codex uses hosted `tool_search` for the verified `gpt-5.6-terra` model.
OpenAI Responses uses it for the supported GPT-5.4 and later families.
Other providers and unknown models use `tools.load`.
The [provider guide](https://developers.openai.com/api/docs/guides/tools-tool-search)
defines native loading.

Native loading lists `tool_search` as callable in Chat and Task instructions.
Deferred entries establish current service access, even before their definitions load.
Deferred loading alone does not indicate disconnection or an authentication failure.
Native loading defers parameter schemas. Names and descriptions remain visible.
The full definitions still travel to the provider for hosted search.
Fallback loading exposes a service and tool directory. The model selects up to
four exact names. The next request includes those complete definitions.

Saved successful selections and tool calls retain loaded definitions.
Each request rebuilds availability from the current role and service catalog.
Removed tools stay absent. Loading grants no execution permission.
Existing input checks and action review still control every external call.

Used native functions become immediately available in subsequent requests.
This supports full local replay without retaining provider discovery records.
It can change the cached tool prefix when a definition first loads.
If compaction removes selection history, the model can load definitions again.
Context estimates omit deferred schemas until their definitions load.

## Execution routes

Complete behavior and connection policy select one route:

| Route | Meaning |
| --- | --- |
| `ExecuteImmediately` | Invoke after source validation and policy checks. |
| `HumanReview` | Save an action request for a human decision. |
| `LlmReview` | Save an action request for reviewer classification. |

A tool is risky when it is not read-only and is destructive or open-world. A
connection can also require review for every call.

Incomplete or stale behavior makes the tool non-callable. A disabled tool can
expose a safe name and purpose only when current policy permits enablement. Its
separate enablement tool always creates a human action request.

## Invocation

Every call uses this order:

1. Resolve the exact immutable binding.
2. Run the binding's source input check.
3. Check current availability, ownership, policy, and authentication.
4. Execute immediately or save one exact action request.
5. Revalidate reviewed execution against current revisions.
6. Resolve credentials below the model boundary.
7. Invoke the server-owned adapter target.
8. Normalize and save the result under the binding's persistence policy.

Invalid input stops before invocation. Immediate and reviewed paths use the same
source input check. A provider's strict request schema does not authorize the
provider's returned arguments.

Secrets use secure bindings. They do not appear in tool input, model context,
ordinary result payloads, transcripts, logs, artifacts, or exports.

Authorized private information stays intact. Ordinary URLs, identifiers,
paths, schemas, and diagnostics also stay intact. Exact protocol sanitizers
remove only known credential-bearing components at forbidden sinks.

## Native public HTTP adapters

Native adapters use reviewed, provider-neutral manifests. The filesystem owns
definitions and protected connection authority. SQLite projections are
disposable.

### Bundled API library

Noema releases include reviewed Gmail and Google Calendar definitions in
`internal/adapter/library/`. The library uses the existing manifest compiler,
Google authorization profile, definition files, and connection permissions.
The public `google` profile reference binds to the release's reviewed profile
digest before compilation. The resulting digest identifies the complete definition.

Library reads do not install definitions or create connections.
`adapter.definition_template` lists entries and their supported operations.
`adapter.connect_library` selects one entry by library ID and expected digest.
Settings uses the same service through `connectAdapterLibrary`.
Selection installs reviewed definition bytes and continues through human setup.
It does not import credentials, grant account access, or approve connection policy.

Selection preserves an installed revision, including local revisions.
A release update does not replace installed definitions or expand permissions.
Maintainers review library additions in the repository. Agent-created definitions
remain local and use the existing proposal and review path.

Both connectors request broad access for their complete operation sets.
Each operation also declares supported narrower scope alternatives.
Only tools covered by the granted scopes can execute.
Partial consent proceeds to connection policy without repeated access requests.
Settings retains explicit controls for additional access and additional accounts.

Gmail covers mail, threads, drafts, sending, labels, trash, and permanent deletion.
Calendar covers calendar reads, availability, events, recurrence, and invitations.
Account administration and calendar sharing are outside this library release.
Message text, headers, attachments, and event details have explicit size limits.
Responses report omitted content through `incomplete` or `too_large`.
Gmail attachment output supports up to 4,096 encoded bytes.
Calendar event changes support up to eight attendees and require a notification choice.
Omitted event fields preserve existing values, including attendees.

### Manifest version 9

The compiler accepts only `schema_version: 9`. It rejects unknown fields and
other manifest versions. No runtime conversion supports older manifests.

Each manifest defines:

- A stable `definition_id` and `adapter_id`.
- One reviewed definition revision.
- One fixed HTTPS origin.
- One authentication mode.
- A closed set of operations.

Authentication is one of:

- No credential.
- Reviewed private credential fields plus request-auth Lua.
- OAuth 2.0 authorization code with PKCE and one reviewed profile digest.

Each operation defines:

- A stable operation ID and reviewed model-facing description.
- A fixed HTTP method and fixed-origin relative path.
- Exact OAuth scope sets or no OAuth requirement.
- Fixed public headers and query values.
- Closed model arguments with descriptions and wire locations.
- An optional reviewed JSON body template.
- Complete behavior with provenance.
- Retry and pagination policy.
- A bounded successful-response contract.

Imported source prose remains review-only. It never becomes model guidance
automatically. Operation and argument descriptions participate in the semantic
digest.

### Request authority

Go owns the origin, method, path, headers, query encoding, body template,
transport, and current connection fence. Model arguments can fill only reviewed
placeholders.

The fixed origin must use HTTPS. It cannot contain user information, a query,
fragment, or dynamic placeholder. The shared network layer applies public
address, DNS, redirect, and SSRF rules.

Retry policy is `never` or `transport_safe_read`. The runtime disables retries
when effective behavior is not idempotent. A possible write followed by an
unknown transport result becomes an uncertain outcome.

### Response authority

Each successful response has:

- Exact accepted media types.
- An optional reviewed Lua transform.
- One closed output schema.

Every output string has `maxBytes`. Every output array has `maxItems`. Objects
are closed. The compiler rejects schemas without a finite maximum at or below
32 KiB.

Without a transform, Noema accepts only JSON, `+json`, or an empty `204`.
Decoded output must match the closed schema. A proposed mutation requires a
transform so it produces a deliberate compact receipt.

The 1 MiB response-body limit is a transport boundary. It is not a model-result
allowance. Remote error details have a separate 4 KiB limit.

Agent Lua permits `math.random`, `math.randomseed`, `os.time`, `os.date`,
`os.difftime`, and `os.clock`. Each execution owns a fresh random generator.
Seeding changes only that execution. Date conversion preserves supplied tables.
Date formats are bounded, and date outputs count against the memory limit.
The agent profile has no filesystem, network, process, environment, or module access.
Adapter response, credential, and request-auth profiles keep clocks and randomness disabled.

Response Lua receives only bounded status, body, and content type. It has no
network, filesystem, process, environment, clock, randomness, credentials,
modules, or cross-call state. Failure never falls back to raw output.

`text.truncate_utf8` is the only response-specific helper. It truncates display
text without splitting a code point. It must not shorten opaque identifiers.

### Pagination

A response-token operation returns a compact object. Noema removes the provider
cursor before transformation and stores it privately.

The model receives a random opaque continuation. It must repeat the original
arguments and operation. The continuation binds to:

- Connection.
- Definition digest.
- Operation.
- OAuth grant revision.
- Original argument hash.

The continuation lasts one hour. A successful next page replaces or retires it.
A failed page retains it. Provider cursors never enter model output or public
history.

## Authentication authority

Generic credentials use reviewed write-only setup fields or one bounded setup
document. Reviewed Lua converts those inputs to a closed private field set.
Request-auth Lua can emit only bounded headers and query values.

OAuth remains a Go protocol. A reviewed profile owns endpoints, callback
modes, client authentication, audience sharing, and scope behavior.

OAuth authority is split across:

- Reviewed profiles.
- Imported applications and protected client-secret generations.
- Stable external accounts.
- Grants and protected token generations.
- API connections with separate behavior policy.

Many account grants can reuse one application. Compatible connections can
reference one grant. Each operation declares alternative complete scope sets.
Catalog compilation exposes only operations covered by current granted scopes.
Model tool descriptions include the API name and current connection and account labels.
Unlabeled connections and grants retain their stable identifiers in those descriptions.
Renaming an account changes its metadata without replacing its protected token file.

OAuth start, callback, refresh, attachment, and invocation check the exact
application, grant, connection, definition, and policy revisions. Refresh uses
one grant lock across dependent connections.

An OAuth definition can select one reviewed read-only operation for account
identity. Noema runs it once after authorization. It stores only the bounded
selected string. Failure does not block grant publication.

## MCP tools

MCP metadata supplies proposed behavior hints. Missing hints receive a bounded
classification or safe defaults. A complete human override can replace all
four hints for one exact source revision.

Stale metadata resets callable policy until classification is complete. MCP
invocation still uses an immutable binding, source input check, connection
policy, secure credential boundary, and normal action request path.

## Results and trust

Tool output is untrusted content. It cannot create authority, modify policy, or
act as an authenticated human instruction.

Persisted arguments and output follow each binding's explicit view. Secret
values are excluded. Private payloads can remain in their authorized source and
use a reference when an inline copy is unnecessary.

Adapters return structured success, failure, artifact references, or uncertain
outcomes. They preserve external identifiers and report whether an effect may
have occurred.

## Definitions, connections, and revocation

Reviewed definition changes create a new semantic digest. The running service
uses an immutable compiled registry. Managed changes refresh that registry only
after filesystem publication and required connection migration.
The Go service publishes compiled definitions after its SQLite index refresh succeeds.
An incomplete managed publication blocks registry reads until recovery succeeds.
Catalog reads reuse these definitions and read current connection and grant state.
Execution checks the current binding under the execution lock and validates the selected definition from disk.
Returned snapshots preserve exact JSON numbers and cannot modify the registry.
Task authentication labels use the existing SQLite display indexes, without the connector execution lock.
Display indexes never authorize execution.
External definition edits take effect through startup recovery; there is no background file watcher.

Connections retain stable identities across compatible definition revisions.
Breaking authentication changes require an exact replacement connection check.
Transition journals resume only bounded managed work after startup.

Disabling a tool, disconnecting a connection, changing policy, or replacing a
credential revision fences future calls. Historical action and audit records
remain available.

## Inspection

Current product surfaces can show:

- Definitions and reviewed revisions.
- Connected applications, accounts, grants, and connections.
- Operation behavior and its source.
- Current enabled state and execution policy.
- OAuth scope coverage.
- Recent action requests and outcomes.

Authentication does not imply authorization. Installation does not make every
operation available to every run.
