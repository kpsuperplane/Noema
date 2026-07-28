# Luau Adapter Transforms and Deterministic Account Identity

- **Mode:** plan only
- **Status:** Milestones 1–4 complete; Milestone 5 pending
- **Scope:** native public-HTTP adapters, connection identity, capability catalog, and setup review
- **Supersedes:** the response-computation portion of Milestone 10 in `2026-07-26-rust-native-adapter-runtime.md`; protocol extensions such as sockets, resumable media, and request signing remain deferred

## Outcome

Noema can compile a small reviewed Luau function into an adapter definition, give it one bounded HTTP success response, and accept only JSON-compatible output matching a declared schema. The function has no network, filesystem, process, environment, clock, randomness, credential, module-loading, or shared-state access. The existing Rust host remains the only authority for HTTP, OAuth, retries, status classification, limits, canonical redaction, governance, and persistence.

The same response pipeline supports a deterministic post-authentication account-identity probe. A Gmail definition can call `users.getProfile`, extract `emailAddress`, persist it as the recognizable account label, show it in Settings, and expose it once in the model's service context. This is deliberately separate from `account_id`, which remains reserved for a stable provider identity rather than a mutable human label.

OAuth token responses are not an identity source. Standard OAuth 2.0 returns authorization material such as an access token, token type, expiry, refresh token, and scopes ([RFC 6749 §5.1](https://www.rfc-editor.org/rfc/rfc6749#section-5.1)); an identity claim normally requires OpenID Connect and an ID token. Noema will use the already-authorized resource API instead, so Gmail needs no scope beyond the existing `gmail.readonly` permission for [`users.getProfile`](https://developers.google.com/workspace/gmail/api/reference/rest/v1/users/getProfile).

## Decisions locked by this plan

- Use embedded **Luau through `mlua`**, without JIT, rather than WASM or an external worker. The capability boundary is narrower than stdin/stdout: one immutable response value enters and one returned value leaves.
- Keep a built-in JSON response mode for ordinary APIs. Requiring every JSON endpoint to carry a pass-through script would increase manifests, latency, and failure modes without adding flexibility.
- Move all success-body interpretation out of the HTTP transport. The transport returns one bounded raw response; one response adapter performs either built-in JSON decoding or Luau transformation.
- A Luau transform is immutable reviewed definition data. Its exact source, accepted media types, and output schema participate in the semantic and operation digests; changing any of them requires a newly reviewed definition.
- Do not persist Luau bytecode. Source is the authority, compile validation uses the same embedded runtime, and each invocation starts from a fresh sandbox. A process-local derived cache can be considered only after profiling proves compilation material.
- Run transforms only for successful HTTP responses. Authentication, rate-limit, rejection, redirect, and uncertain-write handling remain Rust-owned and cannot be reclassified by script.
- Account identity is optional definition data that references an existing reviewed operation. Probe failure never blocks setup; the generated connection slug remains the fallback.
- Run the identity probe only when setup or reauthorization completes. Do not poll, backfill at startup, or add a refresh-label control in this slice.
- Persist a separate `account_label: Option<String>`. Never rewrite `connection_slug`, never use a human-readable email as invocation authority, and never substitute `account_id` as display text.
- Existing connections remain untouched until they are reauthorized or recreated. A failed reauthorization probe preserves any last-known label.
- Use the existing canonical structural secret-field redaction once, after response adaptation. Do not add operation-specific projection, model-only output, blanket error redaction, or a second durable result.

## Manifest and runtime contracts

### Operation response contract

`AdapterOperation` gains an optional `response` block. Its absence retains the current built-in JSON behavior, which keeps existing immutable definitions valid without a compatibility reader.

```yaml
response:
  accepted_content_types: [application/json]
  transform:
    language: luau
    source: |
      return function(response)
        local profile = json.decode(response.body)
        return {
          email = profile.emailAddress,
          message_count = profile.messagesTotal,
        }
      end
  output_schema:
    type: object
    properties:
      email: { type: string }
      message_count: { type: integer }
    required: [email, message_count]
    additionalProperties: false
```

Compiler rules:

- `transform` and `output_schema` appear together; `accepted_content_types` is required and contains 1–16 exact, normalized `type/subtype` values with no wildcard or parameters.
- Source is UTF-8, permits ordinary LF/tab formatting but no other control characters, is at most 32 KiB, and must evaluate to exactly one function. The compiler rejects syntax errors and any chunk that does not have the `return function(response) ... end` shape.
- The output schema is a closed JSON Schema-shaped subset: `type`, object `properties`/`required`/`additionalProperties`, and array `items`; scalar types are `string`, `integer`, `number`, `boolean`, and `null`. Reject `$ref`, remote references, unions, conditionals, regex/format execution, unknown keywords, recursive schemas, depth over 16, or more than 512 nodes.
- The compiler validates required names against properties, requires explicit `additionalProperties`, and canonicalizes property order. This uses a small adapter-owned validator; do not add a full JSON Schema engine.
- A response contract, including source and schema, is execution semantics. It enters `semantic_digest`, `operation_digest`, the review diff, and the operation token.
- Definitions without a transform continue to accept JSON and `+json`, run the shared bounded decoder, and publish the decoded value. They do not instantiate Luau.

### Luau ABI and sandbox

The returned function receives one read-only table:

```lua
response = {
  status = 200,                  -- successful HTTP status only
  content_type = "application/json", -- nil when a 204 has no media type
  body = "<binary-safe bytes>", -- decoded entity body, still bounded by Rust
}
```

The only host object is `json`:

- `json.decode(bytes)` uses Noema's duplicate-key-free, depth/node/collection-bounded JSON parser and returns tagged Luau arrays/objects plus `json.null`.
- `json.array()` and `json.object()` disambiguate empty output tables. Non-empty tables become arrays only when their keys are exactly `1..n`; string-keyed tables become objects; sparse, mixed, cyclic, or unsupported tables fail.
- Returned strings must be UTF-8. Functions, userdata, threads, non-finite numbers, invalid integer ranges, and output exceeding 1 MiB or the canonical JSON shape limits fail the call.

Create a fresh sandbox per call with only deterministic table, string, math-without-randomness, bit, and UTF-8 primitives. Remove `io`, `os`, `package`, `debug`, `require`, dynamic loading, coroutines/yielding, metatable mutation of host objects, and every ambient host callback. Apply a 16 MiB VM memory limit, one million Luau instructions, a 250 ms deadline checked by the VM interrupt/hook, and run on a bounded blocking worker so a transform cannot occupy the async executor. Any syntax/runtime/limit/schema failure is `invalid_response`; a non-idempotent operation consequently remains `outcome_uncertain`. There is no fallback to the raw response after a declared transform fails.

For a body-bearing response, the normalized media type must exactly match the reviewed list before the script runs. A 204 has an empty body and `content_type=nil`; the transform may map it to a schema-valid value. Parameters such as `charset` are stripped only for comparison and are not exposed separately.

### Account-identity contract

`AuthenticationRequirement` gains optional reviewed data:

```yaml
account_identity:
  operation_id: get_profile
  arguments:
    userId: me
  output_pointer: /emailAddress
```

The compiler requires the referenced operation to be `GET`, fully read-only and idempotent, non-destructive, non-open-world, `transport_safe_read`, single-page, and event-free. Fixed arguments must exactly satisfy its compiled input schema, and `output_pointer` must be a valid bounded RFC 6901 pointer. The probe observes the operation's canonical adapted output, so it works first with built-in JSON and can later consume a Luau-normalized result without a second probe protocol.

After token exchange and before credential publication, execute that operation once using the transient access token, the existing request encoder, and the hardened adapter client. Then reacquire the connection lock and revalidate the captured OAuth/definition/credential authority exactly as today. A string result is trimmed, rejected if blank or control-bearing, and capped at 256 bytes. Store only that label, never the profile response. Missing fields, HTTP errors, invalid output, or timeouts produce no label and do not fail OAuth setup.

For Gmail, the manifest data—not production Rust—declares `GET /gmail/v1/users/{userId}/profile`, `userId=me`, and `/emailAddress`. The label is recognizable but mutable, so `account_id` remains unset.

## Simplification and deletion target

The implementation must reduce the number of response authorities instead of placing Luau beside the current parser.

Delete or consolidate:

- Replace `AdapterHttpOutcome::{Success, Rejected, AuthenticationRequired, RateLimited}` with one bounded raw response carrying status, normalized content type, and body. Keep transport failures as the existing typed errors.
- Remove success JSON parsing, `validate_json_content_type`, `rejected_payload`, and duplicate `serde_json::from_slice`/shape checks from `network.rs`. Status and body interpretation move to one `response.rs` authority.
- Consolidate ordinary response JSON, error JSON, identity extraction, and `json.decode` on the same duplicate-key-free bounded parser in `json_limits.rs`.
- Do not resurrect manifest response projections, allowlists, persistence modes, provider routes, or per-operation sanitizers removed by the canonical-result cleanup.
- Do not add persisted bytecode, a plugin registry, a guest SDK, WASI, an RPC process, or transform-specific database tables.
- After cutover, search for and remove superseded response DTOs, parser helpers, fixtures, and comments. A refactor-only milestone must be net-negative.

Keep because they own independent invariants:

- DNS pinning, SSRF checks, HTTPS-only origins, no redirects/proxy, identity content encoding, timeouts, header/body limits, and retry rules stay in Rust transport.
- OAuth token parsing remains a separate protocol parser, while reusing the common bounded JSON primitive where possible.
- Pagination/delta JSON Pointers remain runtime workflow authority; Luau only shapes one page's published result.
- Canonical structural secret-field redaction remains after adaptation, and the one redacted `CapabilityOutput` still feeds model, transcript, continuation, action history, and replay.

## Milestones

### Milestone 1 — raw response authority and parser consolidation

**Outcome:** Existing adapters behave the same, but HTTP transport returns a bounded raw response and `response.rs` owns status classification, JSON decoding, canonical failure payloads, and the final pre-redaction value.

**Primary files:** `adapters/src/network.rs`, new `adapters/src/response.rs`, `adapters/src/json_limits.rs`, `adapters/src/invocation.rs`, focused invocation/network tests.

**Work:** Introduce the raw envelope, move all success/rejection interpretation into the response module, switch invocation to that module, and delete the old variants/helpers in the same commit. Preserve 204-as-null, actionable bounded JSON rejection bodies, authentication/rate-limit categories, and uncertain-write behavior exactly.

**Budget:** production must be net-negative; tests at most +100 net lines and three new tests. Measure with `--require-net-negative` for production and stop if both old and new response paths coexist.

**Unique risks/tests:** JSON and `+json` success remain canonical; a malformed 2xx write is uncertain while the same read fails safely; 401/429/rejected JSON retain their current typed consequences without leaking raw bytes.

### Milestone 2 — deterministic account labels

**Outcome:** A newly authorized connection can acquire a recognizable provider label without polling or provider-specific Rust, and setup still succeeds when the probe does not.

**Primary files:** `definition.rs`, `compiler.rs`, `digest.rs`, `service.rs`, `connection.rs`, `connection_store.rs`, `catalog.rs`, `noema-store/src/{schema,adapters}.rs`, `noema-api/src/graphql/capability_integrations.rs`, `noema-capabilities/src/binding.rs`, `noema-runtime/.../model_tools.rs`, and existing Settings projections.

**Work:** Add and compile `account_identity`; run it once inside OAuth completion; add optional `account_label` to the canonical descriptor and disposable projection; append the forward SQLite migration; retain `display_name` in `CompiledAdapterDefinition`; attach adapter service context; render `account=` once per exact destination; and use `account_label -> connection_slug` in Settings. `account_id` never participates in display fallback.

**Budget:** Rust production +450, Rust tests +180, at most six new tests; frontend/generated GraphQL net +60. Stop if the probe introduces a second HTTP client, a provider branch, a polling loop, or a standalone identity store.

**Unique risks/tests:** unsafe/mismatched probe declarations fail compilation; a successful synthetic Gmail-shaped profile persists the label through SQLite deletion/rebuild; probe failure activates with slug fallback and reauthorization failure preserves an old label; multiple tools emit one service row with the label; the forward migration preserves existing adapter rows.

### Milestone 3 — closed response contract and Luau sandbox

**Outcome:** Reviewed definitions compile deterministic Luau response transforms and execute them under the bounded ABI, but HTTP, credentials, and policy remain unreachable.

**Primary files:** workspace/adapters Cargo manifests, `definition.rs`, `compiler.rs`, `digest.rs`, new `output_schema.rs`, new `luau.rs`, and compiler/sandbox tests. Use `mlua` with only the Luau, vendored, and serialization features actually required; do not enable async, modules, or JIT.

**Work:** Add the manifest types and closed output-schema validator; compile-check source; build a fresh sandbox; implement `json.decode`, table-to-JSON conversion, resource limits, and schema validation; and include exact source/schema/media types in digests and review diffs.

**Budget:** Rust production +650, Rust tests +250, at most seven new tests. This is the explicitly budgeted cross-system capability slice; stop if the ABI needs arbitrary host functions, an OS subprocess, or more than the closed schema subset.

**Unique risks/tests:** forbidden globals and cross-call state are absent; instruction/memory/deadline exhaustion terminates; malformed/duplicate-key JSON and ambiguous Lua tables fail; schema mismatch fails closed; digest changes for one source byte or schema change while presentation-only metadata does not.

### Milestone 4 — invocation cutover, review exposure, and deletion pass

**Outcome:** Operations with a response contract transform successful JSON or non-JSON bodies before the one canonical redaction/output boundary, and humans can review the exact code and schema before activation.

**Primary files:** `response.rs`, `invocation.rs`, adapter setup template/review GraphQL projections, shared chat/Settings review components, and local protocol fixtures.

**Work:** Route declared transforms through the sandbox on the bounded worker; leave default JSON operations on the same response authority without a VM; render language, source digest, exact source, accepted media types, output schema, and identity-probe operation in the existing definition review disclosure; update the agent-facing definition template to explain when to use built-in JSON versus Luau. Delete all superseded parsing/adaptation code and stale tests before committing.

**Budget:** Rust production +300, Rust tests +180, at most five new tests; frontend +120 with no frontend tests. The final production delta for Milestones 1 and 4 combined should be no greater than +250 after deletions.

**Unique risks/tests:** a JSON normalization and a `text/csv` transform produce schema-valid canonical output; non-JSON without a declared transform fails; transforms never run for 401/403/429/5xx; a transformed secret-key field is redacted once for both model and durable consumers; transform failure on a write remains uncertain with no raw fallback.

### Milestone 5 — generation qualification and live development check

**Outcome:** The agent can propose the contract from official public-HTTP API documentation, and a fresh Gmail setup shows the account email in Settings and service-level model context before the first mailbox request.

**Work:** Add offline reviewed fixtures derived from Gmail profile and GitHub user responses to prove the same normalization mechanism across independent services, plus one synthetic non-JSON fixture for transport coverage. Update durable adapter documentation and `docs/context/current.md`. Against a development `NOEMA_HOME`, reconnect Gmail, inspect the reviewed identity request/transform, complete OAuth, confirm the recognizable label, delete only the development SQLite database while Noema is stopped, restart, and confirm the label rebuilds from `connection.json`.

**Budget:** definition/docs changes only unless qualification exposes a demonstrated generic defect. Live credentials, tokens, profile bodies, and mailbox results never enter fixtures, logs, commits, or bug reports.

**Stop conditions:** No live smoke result can override deterministic fixture failure. A provider-specific code change, extra OAuth scope solely for identity, startup backfill, or need for transform-side network/filesystem access leaves that definition unsupported and reopens architecture review.

## Acceptance scenarios

1. A normal JSON operation with no `response` block behaves exactly as before through the consolidated parser.
2. A reviewed Luau transform receives only status/content type/body, returns a schema-valid object, and its canonical redacted value is identical for the model and all durable consumers.
3. A transform cannot observe credentials, call HTTP, read files/environment/time, load code, retain globals, exceed its resource budget, or return an unvalidated value.
4. Non-JSON success is accepted only when an exact reviewed media type and Luau transform declare how to interpret it.
5. Gmail OAuth completes even when `users.getProfile` is unavailable; Settings then shows the stable generated slug rather than claiming identity discovery succeeded.
6. Gmail OAuth with a valid profile stores and displays the email label, emits one `account=` service attribute across all Gmail tools, and still leaves `account_id` unset.
7. Existing connections are not probed on startup. Reauthorization may populate a label; failed reauthorization preserves the previous one.
8. Changing Luau source, output schema, identity operation, fixed identity arguments, or output pointer creates a security-relevant semantic diff requiring review.

## Validation and delivery

Each milestone is one isolated commit on `main`; preserve unrelated worktree changes and update `docs/context/current.md` only when production behavior lands. Use the milestone's own base commit for the Rust size report and do not raise a failed budget without stopping for scope review.

Run focused adapter compiler, connection store, invocation, catalog, store-migration, API projection, and runtime model-tool tests after their owning milestone. Then run:

```bash
cargo fmt --all --check
cargo check-workspace
cargo gate-lint
cargo gate-test
```

For review UI changes, also run `bun run gen:types`, `bun run lint`, and `bun run build` from `apps/web`; visually inspect only with explicit permission. Before every commit run `git status --short --branch`, `git diff --check`, and inspect the staged stat/name-status. Push only when explicitly requested.
