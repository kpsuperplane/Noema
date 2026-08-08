# Information Handling Audit And Remediation Plan

**Status:** In progress; shared payload sanitizer remediated 2026-08-08

**Date:** 2026-08-07

**Authority:** `docs/harness/security.md`

## Bottom Line

Noema is currently careful in the wrong places and not careful enough in a few
places that matter.

The former shared payload sanitizer guessed whether information was secret
from fragments of field names. It removed valid ordinary and private
information while still missing credentials in unsupported locations. That
sanitizer has now been replaced with the bounded protocol-aware behavior
described below. The other forbidden-sink findings in this plan remain open.

The fix is not to remove every safeguard at once. First establish one reliable
boundary that removes actual secrets. Then remove the guessing and preserve all
authorized private and ordinary information unchanged.

## The Required Contract

Noema has exactly three information classes:

- **Secrets** grant authority, such as passwords, API keys, access and refresh
  tokens, private keys, session cookies, authorization codes, PKCE verifiers,
  and recovery codes. They belong only in explicit credential stores or
  protected short-lived authentication storage. They must never reach an LLM,
  logs, conversation history, ordinary events, artifacts, or exports.
- **Private information** may be stored in its canonical governed location and
  may be sent intact to an LLM when the current run is authorized to use it.
  Unauthorized contexts omit or deny access at the boundary without changing
  the stored source. External sharing is decided by egress policy.
- **Ordinary information** stays intact. IDs, paths, URLs, hostnames, ports,
  model names, account IDs, schema fields, diagnostics, and security-related
  field names are not secrets by default.

Redaction is not a substitute for authorization or egress policy.

## Problems Found

### 1. Shared payload sanitization guessed from field-name fragments — remediated

The previous implementation in `crates/noema-capabilities/src/binding.rs` hid a
value when its lowercased field name contained words such as `authorization`,
`secret`, `cookie`, or `api_key`.

This fails in both directions:

- Ordinary fields such as `oauth_authorization_supported`, `cookie_policy`, or
  `secret_rotation_status` are destroyed.
- A real credential under `value`, `credential`, `key`, or arbitrary MCP text
  can pass through unchanged.

The replacement uses one exact, case-insensitive list of standard credential
fields and URL parameters. It removes URL userinfo and credential parameters
from parsed URLs, and adapter connections extend it with their declared header
and query credential names. It preserves lookalike fields, ordinary URL
components, opaque IDs, and arbitrary high-entropy values. Adapters and MCP use
the same sanitizer for model-visible output that binding persistence applies
again idempotently.

This is intentionally best effort for arbitrary third-party response content.
A credential echoed in prose or under a neutral, undeclared field name may not
be recognized. Noema-managed credentials retain their hard guarantee through
credential-store provenance and injection below the model boundary. Stronger
third-party guarantees require a reviewed output contract or another declared
exact location, not substring or entropy guessing.

### 2. Raw provider and MCP content can be written to logs

`crates/noema-home/src/diagnostics.rs` explicitly supports uncapped,
unredacted raw payloads. Provider and MCP error paths can place full response
bodies, streaming data, model output, tool calls, and remote error messages in
`errors.log`.

Remote error bodies can also become runtime error strings, timing output,
conversation error notices, and background-work failures. If a remote service
echoes a credential, Noema does not currently guarantee that the credential is
excluded from those destinations.

### 3. A credential pasted into chat reaches storage and the model

User text is copied directly into the model request and conversation history.
Noema does not check it against credentials already held in its credential
stores and does not divert credential entry to a model-free, write-only flow.

This means a user who pastes a configured API key into chat can cause that key
to be stored and sent to a model.

### 4. The browser bootstrap secret is printed to standard output

On a new installation, the server prints a URL containing the one-use browser
bootstrap capability. Possession of that capability grants session authority.
Standard output is commonly captured as a process log, so it is not an
acceptable secret-delivery channel.

### 5. Tool and governed-action results lose valid information

The runtime sometimes saves the guessed-redacted derivative as the official
governed-action result. Background task continuation then ignores the stored
result payload and keeps only a short textual summary.

Authorized private and ordinary result data can therefore disappear even after
an action was approved and completed successfully.

### 6. Egress decisions do not yet carry information classes

Capability policy currently decides whether calls execute automatically or
receive review, but it does not carry the information classes, source scopes,
destination, audience, and private-data exposure needed by the documented
egress model.

The action reviewer receives exact arguments while its input currently states
that content exposure is false. That statement is wrong whenever the arguments
contain user or workspace content.

### 7. Other ordinary information is hidden unnecessarily

Smaller examples include:

- treating every URL fragment as secret and replacing the entire URL;
- rejecting Work event fields because names contain words such as `result`,
  `description`, or `token` even though the event schema is already closed and
  typed;
- hiding safe MCP commands, arguments, working directories, endpoints, and
  non-secret headers from Settings;
