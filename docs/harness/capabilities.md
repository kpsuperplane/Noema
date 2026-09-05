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