- hiding paths, service URLs, OAuth client IDs, request IDs, and capability
  identities in diagnostics and `Debug` output;
- omitting safe artifact identifiers and failure categories from diagnostics.

## Remediation Order

### 1. Replace the shared payload redactor with one bounded sanitizer — complete

The existing capability binding authority now applies:

- exact standard credential field names;
- URL userinfo and exact standard OAuth, API-key, and signed-URL parameters;
- exact custom header and query names declared by reviewed adapter
  connections; and
- preservation of every other field and URL component.

Adapter and MCP model output is sanitized before it enters the conversation,
and persistence applies the same operation idempotently. The implementation
does not scan arbitrary text, compare entropy, or invent a second
classification framework.

### 2. Close the known forbidden sinks

- Remove generic raw payload support from the ordinary diagnostic logger.
- Keep remote response bodies out of provider and MCP error display strings.
- Log useful typed metadata instead: provider, model, operation, status,
  request ID, byte count, parse category, and a hash when useful.
- Deliver the browser bootstrap capability through protected browser launch,
  IPC, or another private one-shot handoff instead of standard output.
- At chat ingress, reject exact matches to active stored credentials and direct
  the user to the explicit credential-entry flow.
- Give remaining secret-bearing types safe wrappers or custom `Debug`
  implementations.

### 3. Remove broad redaction and restore exact data — shared payload complete

The recursive substring matcher has been removed. Preserve the bounded
sanitizer result exactly.

Keep deliberate retention choices such as storing an artifact reference
instead of a duplicate artifact body, or a memory-page reference instead of a
duplicate page. These are explicit omissions backed by a canonical governed
source, not secrecy redaction, and should be named accordingly.

### 4. Repair tool-result and continuation fidelity

- Store the secret-excluded result as the governed action's canonical output.
- Derive compact audit and UI views separately.
- Include a bounded, typed result in the same task's background continuation.
- Keep owner and run checks around delayed result retrieval.

### 5. Finish private-information authorization and egress

- Carry information class, source scope, purpose, destination, and audience
  into governed-action proposals.
- Calculate content exposure from the actual arguments.
- Apply the exact destination's policy before private information leaves Noema.
- Keep authorized private information intact instead of redacting it.
- Thread run, purpose, and grant information into memory retrieval before
  private memory scopes or additional human principals are introduced.

### 6. Restore ordinary observability

- Preserve URL fragments for display while omitting them from HTTP transport.
- Remove the Work ledger's field-name matcher and rely on its typed schema.
- Expose safe MCP configuration in web and iOS Settings.
- Preserve ordinary paths, IDs, service endpoints, request IDs, and safe error
  categories.
- Remove only the actual secret component from mixed values such as URLs or
  OAuth structures.

## Required Tests

Every secret-handling test must have a paired preservation assertion at the
same boundary.

| Input | Model context | Canonical governed store | Logs | External egress |
| --- | --- | --- | --- | --- |
| Actual secret | Excluded | Credential store only | Excluded | Excluded |
| Authorized private information | Intact | Intact | No raw duplication | Exact destination policy |
| Unauthorized private information | Omitted or denied | Source unchanged | Excluded | Denied |
| Ordinary information | Intact | Intact | Useful safe metadata intact | Normal operation policy |

The regression suite should specifically prove:

- standard credential fields and URL parameters do not reach model output or
  ordinary persistence;
- connection-declared custom credential headers and query parameters do not
  reach adapter model output;
- `oauth_authorization_supported`, `token_count`, `cookie_policy`, paths, URLs,
  IDs, account identifiers, and high-entropy ordinary strings remain exact;
- authorized private content survives model use and persistence unchanged;
- unauthorized private content is omitted before context assembly without
  modifying its canonical source;
- a provider error that echoes a credential preserves status and request ID but
  not the credential;
- startup output contains no bootstrap bearer;
- an ordinary `#section` URL remains visible while HTTP transport uses the
  fragment-free URL;
- foreground and background governed actions receive the same exact safe
  result.

## Existing Work To Reuse

The remediation should preserve these sound patterns:

- provider, adapter, and MCP credentials generally use dedicated private
  stores with atomic writes and restrictive permissions;
- adapter model arguments cannot choose credential sources, and authentication
  is injected below the model boundary;
- built-in same-turn tool handling already separates model output from
  persisted views;
- native memory keeps canonical content intact and stores references when a
  transcript does not need another copy;
- production GraphQL HTTP and WebSocket requests authenticate before schema
  execution;
- web and iOS generally render the backend payload they receive instead of
  adding another field-name redactor.

## Scope Note

No current cross-human memory leak was found. Every production principal today
represents `human:local`. Memory reads still lack the run- and grant-level
checks required for future private scopes or multiple humans, so those features
must not ship until retrieval authorization happens before titles, snippets, or
bodies enter model context.

The shared payload sanitizer portion is implemented and covered by focused
tests. The remaining findings are separate remediation units and remain open.
