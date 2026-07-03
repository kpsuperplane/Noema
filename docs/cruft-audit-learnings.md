# Cruft Audit Learnings

Durable working log for the audit described in `docs/cruft-audit-plan.md`.

Purpose: preserve batch outputs and coordination state so context compaction or
thread interruption does not lose findings. This is not the final synthesized
cut-list. Verification and synthesis still need to run after all Phase 1
auditors finish.

## Coordination State

- Mode: read-only audit coordination.
- Source edits: none intended during Phase 1. This file is a durable audit log.
- User correction: do not run subsystem audits in the root thread. The root
  thread should coordinate, wait for agents, close completed agents, and spawn
  new agents in batches.
- Initial worktree observed before audit:
  - `## main...origin/main [ahead 98]`
  - Dirty unrelated files:
    - `.gitignore`
    - `crates/noema-core/src/daemon/web/assets/app.js`
    - `crates/noema-core/src/daemon/web/assets/styles.css`
    - `crates/noema-core/web/src/theme/noema-neutral.css`
  - Untracked audit plan: `docs/cruft-audit-plan.md`
- First spawn attempt hit the agent thread limit after six agents.

## Agent Ledger

Completed and closed:

- `daemon-web` - agent `019f24ba-618d-7d21-9221-df7d1f0b0460`
- `store-claims` - agent `019f24ba-5f5e-7c12-ab91-de5d313db624`
- `daemon-runtime` - agent `019f24ba-6111-7f71-9bd0-9facd0f29502`
- `graphql` - agent `019f24ba-5fcf-7280-bee9-b3b377c4a2b3`
- `daemon-core` - agent `019f24ba-6071-7782-9096-ed5204a9609f`
- `store-general` - agent `019f24ba-5ebc-7641-a64a-c76182978f92`
- `memory-persistence` - agent `019f24c1-1050-7603-83f0-fcccdf0ed6a9`
- `mcp` - agent `019f24c1-1109-79c3-be08-0b0d266ae94a`
- `memory-pipeline` - agent `019f24c1-0efc-75b2-abb0-baad45ab60fe`
- `provider-adapters` - agent `019f24c1-1258-7093-9298-10dc74515d9b`
- `memory` - agent `019f24c1-0f9c-7831-a947-4e32a04ac420`
- `provider-core` - agent `019f24c1-11c3-7051-9755-6822e0f98faa`
- `desktop` - agent `019f24c6-837b-78d3-888a-f1db25e0842d`
- `cli` - agent `019f24c6-82dd-7ea2-80b0-9f37db3b71c6`
- `frontend` - agent `019f24c6-8435-79f3-a16a-a658d11becb7`
- `cross-cutting-duplication` - agent `019f24c6-84b6-7743-a095-77bfae061479`
- `core-misc` - agent `019f24c6-81d5-7b80-9072-37e19b673829`

Phase 2 verifiers running:

- None. Phase 2 verifier batches are complete.

Phase 2 verifiers completed and closed:

- `verify-daemon-web` - agent `019f24cc-cd6a-7993-89e2-dcc4323b8139`
- `verify-store-claims` - agent `019f24cc-cabe-7142-8a2d-09c0f3023f84`
- `verify-store-general` - agent `019f24cc-ca21-7da0-a1da-43b2498abb97`
- `verify-daemon-runtime` - agent `019f24cc-cccc-7122-9279-77dd293e74fb`
- `verify-graphql` - agent `019f24cc-cb46-7a61-9d62-04b4fd47d81c`
- `verify-daemon-core` - agent `019f24cc-cc17-7971-913b-d5c8f6929e55`
- `verify-memory-persistence` - agent `019f24d1-0d90-7b83-8b77-145de86f86df`
- `verify-provider-core` - agent `019f24d1-0f09-7213-acc5-63fbceb898fc`
- `verify-mcp` - agent `019f24d1-0e29-7772-b88d-1be2bff000ec`
- `verify-memory-pipeline` - agent `019f24d1-0c6a-73c1-a5ba-c0d6fa640bce`
- `verify-memory` - agent `019f24d1-0cee-7ca0-a424-00108e7d7dec`
- `verify-provider-adapters` - agent `019f24d1-0fd9-79e1-b8b6-da74155d1673`
- `verify-desktop` - agent `019f24d7-11e5-7c30-86af-84ccc9466b58`
- `verify-cli` - agent `019f24d7-1155-7e72-aa3e-d90ee0ff0414`
- `verify-frontend` - agent `019f24d7-1294-79e2-a42a-f9d91cdcc9ed`
- `verify-cross-cutting-duplication` - agent `019f24d7-1365-7db3-995f-e4c851a0bbb6`
- `verify-core-misc` - agent `019f24d7-10ba-7590-bacb-d3d6d3fc64a8`

Still running:

- None for Phase 1 or Phase 2.

Not yet successfully spawned:

- None for Phase 1.

## Completed Phase 1 Findings

### daemon-web

Files read: `assets.rs`, `graphql_ws.rs`, `http.rs`, `mod.rs`,
`origin.rs`, `provider_auth.rs`, `replay.rs`.

Findings:

- `DW-001` - `crates/noema-core/src/daemon/web/provider_auth.rs:16`
  - Category: `legacy_dual_path`
  - LOC estimate: 345
  - Symbol: provider auth module, `WebApiError`,
    `ProviderAuthStartRequest`, `start_provider_auth_attempt_view_from_parts`
  - Summary: Provider auth is still owned by daemon-web even though the
    first-party product API moved to GraphQL.
  - Evidence: current context says GraphQL is the first-party client API and
    transitional product web endpoints are retired; `handle_connection` has no
    `/api` provider-auth routes; GraphQL onboarding calls daemon-web helpers.
  - References to check:
    `crates/noema-core/src/graphql/onboarding.rs:238`,
    `:256`, `:270`, `:275`, `:282`
  - Confidence: high
  - Risk: medium

- `DW-002` - `crates/noema-core/src/daemon/web/replay.rs:11`
  - Category: `legacy_dual_path`
  - LOC estimate: 148
  - Symbol: `ConversationReplayItem`, `visible_conversation_replay`,
    `web_conversation_item_from_record`
  - Summary: Conversation replay conversion is live, but it is now
    GraphQL-facing domain/API glue living under daemon-web.
  - Evidence: repo references show only GraphQL chat uses these daemon-web
    exports; conversion is for GraphQL conversation startup replay, not a
    daemon-web product endpoint.
  - References to check:
    `crates/noema-core/src/graphql/chat.rs:206`, `:326`, `:337`, `:343`
  - Confidence: medium-high
  - Risk: medium

- `DW-003` - `crates/noema-core/src/daemon/web/mod.rs:282`
  - Category: `oversized_refactor`
  - LOC estimate: 734
  - Symbol: `#[cfg(test)] mod tests`
  - Summary: `mod.rs` is over the 750-line inspection threshold mostly because
    route/module tests are colocated there.
  - Evidence: file is 1,015 lines; tests cover routing, assets, HTTP parsing,
    origin validation, provider auth, websocket helpers, onboarding, and replay.
  - Confidence: high
  - Risk: low-medium

- `DW-004` - `crates/noema-core/src/daemon/web/mod.rs:162`
  - Category: `dead_test`
  - LOC estimate: 35
  - Symbol: `is_supported_product_route` and
    `legacy_*_is_no_longer_client_product_api` tests
  - Summary: Retired `/api` product endpoint tests look like migration
    scaffolding now that GraphQL is the only product API.
  - Evidence: `is_supported_product_route` is test-only; active source refs to
    `/api/chat/ws`, `/api/status`, `/api/onboarding/status`, and
    `/api/provider-auth` are only these negative tests, while other matches are
    old docs/plans.
  - References to check:
    `docs/superpowers/specs/2026-06-26-provider-auth-onboarding-design.md`,
    `docs/superpowers/plans/2026-06-27-graphql-client-api.md`
  - Confidence: medium
  - Risk: low

- `DW-005` - `crates/noema-core/src/daemon/web/provider_auth.rs:132`
  - Category: `over_abstraction`
  - LOC estimate: 105
  - Symbol: `ProviderAccountStatusStore`, `ProviderAuthAttemptPoller`,
    `CodexDeviceAuthStarter`
  - Summary: Three boxed-future traits exist mainly as unit-test seams around
    concrete `NoemaStore` and `ProviderAuthManager` behavior.
  - Evidence: production impls are only `NoemaStore`/`ProviderAuthManager`;
    test doubles live in `daemon/web/mod.rs`; external callers use top-level
    provider-auth helpers rather than these traits directly.
  - References to check: `crates/noema-core/src/daemon/web/mod.rs:602`
  - Confidence: medium
  - Risk: medium

No credible scoped `unused_dep`, `duplication`, or truly removable production
`dead_code` candidates were found by the daemon-web auditor.

### store-claims

Files read: `consolidation.rs`, `graph.rs`, `inspection.rs`, `labels.rs`,
`model.rs`, `rows.rs`, `write.rs`.

Findings:

- `store-claims-001` -
  `crates/noema-core/src/store/claims/consolidation.rs:31`
  - Category: `duplication`
  - LOC estimate: 170
  - Symbol: `NoemaStore::find_consolidation_matches`
  - Summary: Same SurrealQL claim-match query shape and binds are repeated
    across several branches.
  - Evidence: exact-object, object-with-terms, no-object, and fallback branches
    repeat subject/predicate/status/sensitivity/order/limit logic with small
    variations.
  - References to check:
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:394`,
    `crates/noema-core/src/store/tests/claims.rs:683`
  - Confidence: high
  - Risk: medium

- `store-claims-002` - `crates/noema-core/src/store/claims/write.rs:1`
  - Category: `oversized_refactor`
  - LOC estimate: 896
  - Symbol: claims write module
  - Summary: File exceeds the 750-line inspection threshold and mixes relation
    writes, claim writes, reinforcement validation, supersession, entity upsert,
    evidence insertion, summaries, and fingerprinting.
  - Evidence: related-claim APIs, create/reinforce, supersession,
    validation/merge, persistence helpers, and fingerprinting all live in one
    file.
  - References to check:
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:380`,
    `crates/noema-core/src/store/tests/claims.rs`
  - Confidence: high
  - Risk: medium

- `store-claims-003` - `crates/noema-core/src/store/claims/write.rs:88`
  - Category: `over_abstraction`
  - LOC estimate: 35
  - Symbol: `NoemaStore::related_claims`
  - Summary: Public read API for related-claim relations currently has no
    production caller in repo search.
  - Evidence: production writes related relations via `relate_claims`, but
    `related_claims()` appears only in store tests and its own definition.
  - References to check: future memory graph/provenance/detail surfaces before
    removing or hiding this API.
  - Confidence: medium
  - Risk: low

- `store-claims-004` - `crates/noema-core/src/store/claims/model.rs:66`
  - Category: `speculative`
  - LOC estimate: 8
  - Symbol: `EvidenceAuthority::{HumanCorrection, DocumentSource,
    WeakInference, SystemRule}`
  - Summary: Several evidence authority variants are schema-supported but not
    constructed by current Rust code.
  - Evidence: runtime uses `ExplicitHumanStatement`, `AgentInference`, and
    `RepeatedObservation`; other variants appear only in enum/schema/docs.
  - References to check: document import, correction, contradiction, and
    system-rule memory flows before pruning.
  - Confidence: medium
  - Risk: low

No credible `legacy_dual_path`, `dead_test`, or `unused_dep` candidates were
found inside the scoped files.

### store-general

Files read: `docs/project.md`, `docs/context/current.md`,
`crates/noema-core/src/store.rs`, and all non-test
`crates/noema-core/src/store/*.rs`: `agent_runtime_preferences.rs`,
`agents.rs`, `claims.rs`, `context_summaries.rs`, `conversations.rs`,
`error.rs`, `ids.rs`, `mcp.rs`, `objects.rs`, `ontology.rs`,
`provider_accounts.rs`, `retrieval.rs`, `runtime.rs`, `schema.rs`.

The auditor excluded `store/claims/**` and dedicated tests from final deadness
claims, using repo-wide `rg` only for reference evidence.

Findings:

- `store-general-001` - `crates/noema-core/src/store/schema.rs:121`
  - Category: `speculative`
  - LOC estimate: 31
  - Symbol: `tool_invocations`, `quarantined_tool_results`
  - Summary: MCP invocation/quarantine tables are bootstrapped but have no
    production store API or production writes in current Rust sources.
  - Evidence: repo-wide search in `crates/noema-core/src` found these table
    names only in `schema.rs` and dedicated store tests. Adjacent
    `approval_requests` is wired through `store/mcp.rs` and GraphQL, but
    `tool_invocations`/`quarantined_tool_results` are not.
  - References to check:
    `docs/superpowers/specs/2026-06-30-third-party-mcp-control-plane-design.md`;
    capability gateway audit/quarantine roadmap.
  - Confidence: high
  - Risk: low-medium

- `store-general-002` - `crates/noema-core/src/store/objects.rs:1`
  - Category: `legacy_dual_path`
  - LOC estimate: 8
  - Symbol: `store::objects::{ActorRef,ObjectRef}`
  - Summary: Store-local object aliases appear transitional and unused by
    production code.
  - Evidence: file explicitly says it exists while SurrealDB repositories are
    being split out. Repo-wide search found no production imports of
    `store::objects`; crate-level `ActorRef`/`ObjectRef` are already exported
    from `crates/noema-core/src/objects.rs`.
  - References to check: public API expectations for
    `noema_core::store::objects`; excluded `store/claims/**` imports.
  - Confidence: medium-high
  - Risk: low

- `store-general-003` - `crates/noema-core/src/store/mcp.rs:12`
  - Category: `oversized_refactor`
  - LOC estimate: 1448
  - Symbol: `store::mcp` module
  - Summary: The MCP store module substantially exceeds the 750-line inspection
    threshold and mixes several separable responsibilities.
  - Evidence: file owns server metadata, tool metadata, calibration validation,
    trusted identities, approval requests, row mapping, preview sanitization,
    enum parsing, and MCP-specific record-id encoding. These are production
    surfaces, so this is refactor work rather than dead deletion.
  - References to check: `crates/noema-core/src/graphql/mcp.rs`,
    `crates/noema-core/src/mcp/setup.rs`,
    `crates/noema-core/src/capability/gateway.rs`
  - Confidence: high
  - Risk: medium

- `store-general-004` - `crates/noema-core/src/store/agents.rs:180`
  - Category: `duplication`
  - LOC estimate: 22
  - Symbol: `agent_record_fragment`, `mcp_record_fragment`
  - Summary: Two modules duplicate the same hex record-fragment encoder with
    only the prefix changed.
  - Evidence: `agents.rs` defines `agent_record_fragment` with byte-to-hex
    logic; `mcp.rs:1438` defines the same loop as `mcp_record_fragment`.
    `store/ids.rs` also has a separate `record_fragment` helper with different
    lossy semantics.
  - References to check: `crates/noema-core/src/store/mcp.rs:1438`,
    `crates/noema-core/src/store/ids.rs:16`
  - Confidence: high
  - Risk: low

- `store-general-005` - `crates/noema-core/src/store/schema.rs:319`
  - Category: `legacy_dual_path`
  - LOC estimate: 56
  - Symbol: `supported_by`/`corrected_by`/`contradicted_by`
    `source_object_type` vocabulary
  - Summary: Claim evidence source vocabularies still allow retired/absent
    object-table names such as `memory_item`, `relationship`, and
    `context_packet`.
  - Evidence: embedded schema defines `retrieval_packets`, not
    `context_packets`, and no `memory_items`/`relationships` tables in this
    schema. Search shows those table names still live in `objects.rs`
    `table_name` mappings and memory type comments, while current context says
    graph claims are the durable memory direction.
  - References to check: `crates/noema-core/src/objects.rs:82`,
    `crates/noema-core/src/memory/types.rs:6`, excluded `store/claims/**` for
    intended provenance semantics.
  - Confidence: medium
  - Risk: medium

### daemon-runtime

Files read: `daemon/runtime.rs` and every `.rs` file under
`daemon/runtime/`. Supporting docs/reference read by auditor:
`docs/project.md`, `docs/context/current.md`, `docs/harness/runtime.md`, and
Foundation Local spec/plan material.

Findings:

- `DR-001` - `crates/noema-core/src/daemon/runtime/turn.rs:268`
  - Category: `over_abstraction`
  - LOC estimate: 25
  - Symbol: `refresh_tool_snapshot` / `CachedToolSnapshot`
  - Summary: Tool snapshot caching appears inert, and the first refresh call is
    immediately duplicated.
  - Evidence: first call assigns `_tool_snapshot` and is unused; second call
    re-renders tools. Repo-wide search only found the cached hash read inside
    refresh replacement logic, not to skip rendering or drive behavior.
  - References to check: MCP tool calibration/runtime tests around rendered
    tool prompts.
  - Confidence: high
  - Risk: low

- `DR-002` - `crates/noema-core/src/daemon/runtime/handle.rs:111`
  - Category: `over_abstraction`
  - LOC estimate: 3
  - Symbol: `configured_provider_kind`
  - Summary: Private wrapper only returns `default_provider_kind`, and public
    `provider_kind` immediately delegates to it.
  - Confidence: high
  - Risk: low

- `DR-003` - `crates/noema-core/src/daemon/runtime/memory_writes.rs:48`
  - Category: `legacy_dual_path`
  - LOC estimate: 8
  - Symbol: `canonicalize_memory_write` / `consolidate_promoted_claim`
  - Summary: Provider memory canonicalization and consolidation use the daemon
    default provider, not the selected conversation/agent provider.
  - Evidence: turn execution resolves provider preference, but memory-write
    model calls use `self.default_provider()?`. If the user selects
    `foundation_local` under a Codex-default daemon, memory resolution can still
    go through Codex.
  - References to check: intended maintenance-model policy, privacy expectations
    for local-provider users, memory consolidation tests.
  - Confidence: medium
  - Risk: high

- `DR-004` - `crates/noema-core/src/daemon/runtime/turn.rs:949`
  - Category: `legacy_dual_path`
  - LOC estimate: 24
  - Symbol: `provider_selection_for_conversation`
  - Summary: Conversation override is model-only and forces the daemon default
    provider, bypassing saved agent provider preference.
  - Evidence: a nonempty `conversation_model` returns default provider plus
    model; only absent model checks `agent_runtime_preference`.
  - References to check: GraphQL `startPrimaryConversation(model, cwd)`, CLI
    model override behavior.
  - Confidence: medium
  - Risk: medium

- `DR-005` - `crates/noema-core/src/daemon/runtime/actor.rs:10`
  - Category: `legacy_dual_path`
  - LOC estimate: 40
  - Symbol: `CodexRuntimeActor` / `CodexRuntimeCommand`
  - Summary: Runtime actor/command names remain Codex-specific while behavior
    is provider-neutral.
  - Evidence: actor holds provider map and handles `foundation_local`, OpenAI,
    and Codex through `RuntimeModelProvider`; repo refs show broad internal use
    of Codex-named runtime types.
  - References to check: `runtime_host.rs`, `graphql/runtime_state.rs`, daemon
    tests.
  - Confidence: high
  - Risk: medium

- `DR-006` - `crates/noema-core/src/daemon/runtime/turn.rs:796`
  - Category: `speculative`
  - LOC estimate: 10
  - Symbol: `agent_identity_for_conversation`
  - Summary: Helper accepts a conversation id but ignores it and always resolves
    `agent:primary`.
  - Evidence: parameter is named `_conversation_id`; current product is
    primary-agent only, so the abstraction is ahead of current state.
  - References to check: future multi-agent/conversation-agent ownership design.
  - Confidence: medium
  - Risk: low

No credible `dead_test` or `unused_dep` candidates surfaced inside the scoped
daemon-runtime files.

### daemon-core

Files read in scope: `crates/noema-core/src/daemon.rs`,
`daemon/agent_name_tool.rs`, `daemon/agent_onboarding.rs`,
`daemon/client.rs`, `daemon/memory_tool.rs`, `daemon/prompts.rs`,
`daemon/protocol.rs`, `daemon/server.rs`. The auditor also read project context
docs and targeted reference snippets outside scope for `rg` verification.

Findings:

- `DC-001` - `crates/noema-core/src/daemon/memory_tool.rs:250`
  - Category: `speculative`
  - LOC estimate: 8
  - Symbol: `explicit_memory_request`
  - Summary: English substring matching is used to set a trusted memory-policy
    flag.
  - Evidence: `build_request` feeds `explicit_memory_request(&context.user_input)`
    into `ClaimRetrievalRequest`; memory policy later uses that flag to allow
    private/sensitive claim retrieval.
  - References to check: `crates/noema-core/src/memory/model.rs:90`,
    `crates/noema-core/src/memory/model.rs:681`
  - Confidence: high
  - Risk: medium-high; keeping it conflicts with the repo standard against
    English phrase intent authority.

- `DC-002` - `crates/noema-core/src/daemon/memory_tool.rs:49`
  - Category: `over_abstraction`
  - LOC estimate: 35
  - Symbol: `SearchMemoryArguments.purpose`, `runtime_purpose`, `parse_purpose`
  - Summary: `search_memory` accepts and validates many purpose values, but
    graph retrieval ignores them and always uses `Answer`/`AnswerHumanQuestion`
    semantics.
  - Evidence: `runtime_purpose` validates supplied purpose then returns
    `Purpose::AnswerHumanQuestion`; `build_request` always sets
    `use_mode: UseMode::Answer`. Inline test says purpose is validated but not
    trusted.
  - References to check: `crates/noema-core/src/memory/model.rs:90`,
    `crates/noema-core/src/memory/store.rs`
  - Confidence: high
  - Risk: low-medium

- `DC-003` - `crates/noema-core/src/daemon/memory_tool.rs:91`
  - Category: `speculative`
  - LOC estimate: 25
  - Symbol: `context_packet_id`
  - Summary: The tool fabricates and returns a `context_packet_id` without
    persisting a context packet or retrieval record in this path.
  - Evidence: `execute_search_memory_inner` creates a deterministic string and
    returns it; `retrieve_claims_scoped` only returns `ClaimRetrievalResult` and
    does not write a packet.
  - References to check: `crates/noema-core/src/store/retrieval.rs:70`,
    `crates/noema-core/src/objects.rs:102`
  - Confidence: high
  - Risk: medium

- `DC-004` - `crates/noema-core/src/daemon/client.rs:94`
  - Category: `legacy_dual_path`
  - LOC estimate: 110
  - Symbol: `DaemonClient::start_conversation`, `turn`, `turn_streaming`
  - Summary: Client-side Unix-socket chat helpers appear obsolete after CLI chat
    moved to GraphQL.
  - Evidence: repo-wide search found no non-test callers; CLI chat uses
    `start_primary_conversation` and `stream_conversation_turn` over GraphQL,
    keeping `DaemonClient` only for connect/hello/end/shutdown lifecycle.
  - References to check: `crates/noema-cli/src/commands/chat.rs:31`,
    `crates/noema-cli/src/commands.rs:44`,
    `crates/noema-core/src/daemon/server.rs:127`
  - Confidence: high inside repo; medium if external raw daemon clients count.
  - Risk: medium

- `DC-005` - `crates/noema-core/src/daemon/protocol.rs:29`
  - Category: `dead_code`
  - LOC estimate: 5
  - Symbol: `DaemonRequest::ConversationStart.instructions`
  - Summary: Unix protocol carries an `instructions` field that is never set by
    `DaemonClient` and is ignored by the server.
  - Evidence: `DaemonClient` sends `instructions: None`; server destructures
    `instructions: _` and calls `runtime.start_conversation(model, cwd)`.
  - References to check: `crates/noema-core/src/daemon/client.rs:100`,
    `crates/noema-core/src/daemon/server.rs:188`
  - Confidence: high
  - Risk: low

- `DC-006` - `crates/noema-core/src/daemon/protocol.rs:318`
  - Category: `duplication`
  - LOC estimate: 6
  - Symbol: `socket_path_for_home`
  - Summary: Public helper duplicates `NoemaPaths` socket construction and has
    no non-test in-repo callers.
  - Evidence: `NoemaPaths::from_home_dir(home).socket_path()` already models
    `home/.noema/run/noema.sock`; search only found `socket_path_for_home` in
    excluded daemon tests and public re-exports.
  - References to check: `crates/noema-core/src/paths.rs:53`,
    `crates/noema-core/src/paths.rs:132`,
    `crates/noema-core/src/lib.rs:55`
  - Confidence: high for in-repo deadness
  - Risk: low-medium because it is exported publicly

- `DC-007` - `crates/noema-core/src/daemon/protocol.rs:288`
  - Category: `dead_code`
  - LOC estimate: 3
  - Symbol: `DaemonError::Memory`
  - Summary: `DaemonError` still has a `MemoryPersistenceError` variant after
    daemon paths moved to Store/Provider errors.
  - Evidence: search found `MemoryPersistenceError` in daemon only at this
    import/variant; runtime memory writes propagate `StoreError` through
    `DaemonError::Store` and provider failures through `DaemonError::Provider`.
  - References to check:
    `crates/noema-core/src/daemon/runtime/memory_writes.rs`,
    `crates/noema-core/src/daemon/runtime/transcript_persistence.rs`
  - Confidence: medium-high
  - Risk: low

- `DC-008` - `crates/noema-core/src/daemon/agent_name_tool.rs:1`
  - Category: `dead_code`
  - LOC estimate: 1
  - Symbol: `#![allow(dead_code)]`
  - Summary: Whole-file dead-code suppression looks stale now that the tool is
    integrated through `runtime/local_tools`.
  - Evidence: repo-wide search shows `AgentNameToolRuntimeContext`,
    `AgentNameToolResult`, `execute_update_own_name`, and
    `is_update_own_name_tool` are used by `daemon/runtime/local_tools.rs`.
  - References to check:
    `crates/noema-core/src/daemon/runtime/local_tools.rs:10`
  - Confidence: medium
  - Risk: low

No credible `unused_dep`, `dead_test`, or `oversized_refactor` candidates
surfaced within the requested non-test daemon-core scope.

### graphql

Files read: `graphql.rs`, `agents.rs`, `chat.rs`, `errors.rs`,
`local_status.rs`, `mcp.rs`, `memory.rs`, `onboarding.rs`,
`provider_accounts.rs`, `resolvers.rs`, `runtime_state.rs`, `schema.rs`,
`subscriptions.rs`, `types.rs`.

Findings:

- `GQL-001` - `crates/noema-core/src/graphql/types.rs:1`
  - Category: `dead_code`
  - LOC estimate: 2
  - Symbol: `graphql::types`
  - Summary: Private module only re-exports other GraphQL modules under
    `#[allow(unused_imports)]`.
  - Evidence: repo-wide search found only `mod types`; schema imports concrete
    modules directly.
  - References to check: `crates/noema-core/src/graphql.rs:19`,
    `crates/noema-core/src/graphql/schema.rs:9`
  - Confidence: high
  - Risk: low

- `GQL-002` - `crates/noema-core/src/graphql/resolvers.rs:1`
  - Category: `dead_test`
  - LOC estimate: 87
  - Symbol: `graphql::resolvers`
  - Summary: File has no production resolver helpers, only old local-status
    tests.
  - Evidence: only referenced by `mod resolvers`; implementation now lives in
    `local_status.rs` and `schema.rs`.
  - References to check: `crates/noema-core/src/graphql.rs:15`,
    `crates/noema-core/src/graphql/local_status.rs:47`
  - Confidence: medium
  - Risk: low if tests are moved, medium if deleted outright

- `GQL-003` - `crates/noema-core/src/graphql/schema.rs:553`
  - Category: `oversized_refactor`
  - LOC estimate: 2664
  - Symbol: `mod tests`
  - Summary: Schema root test module covers provider accounts, agents, MCP
    setup, approvals, memory, runtime, and subscriptions.
  - Evidence: `schema.rs` is 3,216 LOC; production schema roots are smaller, but
    tests dominate the file.
  - References to check: split by subsystem near
    `graphql/{agents,mcp,memory,chat}_tests.rs`
  - Confidence: high
  - Risk: medium

- `GQL-004` - `crates/noema-core/src/graphql/schema.rs:138`
  - Category: `over_abstraction`
  - LOC estimate: 137
  - Symbol: `GraphqlMcpSetupTransport`
  - Summary: Schema root owns MCP transport construction, fake setup outcomes,
    and `McpTransport` impl.
  - Evidence: used only by MCP setup paths in `mcp.rs` plus tests; not part of
    schema root definition.
  - References to check: `crates/noema-core/src/graphql/mcp.rs:617`
  - Confidence: high
  - Risk: medium

- `GQL-005` - `crates/noema-core/src/graphql/mcp.rs:25`
  - Category: `oversized_refactor`
  - LOC estimate: 1023
  - Symbol: `graphql::mcp`
  - Summary: One module combines schema types, record mapping, setup commands,
    OAuth completion, approval reads, autofill, and string parsers.
  - Evidence: file exceeds the 750-line inspection threshold with distinct
    responsibilities.
  - References to check: web operations use these fields/mutations, so split
    only mechanically.
  - Confidence: high
  - Risk: medium-high

- `GQL-006` - `crates/noema-core/src/graphql/memory.rs:293`
  - Category: `legacy_dual_path`
  - LOC estimate: 20
  - Symbol: `GraphqlMemoryGraphNode.entity_id`, `redacted`, `fact_redacted`
  - Summary: `entity_id` is always the opaque `node_id`; graph `redacted` and
    `fact_redacted` are always false.
  - Evidence: tests assert `entityId == nodeId` and false redaction flags. Web
    queries still request these fields, so this is ambiguous API surface, not
    confirmed dead.
  - References to check:
    `crates/noema-core/web/src/graphql/operations.ts:350`, memory graph
    docs/current context.
  - Confidence: medium
  - Risk: medium

- `GQL-007` - `crates/noema-core/src/graphql/memory.rs:389`
  - Category: `duplication`
  - LOC estimate: 19
  - Symbol: `mapped_memory_graph_node_id`,
    `mapped_memory_graph_edge_endpoint_id`
  - Summary: Two helpers have identical logic and differ only in panic message.
  - Confidence: high
  - Risk: low

- `GQL-008` - `crates/noema-core/src/graphql/local_status.rs:13`
  - Category: `legacy_dual_path`
  - LOC estimate: 8
  - Symbol: `GraphqlAssistantConnection::Codex`
  - Summary: Local status always reports `CODEX`, while chat/provider selection
    can now use other providers.
  - Evidence: `startPrimaryConversation` returns selected provider separately;
    frontend queries `assistantConnection` but does not appear to use it
    directly.
  - References to check:
    `crates/noema-core/web/src/graphql/operations.ts:5`,
    `crates/noema-cli/src/graphql/mod.rs:29`
  - Confidence: medium
  - Risk: medium

No credible `unused_dep` candidates within GraphQL. `async-graphql`,
`async-stream`, and `futures-util` are used in scope; `ts-rs` remains used
elsewhere.

### memory-persistence

Scope requested: `crates/noema-core/src/memory_persistence.rs` plus
`crates/noema-core/src/memory_persistence/*.rs`.

Findings: no credible in-scope candidates because the requested scope no longer
exists in the current tree.

Evidence:

- `crates/noema-core/src/memory_persistence.rs` is absent.
- `crates/noema-core/src/memory_persistence/` is absent.
- `git ls-files` has no matching paths.
- History shows commit `94986f35806a22239efeb466e45ef131d4ea45f2` deleted
  `memory_persistence.rs` and all `memory_persistence/*.rs`.
- `docs/superpowers/plans/2026-06-28-surrealdb-graph-memory-store.md` called for
  deleting those files.
- `docs/context/current.md` confirms graph claims replaced the legacy memory
  persistence path.

Out-of-scope follow-up references noted by auditor:

- `crates/noema-core/src/memory.rs` exports `MemoryStore`, described as a
  persistence-mirroring retrieval model.
- `crates/noema-core/src/memory/store.rs` and
  `crates/noema-core/src/memory/tests.rs` may hold the old in-memory
  `MemoryItem`/relationship policy surface, mostly test-driven.
- `crates/noema-core/src/ids.rs`, `crates/noema-core/src/objects.rs`, and
  `crates/noema-core/src/store/schema.rs` still reference legacy object names
  like `memory_item`, `memory_items`, `relationship`, and `context_packet`.

Files read: `docs/project.md`, `docs/context/current.md`, `docs/memory.md`,
the 2026-06-28 graph-memory spec/plan, `crates/noema-core/src/lib.rs`,
`crates/noema-core/src/memory.rs`.

### mcp

Files read: `crates/noema-core/src/mcp.rs`, `mcp/autofill.rs`,
`mcp/autofill/tests.rs`, `mcp/client.rs`, `mcp/http.rs`, `mcp/oauth.rs`,
`mcp/secrets.rs`, `mcp/setup.rs`, `mcp/stdio.rs`.

Supporting docs read: `docs/project.md`, `docs/context/current.md`, MCP
control-plane/setup/autofill specs, and `docs/cruft-audit-plan.md`.

Findings:

- `MCP-001` - `crates/noema-core/src/mcp.rs:157`
  - Category: `dead_code`
  - LOC estimate: 10
  - Symbol: `McpToolSchema`
  - Summary: Public Rust schema wrapper appears unused in-repo.
  - Evidence: repo search found only the definition and `lib.rs` re-export;
    current code uses `DiscoveredMcpTool`, `McpToolRecord`, and GraphQL/web
    schema shapes instead.
  - References to check: `crates/noema-core/src/lib.rs` public API export;
    external crate consumers, if any.
  - Confidence: medium-high
  - Risk: low-medium

- `MCP-002` - `crates/noema-core/src/mcp/client.rs:448`
  - Category: `dead_test`
  - LOC estimate: 5
  - Symbol: `FakeMcpTransport::call_tool`
  - Summary: Test-only inherent helper is explicitly `allow(dead_code)` and is
    not called.
  - Evidence: trait impl `call_tool` already increments `call_count`; tests
    inspect `call_count` after `discover_tools` without using this inherent
    method.
  - References to check: `FakeMcpTransport::call_tool`
  - Confidence: high
  - Risk: low

- `MCP-003` - `crates/noema-core/src/mcp/stdio.rs:174`
  - Category: `duplication`
  - LOC estimate: 20
  - Symbol: `call_tool_params`, `call_tool_arguments`,
    `call_tool_result_value`
  - Summary: RMCP tool-call argument/result helpers are duplicated with HTTP
    transport code.
  - Evidence: equivalent helpers also live in `mcp/http.rs:725` and are used by
    live stdio/Streamable HTTP `call_tool` paths.
  - References to check: `crates/noema-core/src/mcp/http.rs:725`,
    `crates/noema-core/src/capability/gateway.rs:127`
  - Confidence: high
  - Risk: low-medium

- `MCP-004` - `crates/noema-core/src/mcp/setup.rs:535`
  - Category: `duplication`
  - LOC estimate: 35
  - Symbol: `string_field`, `optional_string_field`, `string_array_field`,
    `string_map_field`
  - Summary: JSON config field parsing is repeated across setup and transport
    modules.
  - Evidence: similar parsing helpers appear in `stdio.rs:197`,
    `http.rs:763`, and `client.rs:273` with only error-type/message
    differences.
  - References to check: `normalize_safe_config`,
    `StdioMcpTransport::from_server_config`, `http_config_from_server`
  - Confidence: medium
  - Risk: medium

- `MCP-005` - `crates/noema-core/src/mcp/http.rs:45`
  - Category: `oversized_refactor`
  - LOC estimate: 0 direct removal; 250-350 extractable
  - Symbol: `StreamableHttpMcpTransport`, `SseMcpTransport`,
    OAuth/header/SSE helpers
  - Summary: File exceeds the 750-line guideline and carries two transports
    plus shared auth/parsing helpers.
  - Evidence: SSE is not dead; GraphQL setup and Capability Gateway both still
    dispatch to `SseMcpTransport`. This is a split/refactor candidate, not a
    deletion candidate.
  - References to check: `crates/noema-core/src/graphql/schema.rs:172`,
    `crates/noema-core/src/capability/gateway.rs:127`
  - Confidence: high
  - Risk: medium

- `MCP-006` - `crates/noema-core/src/mcp/setup.rs:92`
  - Category: `oversized_refactor`
  - LOC estimate: 0 direct removal; around 380 test-split candidate
  - Symbol: guided setup orchestration plus inline tests
  - Summary: File is 1,064 lines mostly because integration-style tests are
    colocated with setup orchestration.
  - Evidence: production code ends around line 679, under the 750-line
    guideline; inline tests occupy lines 681-1064.
  - References to check: `mcp::setup` test module, GraphQL create/continue
    setup callers.
  - Confidence: high
  - Risk: low

No credible `legacy_dual_path` deletion candidate inside this scope: legacy SSE
is documented as supported and is wired through both setup/discovery and
Capability Gateway execution. No scoped `unused_dep` candidate found.

### memory-pipeline

Files read: `crates/noema-core/src/daemon/memory_pipeline.rs` whole file,
`docs/project.md`, `docs/context/current.md`, `docs/memory.md`,
`docs/superpowers/plans/2026-06-30-one-call-memory-write-resolution.md`, plus
referenced Rust files under daemon runtime/tests and adjacent GraphQL/MCP
helpers.

Findings:

- `MP-001` - `crates/noema-core/src/daemon/memory_pipeline.rs:88`
  - Category: `dead_test`
  - LOC estimate: 71
  - Symbol: `explicit_memory_claim_candidate`, `provider_memory_claim_candidate`
  - Summary: Legacy claim-candidate wrappers are tests-only; runtime writes use
    proposal routing.
  - Evidence: both wrappers have dead-code allows saying legacy daemon tests
    exercise them. Search finds only `daemon/tests.rs` callers; runtime calls
    `explicit_memory_write_proposal`/`provider_memory_write_proposal` instead.
  - References to check: `crates/noema-core/src/daemon/tests.rs:1037`,
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:169`,
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:571`
  - Confidence: high
  - Risk: low-medium

- `MP-002` - `crates/noema-core/src/daemon/memory_pipeline.rs:921`
  - Category: `speculative`
  - LOC estimate: 26
  - Symbol: `infer_chat_memory_type`, `title_from_memory_content`
  - Summary: Staged “next slice” helpers have no repo references.
  - Evidence: search finds only definitions and `#[expect(dead_code)]` reasons.
    Current context says graph-claim memory write bridge has landed.
  - References to check: none found by search in `crates/noema-core/src` or
    `crates/noema-cli/src`.
  - Confidence: high
  - Risk: low

- `MP-003` - `crates/noema-core/src/daemon/memory_pipeline.rs:320`
  - Category: `duplication`
  - LOC estimate: 182
  - Symbol: `parse_explicit_claim`, `parse_provider_claim`
  - Summary: Two near-parallel deterministic English prefix parsers duplicate
    relation inference and fallback-note shaping.
  - Evidence: both classify likes/dislikes/prefers through hard-coded prefixes,
    then call `fallback_note_claim`; provider path only adds aliases and has an
    unused `_title` parameter.
  - References to check:
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:261`,
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:571`,
    `crates/noema-core/src/daemon/tests.rs:1037`
  - Confidence: medium-high
  - Risk: medium

- `MP-004` - `crates/noema-core/src/daemon/memory_pipeline.rs:62`
  - Category: `legacy_dual_path`
  - LOC estimate: 24
  - Symbol: `explicit_memory_content`
  - Summary: Runtime explicit-memory intent is still gated by direct
    English/prefix matching.
  - Evidence: `turn.rs:321` calls this before provider generation; successful
    explicit detection suppresses provider memory proposal persistence later in
    the turn. Natural-language `remember this/that` prefixes are semantic phrase
    matching, distinct from a structured `/remember` command.
  - References to check: `crates/noema-core/src/daemon/runtime/turn.rs:320`,
    `crates/noema-core/src/daemon/runtime/turn.rs:760`,
    `crates/noema-core/src/daemon/tests.rs:996`
  - Confidence: medium
  - Risk: medium

- `MP-005` - `crates/noema-core/src/daemon/memory_pipeline.rs:634`
  - Category: `dead_code`
  - LOC estimate: 68
  - Symbol: `provider_subject_entity` helper chain
  - Summary: Several dead-code allowances are stale or misleading; these
    helpers are runtime-live through `provider_memory_write_proposal`.
  - Evidence: `provider_memory_write_proposal` calls `provider_subject_entity`,
    and runtime calls `provider_memory_write_proposal`; allow reasons still say
    legacy provider candidate wrapper/tests.
  - References to check:
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:169`,
    `crates/noema-core/src/daemon/tests.rs:1182`
  - Confidence: high
  - Risk: low

- `MP-006` - `crates/noema-core/src/daemon/memory_pipeline.rs:1`
  - Category: `oversized_refactor`
  - LOC estimate: 1058
  - Symbol: module
  - Summary: File exceeds the 750-line threshold and mixes command parsing,
    proposal construction, canonical claim conversion, deterministic parsers,
    entity helpers, and transcript activity helpers.
  - Evidence: `wc` reports 1,058 lines. Runtime imports a broad set of unrelated
    helpers from this one module.
  - References to check:
    `crates/noema-core/src/daemon/runtime/memory_writes.rs:27`,
    `crates/noema-core/src/daemon/runtime/transcript_persistence.rs:14`
  - Confidence: medium
  - Risk: medium

No credible `unused_dep` candidates were found within the scoped file.

### provider-adapters

Files read: `openai.rs`, `codex_responses.rs`, `responses.rs`, `sse.rs`,
`noema_response_stream.rs`, `codex_oauth.rs`, `foundation_local.rs`,
`foundation_bridge_process.rs`, `foundation_bridge_protocol.rs`, plus
`docs/project.md` and `docs/context/current.md`.

Findings:

- `provider-adapters-001` -
  `crates/noema-core/src/provider/adapters/sse.rs:286`
  - Category: `dead_code`
  - LOC estimate: 7
  - Symbol: `sse_events`
  - Summary: Unused compatibility parser wrapper.
  - Evidence: search finds only definition plus old plan docs; production
    streaming uses `SseAccumulator::push_bytes` and
    `next_sse_event_boundary`.
  - References to check:
    `docs/superpowers/plans/2026-06-28-realtime-agent-output-streaming.md`
  - Confidence: high
  - Risk: low

- `provider-adapters-002` -
  `crates/noema-core/src/provider/adapters/responses.rs:333`
  - Category: `over_abstraction`
  - LOC estimate: 19
  - Symbol: `ResponsesTransport::send_stream`
  - Summary: Thin unused wrapper over `send_streaming`.
  - Evidence: search finds no callers; Codex uses `send_streaming` directly.
  - References to check: external crate API exposure before removal.
  - Confidence: medium-high
  - Risk: low-medium

- `provider-adapters-003` -
  `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs:121`
  - Category: `legacy_dual_path`
  - LOC estimate: 21
  - Symbol: `FoundationBridgeProcess::generate`
  - Summary: One-shot generate path appears superseded by
    `FoundationLocalProvider` session reuse.
  - Evidence: `FoundationLocalProvider` calls
    `create_session`/`session_for_request` plus `generate_in_session`; search
    shows this method only used in adapter tests.
  - References to check: one-shot compaction/CLI transient Foundation behavior
    before removal.
  - Confidence: medium
  - Risk: medium

- `provider-adapters-004` -
  `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs:248`
  - Category: `speculative`
  - LOC estimate: 48
  - Symbol: `replay_turns`, `cancel_request`
  - Summary: Bridge replay/cancel methods are implemented and tested but have no
    runtime callers yet.
  - Evidence: search finds only definitions/tests and design docs. Current
    context says replay/cancel were intentionally exposed, so this may be staged
    work.
  - References to check: `docs/context/current.md`, Swift bridge support,
    future runtime cancellation/replay plan.
  - Confidence: medium
  - Risk: high

- `provider-adapters-005` -
  `crates/noema-core/src/provider/adapters/foundation_bridge_protocol.rs:64`
  - Category: `speculative`
  - LOC estimate: 7
  - Symbol: `BridgeRequestPayload::CloseSession`,
    `BridgeRequestPayload::Shutdown`
  - Summary: Protocol variants exist, Swift decodes them, but Rust adapter code
    does not construct them.
  - Evidence: search finds variants and Swift bridge handling, but no Rust
    process methods or callers in scoped adapter code.
  - References to check:
    `crates/noema-core/apple-foundation-bridge/Sources/NoemaFoundationBridge/main.swift`,
    daemon shutdown lifecycle.
  - Confidence: medium
  - Risk: high

- `provider-adapters-006` -
  `crates/noema-core/src/provider/adapters/openai.rs:250`
  - Category: `duplication`
  - LOC estimate: 82
  - Symbol: `log_malformed_response`, `log_malformed_response_raw`
  - Summary: OpenAI and Codex duplicate malformed Noema-envelope diagnostic
    logging.
  - Evidence: nearly identical block also exists in `codex_responses.rs:329`;
    shared `ResponsesDiagnosticContext` already logs transport/SSE malformed
    responses.
  - References to check: `required_output_items_from_text` failure logging
    semantics.
  - Confidence: high
  - Risk: low

- `provider-adapters-007` -
  `crates/noema-core/src/provider/adapters/openai.rs:562`
  - Category: `duplication`
  - LOC estimate: 220
  - Symbol: `CapturedRequest`, `spawn_server`, `read_request`,
    `parse_request`, `parse_content_length`
  - Summary: OpenAI and Codex adapter tests duplicate a local HTTP test server
    and parser.
  - Evidence: matching helper block exists in `codex_responses.rs:656`;
    `model_catalog.rs` has another similar test server.
  - References to check: shared test helper module gated to tests only.
  - Confidence: high
  - Risk: low-medium

- `provider-adapters-008` -
  `crates/noema-core/src/provider/adapters/foundation_local.rs:388`
  - Category: `duplication`
  - LOC estimate: 170
  - Symbol: bridge script test fixtures
  - Summary: Foundation Local and bridge-process tests repeat fake bridge shell
    scripts and helper setup.
  - Evidence: `foundation_bridge_process.rs:477` repeats
    handshake/health/generate shell fixtures and `bridge_script` helper.
  - References to check: shared fake Foundation bridge test fixture.
  - Confidence: medium-high
  - Risk: low-medium

- `provider-adapters-009` -
  `crates/noema-core/src/provider/adapters/codex_oauth.rs:1`
  - Category: `oversized_refactor`
  - LOC estimate: 861
  - Symbol: `codex_oauth` module
  - Summary: File exceeds the 750-line soft limit and mixes token storage,
    OAuth HTTP client, device auth orchestration, polling, error helpers, and
    tests.
  - Evidence: file is 861 lines; internal section boundaries are clear around
    `CodexTokenStore`, `CodexOAuthClient`, `start_codex_device_auth`, and tests.
  - References to check: `provider/auth.rs` and `provider/model_catalog.rs`
    imports.
  - Confidence: high
  - Risk: medium

- `provider-adapters-010` -
  `crates/noema-core/src/provider/adapters/foundation_bridge_process.rs:1`
  - Category: `oversized_refactor`
  - LOC estimate: 822
  - Symbol: `foundation_bridge_process` module
  - Summary: File exceeds the 750-line soft limit and combines process
    lifecycle, request dispatch, Swift build materialization, and large
    fake-bridge tests.
  - Evidence: file is 822 lines; tests alone occupy roughly lines 457-822.
  - References to check: split process runtime, build/materialization, and test
    fixture helpers.
  - Confidence: medium-high
  - Risk: medium

No credible `unused_dep` candidate found inside the scoped adapter files.

### memory

Files read: `memory.rs`, `memory/consolidation.rs`, `memory/error.rs`,
`memory/extraction.rs`, `memory/extraction/tests.rs`, `memory/model.rs`,
`memory/store.rs`, `memory/store_helpers.rs`, `memory/tests.rs`,
`memory/types.rs`.

Findings:

- `memory-001` - `crates/noema-core/src/memory/store.rs:1`
  - Category: `legacy_dual_path`
  - LOC estimate: 1250
  - Symbol: `MemoryStore` / `MemoryRetrievalRequest` / `MemoryItem`
  - Summary: In-memory retrieval policy store appears to be a legacy parallel
    path beside current SurrealDB graph-claim retrieval.
  - Evidence: runtime graph retrieval uses `ClaimRetrievalRequest`,
    `PolicyClaim`, and `claim_policy_allows` in `store/retrieval.rs`;
    `MemoryStore` is referenced only by its own module/tests and public
    re-export.
  - References to check: `crates/noema-core/src/store/retrieval.rs`,
    `crates/noema-core/src/lib.rs:21`
  - Confidence: high
  - Risk: medium

- `memory-002` - `crates/noema-core/src/memory/types.rs:71`
  - Category: `legacy_dual_path`
  - LOC estimate: 260
  - Symbol: `NewMemoryCandidate`, `MemorySummary`, `MemoryAuthorityLevel`,
    `MemoryExtractionMethod`
  - Summary: Old memory persistence DTOs remain exported but are not used by
    current graph-claim write/read paths.
  - Evidence: repo-wide search found these symbols only in `memory/types.rs`
    and `lib.rs` re-exports; current memory writes use `MemoryWriteProposal` and
    `store::NewClaimCandidate`.
  - References to check: `crates/noema-core/src/lib.rs:86`,
    `crates/noema-core/src/daemon/runtime/memory_writes.rs`
  - Confidence: high
  - Risk: medium

- `memory-003` - `crates/noema-core/src/memory/extraction.rs:429`
  - Category: `legacy_dual_path`
  - LOC estimate: 25
  - Symbol: `is_explicit_memory_command`
  - Summary: Extractor re-detects explicit memory commands using direct
    English/prefix matching instead of trusted structured runtime state.
  - Evidence: validation rejects proposals when user text starts with
    `/remember`, `remember this:`, or `remember that:`. This is semantic intent
    authority based on brittle text matching.
  - References to check:
    `crates/noema-core/src/daemon/runtime/memory_writes.rs` explicit-memory
    path.
  - Confidence: high
  - Risk: medium

- `memory-004` - `crates/noema-core/src/memory/extraction.rs:100`
  - Category: `speculative`
  - LOC estimate: 70
  - Symbol: `MemoryExtractionSubject::implies_local_human_for_evidence`
  - Summary: Local-human and third-party detection is hand-rolled with English
    names, kinship words, and ASCII tokenization.
  - Evidence: logic special-cases Kevin/current human/user/me plus English
    relation words. Runtime uses the exported helper through `memory_pipeline`,
    so this is not only test scaffolding.
  - References to check:
    `crates/noema-core/src/daemon/memory_pipeline.rs:956`
  - Confidence: high
  - Risk: high

- `memory-005` - `crates/noema-core/src/memory/consolidation.rs:546`
  - Category: `duplication`
  - LOC estimate: 120
  - Symbol: `sensitivity_json` / `memory_type_json`
  - Summary: Closed enum string mappings are duplicated across memory modules.
  - Evidence: `Sensitivity` serializer/parser is repeated in consolidation and
    extraction; `MemoryType` string mapping is duplicated between
    `MemoryType::as_str` and extraction parser.
  - References to check:
    `crates/noema-core/src/daemon/memory_pipeline.rs:865` has another
    `MemoryType` label table outside scope.
  - Confidence: high
  - Risk: low

- `memory-006` - `crates/noema-core/src/memory/consolidation.rs:591`
  - Category: `oversized_refactor`
  - LOC estimate: 375
  - Symbol: `consolidation::tests`
  - Summary: `consolidation.rs` is 965 lines mostly because tests live inline.
  - Evidence: file exceeds the project’s 750-line inspection threshold;
    extraction already uses a sibling `extraction/tests.rs` pattern.
  - References to check: `crates/noema-core/src/memory/extraction/tests.rs`
  - Confidence: medium
  - Risk: low

- `memory-007` - `crates/noema-core/src/memory/extraction.rs:264`
  - Category: `oversized_refactor`
  - LOC estimate: 500
  - Symbol: extraction prompt/validation/heuristics/serde modules
  - Summary: `extraction.rs` is a 975-line production file combining prompt
    construction, response partitioning, validation, sensitivity inference,
    identity heuristics, and serde vocabularies.
  - Evidence: this makes brittle semantic heuristics and reusable serde mappings
    harder to isolate or replace.
  - References to check: `memory-003`, `memory-004`, `memory-005`
  - Confidence: medium
  - Risk: medium

- `memory-008` - `crates/noema-core/src/memory/error.rs:4`
  - Category: `over_abstraction`
  - LOC estimate: 77
  - Symbol: `MemoryPersistenceError`
  - Summary: `MemoryPersistenceError` has become a cross-domain
    parse/object/provider error, despite current store errors being
    `StoreError`.
  - Evidence: references include IDs, objects, conversation enums, provider
    account parsing, daemon protocol, and CLI error conversion. Several uses are
    not memory persistence.
  - References to check: `crates/noema-core/src/objects.rs`,
    `crates/noema-core/src/conversation.rs`,
    `crates/noema-core/src/provider/accounts.rs`
  - Confidence: medium
  - Risk: low

No credible `unused_dep` candidate was found within the scoped files.

### provider-core

Files read in scope: `provider.rs`, `provider/accounts.rs`,
`provider/adapters.rs`, `provider/auth.rs`, `provider/contract.rs`,
`provider/model_catalog.rs`. The auditor also read `docs/project.md`,
`docs/context/current.md`, and used repo-wide search only for reference checks.

Findings:

- `provider-core-001` -
  `crates/noema-core/src/provider/accounts.rs:97`
  - Category: `duplication`
  - LOC estimate: 32
  - Symbol: `parse_auth_method`, `parse_account_status`
  - Summary: Public provider enum parsers appear unused and duplicate
    store-local parsers.
  - Evidence: repo-wide search found only the `lib.rs` re-export plus these
    definitions; `store/provider_accounts.rs` has separate
    `parse_provider_auth_method` and `parse_provider_account_status` with the
    same vocabulary.
  - References to check: `crates/noema-core/src/lib.rs:95`,
    `crates/noema-core/src/store/provider_accounts.rs:329`
  - Confidence: high
  - Risk: low

- `provider-core-002` -
  `crates/noema-core/src/provider/model_catalog.rs:227`
  - Category: `dead_code`
  - LOC estimate: 10
  - Symbol: `profiles_metadata`
  - Summary: Helper claims to support tests and seeded local providers, but has
    no live references.
  - Evidence: search for `profiles_metadata` outside this file returned no
    matches; Foundation Local seed metadata is hardcoded in
    `store/provider_accounts.rs`.
  - References to check: `crates/noema-core/src/store/provider_accounts.rs:101`
  - Confidence: high
  - Risk: low

- `provider-core-003` -
  `crates/noema-core/src/provider/model_catalog.rs:198`
  - Category: `speculative`
  - LOC estimate: 14
  - Symbol: `profile_value_from_model` metadata extras
  - Summary: Codex catalog stores `default_reasoning_effort` and
    `input_modalities`, but current GraphQL profile mapping only consumes id and
    label.
  - Evidence: `metadata_profiles` maps only id/label/disabledReason; search
    found no live consumers for `default_reasoning_effort` or `input_modalities`
    outside this producer and its unit test.
  - References to check: `crates/noema-core/src/graphql/agents.rs:249`
  - Confidence: medium
  - Risk: low

- `provider-core-004` -
  `crates/noema-core/src/provider/contract.rs:280`
  - Category: `legacy_dual_path`
  - LOC estimate: 61
  - Symbol: `output_items_from_structured_value`,
    `output_items_from_concatenated_json`
  - Summary: Required Noema responses are documented as strict envelopes, but
    parser also accepts implicit top-level output arrays and concatenated JSON
    envelopes, returning only the first envelope.
  - Evidence: docs say required mode needs strict `type=noema_response`; parser
    synthesizes that type when any output array exists and accepts multiple JSON
    values while discarding later outputs. Tests preserve these compatibility
    paths.
  - References to check: `crates/noema-core/src/daemon/prompts.rs:149`,
    `crates/noema-core/src/provider/adapters/foundation_local.rs:288`
  - Confidence: medium-high
  - Risk: medium

- `provider-core-005` -
  `crates/noema-core/src/provider/contract.rs:529`
  - Category: `speculative`
  - LOC estimate: 6
  - Symbol: `ProviderError::UnsupportedFeature`
  - Summary: Error variant is never constructed in live source, only handled in
    exhaustive matches.
  - Evidence: search found matches in turn error handling and Codex auth
    safe-message mapping, but no constructor outside the enum.
  - References to check: `crates/noema-core/src/daemon/runtime/turn.rs:559`,
    `crates/noema-core/src/provider/adapters/codex_oauth.rs:723`
  - Confidence: medium
  - Risk: low

- `provider-core-006` -
  `crates/noema-core/src/provider/contract.rs:1`
  - Category: `oversized_refactor`
  - LOC estimate: 762
  - Symbol: provider contract module
  - Summary: File exceeds the project’s 750-line soft threshold and combines
    trait/types, parser, error enum, and 216 lines of tests.
  - Evidence: file is 762 lines; test module starts at line 546. Parser
    compatibility is a natural split point if this file is cleaned.
  - Confidence: medium
  - Risk: low

No credible `unused_dep` candidate found in this scope.

### desktop

Files read in scope: `crates/noema-desktop/Cargo.toml`,
`desktop_state.rs`, `external_url.rs`, `graphql_ipc.rs`, `main.rs`,
`mcp_oauth_callback.rs`.

Findings:

- `desktop-cruft-001` -
  `crates/noema-desktop/src/mcp_oauth_callback.rs:38`
  - Category: `duplication`
  - LOC estimate: 165
  - Symbol: `mcp_oauth_callback::{handle_connection, read_request,
    parse_request_head, query_value, write_response}`
  - Summary: Desktop owns a second hand-rolled MCP OAuth callback HTTP path
    parallel to daemon web.
  - Evidence: it parses `GET /mcp/oauth/callback`, extracts `attemptId`,
    reconstructs `callback_url`, calls `complete_mcp_server_oauth_setup`, and
    emits the same success/failure HTML messages as daemon web.
  - References to check: `crates/noema-core/src/daemon/web/mod.rs:171`,
    `crates/noema-core/src/daemon/web/mod.rs:244`
  - Confidence: high
  - Risk: medium

- `desktop-cruft-002` -
  `crates/noema-desktop/src/desktop_state.rs:39`
  - Category: `speculative`
  - LOC estimate: 25
  - Symbol: `DesktopRuntime::mcp_oauth_callback_server`
  - Summary: The MCP OAuth loopback listener is started unconditionally during
    desktop app initialization.
  - Evidence: desktop startup calls `mcp_oauth_callback::start` before the app
    is usable and stores the listener until shutdown, even though the URL is
    only needed during hosted MCP OAuth setup. A bind failure currently makes
    runtime initialization fail.
  - References to check:
    `crates/noema-desktop/src/mcp_oauth_callback.rs:17`,
    `crates/noema-core/web/src/graphql/mcpOAuthCallback.ts:12`
  - Confidence: medium
  - Risk: medium

- `desktop-cruft-003` -
  `crates/noema-desktop/src/graphql_ipc.rs:102`
  - Category: `dead_test`
  - LOC estimate: 14
  - Symbol: `subscription_event_payload_keeps_subscription_id`
  - Summary: The test does not cover the frontend-facing serialization contract
    it appears to protect.
  - Evidence: frontend listens for `payload.subscriptionId`, and the Rust struct
    relies on serde rename, but the test only reads Rust field `subscription_id`
    and never serializes the payload.
  - References to check: `crates/noema-desktop/src/graphql_ipc.rs:12`,
    `crates/noema-core/web/src/graphql/desktopTransport.ts:7`
  - Confidence: medium
  - Risk: low

- `desktop-cruft-004` -
  `crates/noema-desktop/src/external_url.rs:24`
  - Category: `over_abstraction`
  - LOC estimate: 12
  - Symbol: `ExternalUrlError`
  - Summary: Error enum preserves distinctions that production code collapses
    to one user-facing message.
  - Evidence: invalid URL, open failure, and unsupported scheme all display
    `Noema could not open your browser.` Variants are mainly used by unit tests.
  - References to check: `crates/noema-desktop/src/external_url.rs:10`
  - Confidence: low
  - Risk: low

No credible `unused_dep`, `oversized_refactor`, or in-scope `dead_code`
candidates found. Every dependency in `crates/noema-desktop/Cargo.toml` has
direct in-scope usage.

### cli

Current state after the 2026-07-03 campaign Task 1 cleanup: `noema-cli` has
been deleted, so `cli-*` findings below are historical audit evidence rather
than active implementation opportunities.

Files read in scope: `crates/noema-cli/Cargo.toml`, `commands.rs`,
`commands/chat.rs`, `commands/config.rs`, `commands/memory.rs`,
`commands/start.rs`, `dev.rs`, `graphql/http.rs`, `graphql/mod.rs`,
`graphql/transcript.rs`, `graphql/ws.rs`, `graphql_client.rs`,
`inspection.rs`, `inspection_tests.rs`, `lib.rs`, `main.rs`.

Findings:

- `cli-001` - `crates/noema-cli/src/graphql_client.rs:1`
  - Category: `legacy_dual_path`
  - LOC estimate: 8
  - Symbol: `graphql_client` facade
  - Summary: Compatibility facade now just re-exports `crate::graphql` and
    keeps the old module name alive.
  - Evidence: file is only re-exports, including an allowed unused
    `GraphqlTurnEvent` re-export. Current call sites could import
    `crate::graphql` directly.
  - References to check: `commands/chat.rs:6`, `commands/memory.rs:6`,
    `inspection.rs:13`, `main.rs:20`
  - Confidence: high
  - Risk: low

- `cli-002` - `crates/noema-cli/src/inspection.rs:55`
  - Current status: resolved/superseded by Task 1 deletion of `noema-cli`; the
    command is no longer live or user-visible.
  - Category: `speculative`, `dead_test`
  - LOC estimate: 45
  - Symbol: `ContextCommand::Graph` / `run_context`
  - Summary: `noema context graph` is a reachable CLI scaffold that always
    returns unavailable.
  - Evidence: command exists, parse tests cover it, but execution only returns
    `context graph inspection is unavailable until graph retrieval lands`.
    Current docs say graph-native inspection should replace context graph
    inspection and list richer graph inspection as an open loop.
  - References to check: `main.rs:67`, `main.rs:139`, `main.rs:325`,
    `docs/context/current.md:311`,
    `docs/superpowers/specs/2026-06-28-surrealdb-graph-memory-store-design.md:469`
  - Confidence: medium-high
  - Risk: medium

- `cli-003` - `crates/noema-cli/src/inspection.rs:158`
  - Current status: resolved/superseded by Task 1 deletion of `noema-cli`; there
    is no remaining CLI GraphQL DTO surface to refactor.
  - Category: `duplication`
  - LOC estimate: 50
  - Symbol: `GraphqlMemoryClaim`/`GraphqlMemoryClaimDetail` and
    `GraphqlPredicateProposalSummary`/`GraphqlPredicateProposal`
  - Summary: CLI inspection duplicates GraphQL field selections and DTO fields
    across list/detail shapes.
  - Evidence: claim list/detail structs repeat most claim fields; predicate
    proposal summary/detail do the same. Query strings also repeat parallel
    field selections, creating schema-drift cleanup risk.
  - References to check: `inspection_tests.rs:5`, noema-core GraphQL memory
    schema/resolver tests before changing response assumptions.
  - Confidence: medium
  - Risk: low-medium

No credible `unused_dep`, `oversized_refactor`, or standalone `dead_code`
findings beyond the candidates above. The auditor excluded the direct one-shot
provider path because README and current context still describe one-shot
CLI/provider requests as intentional.

### frontend

Read or directly inspected: `docs/project.md`, `docs/context/current.md`,
`web/package.json`, `tsconfig.json`, `App.tsx`, `routes.ts`,
`graphql/operations.ts`, `SettingsPage.tsx`, settings pane wrappers/content,
transcript barrel/components, daemon asset `index.html`, asset references in
Rust.

Sampled/enumerated: `crates/noema-core/web/src/**`,
`crates/noema-core/web/src/components/{settings,transcript,memory,shell}/**`,
`crates/noema-core/web/src/graphql/**`,
`crates/noema-core/src/daemon/web/assets/**`.

Findings:

- `frontend-001` - `crates/noema-core/web/src/App.tsx:72`
  - Category: `dead_test`
  - LOC estimate: 9
  - Symbol: `shouldRouteThroughAppShell`
  - Summary: Exported shell-routing helper appears to be leftover from removed
    frontend unit tests.
  - Evidence: repo-wide source search finds only the definition. Superpowers
    plans reference it as a test target, but `crates/noema-core/web/tests` is
    gone.
  - References to check:
    `docs/superpowers/plans/2026-06-30-route-derived-shell-settings.md`
  - Confidence: high
  - Risk: low

- `frontend-002` - `crates/noema-core/web/src/routes.ts:66`
  - Category: `dead_test`
  - LOC estimate: 3
  - Symbol: `settingsFallbackRoute`
  - Summary: Exported route fallback helper is no longer used by runtime source.
  - Evidence: source search finds only the definition. Historical docs mention
    keeping it for tests/code, but current runtime uses `settingsBackNavigation`
    directly and web tests were removed.
  - References to check:
    `docs/superpowers/plans/2026-06-30-route-derived-shell-settings.md`,
    `docs/superpowers/plans/2026-06-30-settings-providers-page.md`
  - Confidence: high
  - Risk: low

- `frontend-003` -
  `crates/noema-core/web/src/components/transcript/index.ts:1`
  - Category: `dead_code`
  - LOC estimate: 4
  - Symbol: transcript barrel exports
  - Summary: Transcript folder barrel is not imported by in-scope source.
  - Evidence: import graph and search show active code imports
    `./components/Transcript` or direct transcript files, not
    `components/transcript/index.ts`.
  - References to check:
    `crates/noema-core/web/src/components/Transcript.tsx`,
    `crates/noema-core/web/src/components/ChatSurface.tsx`
  - Confidence: high
  - Risk: low

- `frontend-004` -
  `crates/noema-core/web/src/components/settings/AuditSettingsPaneContent.tsx:3`
  - Category: `speculative`
  - LOC estimate: 35
  - Symbol: `AuditSettingsPane` / `AuditSettingsPaneContent`
  - Summary: Route-visible Audit settings surface is only placeholder UI, with
    no frontend GraphQL audit operation/schema field found.
  - Evidence: Settings nav exposes Audit and `SettingsPage` renders
    `AuditSettingsPane`, but content only says records will appear later. Search
    for audit in operations/schema found no frontend read model.
  - References to check:
    `crates/noema-core/web/src/components/settings/AuditSettingsPane.tsx`,
    `crates/noema-core/web/src/components/shell/shellNavigation.ts`,
    `crates/noema-core/web/src/graphql/operations.ts`
  - Confidence: medium
  - Risk: medium

No credible scoped findings for `unused_dep`, `legacy_dual_path`,
`oversized_refactor`, or `dead_test` files beyond the test-leftover helper
exports above.

### cross-cutting-duplication

Files read / searches performed: `docs/project.md`, `docs/context/current.md`,
`docs/cruft-audit-plan.md`, cleanup plan/spec docs, root and crate
`Cargo.toml` files, targeted memory/store/daemon/MCP/GraphQL/CLI/web asset
files listed below. Searches included `git status --short --branch`,
`rg --files crates`, file line-count scan, duplicate symbol-name scan,
legacy/deprecated/dead-code marker scan, dependency reference scan,
`MemoryStore`/memory-pipeline/graph retrieval references, GraphQL operation
references, MCP enum/parser references, and embedded asset build-flow
references.

Findings:

- `ccd-001` - cross-cutting memory retrieval path
  - Files:
    - `crates/noema-core/src/memory.rs:1`
    - `crates/noema-core/src/memory/store.rs:9`
    - `crates/noema-core/src/memory/model.rs:291`
    - `crates/noema-core/src/store/retrieval.rs:44`
  - Lines: `memory.rs:1-21`, `memory/store.rs:9-20,176-198,333-654`,
    `memory/model.rs:291-726`, `store/retrieval.rs:44-153`
  - Category: `legacy_dual_path`
  - LOC estimate: 700-1050
  - Symbol: `MemoryStore`, `MemoryRetrievalRequest`, `MemoryItem`,
    `RetrievedMemory`
  - Summary: Old in-memory row-style memory retrieval store remains beside the
    live SurrealDB graph-claim retrieval path.
  - Evidence: repo search found `MemoryStore` used only by memory tests plus
    public re-export; live `search_memory` routes through
    `NoemaStore::retrieve_claims_scoped` and `ClaimRetrievalRequest`. Current
    context says graph claims are durable memory and `search_memory` returns
    claim-shaped results.
  - References to check: external/public `noema-core` consumers,
    `memory/tests.rs` policy coverage, `MemoryPersistenceError::MemoryStore`
  - Confidence: high repo-internal; needs-human for public crate API compatibility
  - Risk: medium

- `ccd-002` - cross-cutting memory write transition
  - Files:
    - `crates/noema-core/src/daemon/memory_pipeline.rs:88`
    - `crates/noema-core/src/daemon/runtime/memory_writes.rs:27`
  - Lines: `memory_pipeline.rs:88-162,634-681,730-944,948-980`,
    `runtime/memory_writes.rs:27-33,167-204`
  - Category: `legacy_dual_path`
  - LOC estimate: 90-140
  - Symbol: `explicit_memory_claim_candidate`, `provider_memory_claim_candidate`,
    `infer_chat_memory_type`, `title_from_memory_content`
  - Summary: Legacy memory candidate wrappers and staged helpers sit next to
    the live proposal/canonicalization write path.
  - Evidence: several symbols carry explicit dead-code reasons like legacy
    daemon tests exercise this wrapper while runtime writes use proposal
    routing; `runtime/memory_writes` imports proposal/canonical helpers, not the
    legacy candidate wrappers.
  - References to check: `daemon/tests.rs` references that only preserve wrapper
    behavior; any intended next-slice use of
    `infer_chat_memory_type`/`title_from_memory_content`
  - Confidence: high
  - Risk: low-medium

- `ccd-003` - cross-client GraphQL duplication
  - Current status: partially resolved/superseded by Task 1 deletion of
    `noema-cli`. Any future codegen strategy should target active web/desktop
    clients, not the deleted CLI.
  - Files:
    - `crates/noema-cli/src/graphql/mod.rs:19`
    - `crates/noema-cli/src/graphql/ws.rs:16`
    - `crates/noema-cli/src/inspection.rs:158`
    - `crates/noema-core/web/src/graphql/operations.ts:348`
  - Lines: `cli/graphql/mod.rs:19-41`, `cli/graphql/ws.rs:16-65`,
    `cli/inspection.rs:158-211,267-346`, `web/operations.ts:348-413,432-530`
  - Category: `duplication`
  - LOC estimate: 250-450
  - Symbol: `StartPrimaryConversation`, `SendConversationTurn`,
    `ConversationEvents`, `memoryClaim`
  - Summary: CLI and web maintain separate GraphQL operation documents and
    response decoders for overlapping first-party API surfaces.
  - Evidence: same chat and memory fields are selected in Rust string constants
    and TS GraphQL documents; web has generated TS types, while CLI manually
    deserializes serde structs and JSON `Value` transcript events.
  - References to check: generated GraphQL clients/schema, CLI output tests,
    whether Rust GraphQL codegen is acceptable.
  - Confidence: medium; needs-human/verification because GraphQL surfaces are
    live public client API.
  - Risk: medium

- `ccd-004` - cross-module string vocabulary duplication
  - Files:
    - `crates/noema-core/src/mcp.rs:28`
    - `crates/noema-core/src/graphql/mcp.rs:859`
    - `crates/noema-core/src/store/mcp.rs:1315`
    - `crates/noema-core/src/store/retrieval.rs:357`
    - `crates/noema-core/src/store/claims/labels.rs:8`
  - Lines: `mcp.rs:28-128`, `graphql/mcp.rs:859-969,999-1044`,
    `store/mcp.rs:1315-1340`, `store/retrieval.rs:357-400`,
    `store/claims/labels.rs:8-24`
  - Category: `duplication`
  - LOC estimate: 100-180
  - Symbol: `McpTransportKind`, `McpTrustClassification`,
    `ClaimStatusForPolicy`, `Sensitivity` parsers
  - Summary: String vocabularies are parsed/rendered in several modules instead
    of one authoritative typed parser per enum.
  - Evidence: MCP transport strings are mapped in `mcp.rs`, GraphQL input
    parsing, and store row parsing. Sensitivity parsing exists in
    `store/claims/labels.rs` and separately in `store/retrieval.rs`; claim
    lifecycle is duplicated as `ClaimStatus` and `ClaimStatusForPolicy`.
  - References to check: GraphQL string input compatibility, generated
    schema/types, persisted SurrealDB values.
  - Confidence: medium
  - Risk: medium

- `ccd-005` - embedded web assets
  - Files:
    - `crates/noema-core/web/vite.config.ts:33`
    - `crates/noema-core/src/daemon/web/assets.rs:43`
    - `crates/noema-cli/src/dev.rs:51`
    - `crates/noema-core/src/daemon/web/assets/app.js:1`
  - Lines: `vite.config.ts:33-48`, `assets.rs:43-64`,
    `dev.rs:51-53,226-233`, `assets/*` 87 minified lines total
  - Category: `duplication`
  - LOC estimate: 87 source lines, but large generated bundle bytes
  - Symbol: embedded web assets
  - Summary: Frontend source and generated daemon-served bundle both live under
    `crates/`.
  - Evidence: Vite writes to `../src/daemon/web/assets`; release builds
    `include_bytes!` those files; debug builds read generated files from disk;
    historically, the deleted `dev-daemon` watcher ignored asset changes to
    avoid Rust rebuild loops.
  - References to check: release packaging requirements, CI/build-from-clean
    behavior, whether generated assets should stay committed or be produced in
    build/packaging.
  - Confidence: medium as duplication; low as removable without a build-flow
    decision.
  - Risk: high if cut without replacement

No credible `unused_dep` candidate surfaced from the dependency usage scan;
every declared Rust dependency had repo references outside generated/asset
noise.

### core-misc

Files read: `docs/project.md`, `docs/context/current.md`, `docs/harness.md`,
`docs/harness/capabilities.md`, `docs/harness/runtime.md`,
`docs/harness/security.md`, `docs/cruft-audit-plan.md`,
`docs/cruft-audit-learnings.md`, plus every scoped file:
`crates/noema-core/src/lib.rs`, `runtime_host.rs`, `config.rs`,
`config/tests.rs`, `capability.rs`, `capability/gateway.rs`,
`capability/examination.rs`, `capability/ownership.rs`,
`crates/noema-core/Cargo.toml`.

Findings:

- `core-misc-001` - `crates/noema-core/src/capability.rs:17`
  - Category: `speculative`
  - LOC estimate: 315
  - Symbol: `CapabilityAxis`, `OwnerTrust`, `CapabilityPolicyInput`,
    `CapabilityPolicyDecision`, `evaluate_capability_policy`
  - Summary: Deterministic capability policy matrix is public and tested, but
    not wired into the active Capability Gateway execution path.
  - Evidence: repo-wide search finds production use only as re-exports; active
    gateway checks server/tool/calibration status directly in
    `capability/gateway.rs` and never calls `evaluate_capability_policy`.
  - References to check: `capability/gateway.rs`,
    `daemon/runtime/local_tools.rs`,
    `docs/superpowers/specs/2026-06-30-third-party-mcp-control-plane-design.md`
  - Confidence: high
  - Risk: medium

- `core-misc-002` -
  `crates/noema-core/src/capability/examination.rs:1`
  - Category: `speculative`
  - LOC estimate: 167
  - Symbol: `examine_read_result`, `ReadExaminationInput`,
    `ReadExaminationDecision`, `ReadExaminationOutcome`
  - Summary: Quarantined read-result examination is implemented and tested, but
    no runtime caller invokes it before MCP tool results are released.
  - Evidence: search finds only definitions, tests, lib/capability re-exports,
    and docs. `capability/gateway.rs` returns MCP payload directly on success.
  - References to check: `capability/gateway.rs:63`,
    `store/schema.rs` `quarantined_tool_results`, `store/tests/mcp.rs`
  - Confidence: high
  - Risk: medium-high

- `core-misc-003` -
  `crates/noema-core/src/capability/ownership.rs:1`
  - Category: `speculative`
  - LOC estimate: 51
  - Symbol: `resolve_owner_from_json`, `ResolvedOwner`
  - Summary: Deterministic owner extraction helper is not used by the gateway or
    MCP runtime path.
  - Evidence: search finds only definition, one unit test, re-export, and old
    plan docs.
  - References to check: `store/mcp.rs` owner extractor persistence,
    `graphql/mcp.rs` calibration setup, `capability/gateway.rs`
  - Confidence: high
  - Risk: medium

- `core-misc-004` -
  `crates/noema-core/src/capability/gateway.rs:21`
  - Category: `over_abstraction`
  - LOC estimate: 6
  - Symbol: `GatewayToolProposal.agent_id`, `GatewayToolProposal.scope_ids`
  - Summary: Active callers pass agent/scope context into the gateway, but the
    gateway explicitly discards both.
  - Evidence: `local_tools.rs` builds `scope_ids` and `agent_id`, then
    `gateway.rs` assigns them to `_`.
  - References to check: `daemon/runtime/local_tools.rs:30`,
    `daemon/runtime/local_tools.rs:55`, future grant/policy evaluation path.
  - Confidence: high
  - Risk: medium

- `core-misc-005` - `crates/noema-core/src/config.rs:233`
  - Category: `legacy_dual_path`
  - LOC estimate: 45
  - Symbol: `Config::load_codex`
  - Summary: Codex-specific config loader appears left over after
    provider-neutral `load`/`load_daemon` became the active paths.
  - Evidence: production callers use `Config::load` or `Config::load_daemon`;
    search finds `load_codex` only in config tests and its definition.
  - References to check: noema-cli chat/start/memory config calls,
    noema-desktop startup config.
  - Confidence: high
  - Risk: low-medium

- `core-misc-006` - `crates/noema-core/src/runtime_host.rs:71`
  - Category: `dead_test`
  - LOC estimate: 18
  - Symbol: `NoemaRuntimeHost::for_tests_with_store_and_runtime`
  - Summary: Test-only runtime-host constructor has no repo caller.
  - Evidence: search finds only its definition. Active desktop/daemon runtime
    host path is live via `NoemaRuntimeHost::start`.
  - References to check: `graphql/runtime_state.rs` test helpers, daemon
    runtime tests.
  - Confidence: medium-high
  - Risk: low

Unused dependency check: no credible `unused_dep` candidate in
`crates/noema-core/Cargo.toml`. Targeted searches found real uses for all
declared dependencies, including ambiguous ones: `ring` is used by MCP OAuth
randomness and GraphQL WebSocket SHA-1, `async-stream` by GraphQL chat
subscriptions, and `bytes` by MCP HTTP streaming.

## Phase 2 Verification Results

### verify-daemon-web

Verdicts:

- `DW-001`: `needs_human`
  - Evidence: provider-auth glue is defined in `daemon/web/provider_auth.rs`,
    re-exported from daemon web, and used by GraphQL onboarding. Old
    `/api/provider-auth` routes are gone, but this code is live.
  - Live references: `graphql/onboarding.rs:238`, `:256`, `:270`, `:275`;
    `graphql/schema.rs:301`, `:429`; `web/src/App.tsx:198`.
  - Suggested slice: move GraphQL-facing provider-auth orchestration out of
    `daemon/web` into GraphQL/provider-auth service code. Not deletion-safe.

- `DW-002`: `needs_human`
  - Evidence: replay conversion lives in `daemon/web/replay.rs` and is
    re-exported from daemon web, but GraphQL chat is the live consumer.
  - Live references: `graphql/chat.rs:206`, `:337`, `:343`; frontend consumes
    `startPrimaryConversation.replay` in `web/src/App.tsx:143`.
  - Suggested slice: move replay mapping beside `graphql/chat.rs` or a shared
    conversation transcript mapper. Not deletion-safe.

- `DW-003`: `needs_human`
  - Evidence: `daemon/web/mod.rs` has a large colocated test module covering
    live routes/helpers.
  - Suggested slice: split tests by module or move focused tests into
    module-local test files. Refactor-only.

- `DW-004`: `confirmed_cruft`
  - Evidence: `is_supported_product_route` is `#[cfg(test)]` only. Repo-wide
    source search found old `/api/chat/ws`, `/api/status`,
    `/api/onboarding/status`, `/api/provider-accounts`, and
    `/api/provider-auth` only in negative tests. `handle_connection` has no
    matching `/api/...` branches.
  - Suggested slice: remove `is_supported_product_route` and legacy negative
    route tests. Positive GraphQL route summary test may also be removable if
    individual route tests already cover those paths.

- `DW-005`: `needs_human`
  - Evidence: `ProviderAccountStatusStore`, `ProviderAuthAttemptPoller`, and
    `CodexDeviceAuthStarter` are live abstractions inside provider-auth
    orchestration/tests. Production impls/calls remain in `provider_auth.rs`.
  - Suggested slice: revisit after `DW-001`; replace boxed-future test-seam
    traits with concrete async helpers or narrower tests after moving the code.

### verify-store-claims

Verdicts:

- `store-claims-001`: `needs_human`
  - Evidence: `NoemaStore::find_consolidation_matches` is live production code
    called from `daemon/runtime/memory_writes.rs:396`. Tests cover live
    consolidation behavior in `store/tests/claims.rs:693`, `:719`, `:760`,
    `:808`, `:842`, `:858`. Duplicated query branch structure is real but not
    safe deletion.
  - Suggested slice: extract shared claim-match query/read mapping helpers while
    preserving exact-object/private-sensitivity/query-term branch behavior.

- `store-claims-002`: `needs_human`
  - Evidence: `store/claims/write.rs` is 896 lines and contains live write APIs:
    `create_or_reinforce_claim`, `relate_claims`, `supersede_claim`,
    `reinforce_matched_claim_by_id`, all called from
    `daemon/runtime/memory_writes.rs`.
  - Suggested slice: split relation writes, claim lifecycle writes,
    entity/evidence helpers, and fingerprinting into focused modules.

- `store-claims-003`: `keep`
  - Evidence: `NoemaStore::related_claims` has no production caller, but tests
    at `store/tests/claims.rs:903` and `:997` guard the live `relate_claims`
    persistence/idempotency path, which runtime memory consolidation calls at
    `daemon/runtime/memory_writes.rs:505`.
  - Classification note: tests guarding a live feature count as live references.

- `store-claims-004`: `needs_human`
  - Evidence: `EvidenceAuthority::{HumanCorrection, DocumentSource,
    WeakInference, SystemRule}` are not constructed by current Rust write paths,
    but schema allow-lists accept the strings in `store/schema.rs:326`, `:345`,
    `:364`. GraphQL exposes evidence authority as `string`, and generated
    frontend type is `authority: string`.
  - Suggested slice: decide whether correction/document/weak/system evidence is
    still planned; if not, trim Rust enum and SurrealDB authority allow-lists
    together.

### verify-store-general

Verdicts:

- `store-general-001`: `confirmed_cruft`
  - Evidence: `tool_invocations` / `quarantined_tool_results` appear as schema
    tables in `store/schema.rs:121` and schema tests in `store/tests/mcp.rs`,
    but no production store API or writes reference the table names. Adjacent
    live approval `tool_invocation_id` fields do not use these tables.
  - Suggested slice: remove both schema table definitions/indexes and their
    dedicated schema-only tests; leave roadmap docs alone.

- `store-general-002`: `confirmed_cruft`
  - Evidence: `store::objects` is only exported from `store.rs` and aliases
    crate-root refs in `store/objects.rs`. Repo-wide search found no imports of
    `store::objects`, `crate::store::objects`, or
    `noema_core::store::objects`. Live refs are crate-root `ActorRef` /
    `ObjectRef`.
  - Suggested slice: delete `store/objects.rs` and remove `pub mod objects`;
    do not touch crate-root `objects.rs`.

- `store-general-003`: `needs_human`
  - Evidence: `store::mcp` is live; GraphQL, MCP setup, Capability Gateway, and
    runtime prompt tool exposure all call it.
  - Suggested slice: refactor, not delete. Split server/tool/calibration/trusted
    identity/approval rows and methods into focused store submodules.

- `store-general-004`: `needs_human`
  - Evidence: duplication is real but live. `agent_record_fragment` and
    `mcp_record_fragment` duplicate prefixed hex encoding. `ids::record_fragment`
    has different lossy semantics and is live elsewhere.
  - Suggested slice: introduce a small prefixed-hex helper and replace the two
    duplicated hex functions; leave `ids::record_fragment` unchanged unless
    separately redesigned.

- `store-general-005`: `confirmed_cruft`
  - Evidence: removable part is absent-table values inside evidence
    `source_object_type` assertions: `memory_item`, `relationship`,
    `context_packet` in `store/schema.rs:324`, `:343`, `:362`. Current
    production evidence writes `source_kind = 'item'` and
    `source_object_type = NONE`; tests only validate object source with
    `conversation`.
  - Suggested slice: remove only `memory_item`, `relationship`, and
    `context_packet` from the three evidence assertion lists; add/update schema
    tests to reject those values.

### verify-daemon-runtime

Verdicts:

- `DR-001`: `confirmed_cruft`
  - Evidence: cache storage is inert; the first tool refresh is discarded, then
    immediately repeated. Tool rendering itself is live and must remain.
  - Suggested slice: remove only duplicate first refresh plus cached
    `tool_snapshot` storage, `CachedToolSnapshot.hash`, and `stable_hash`; keep
    one render pass for prompts.

- `DR-002`: `confirmed_cruft`
  - Evidence: `configured_provider_kind` only returns `default_provider_kind`;
    sole caller is live `provider_kind()`.
  - Suggested slice: inline `provider_kind()` to return
    `&self.default_provider_kind`; delete private wrapper.

- `DR-003`: `needs_human`
  - Evidence: correctness/privacy policy issue, not cruft. Turn generation
    resolves selected provider via conversation provider, but memory
    canonicalization/consolidation use `self.default_provider()`. Tests prove
    agent preference can route turns to `foundation_local` with Codex as
    default.
  - Suggested slice: decide maintenance-model policy. If selected-provider
    privacy is intended, pass conversation provider into
    canonicalization/consolidation and add two-provider regression test.

- `DR-004`: `needs_human`
  - Evidence: correctness/API semantics issue. Nonempty `conversation_model`
    forces default provider; saved preference is only consulted after that
    branch. GraphQL/CLI expose model override.
  - Suggested slice: clarify whether model override is provider-local. Likely
    fix: keep saved provider when only model is overridden, or make provider
    override explicit.

- `DR-005`: `keep`
  - Evidence: `CodexRuntimeActor` / `CodexRuntimeCommand` names are
    Codex-specific, but the types are live core runtime internals with broad
    references from runtime host, GraphQL runtime state, and tests.
  - Suggested slice: optional naming cleanup only, coordinated runtime refactor
    if desired.

- `DR-006`: `confirmed_cruft`
  - Evidence: conversation-specific abstraction is ahead of current state;
    helper ignores conversation id and always loads `agent:primary`. Identity
    lookup itself is live.
  - Suggested slice: remove unused parameter and rename helper to primary-agent
    identity while keeping the lookup.

### verify-graphql

Verdicts:

- `GQL-001`: `confirmed_cruft`
  - Evidence: `graphql.rs` declares `mod types`; `types.rs` only re-exports
    under `#[allow(unused_imports)]`. No source use found.
  - Suggested slice: delete `graphql/types.rs` and remove `mod types;`.

- `GQL-002`: `keep`
  - Evidence: `resolvers.rs` is test-only, but tests query live `localStatus`.
    Schema and frontend still expose/query `localStatus`.
  - Suggested slice: do not delete coverage. Optional cleanup: move tests beside
    `local_status.rs` or schema tests, then remove empty `resolvers` module.

- `GQL-003`: `needs_human`
  - Evidence: `schema.rs` contains a 2.6k-line test module covering live schema,
    MCP, memory graph, chat, subscriptions, provider accounts, and agents.
  - Suggested slice: human-approved refactor splitting tests into focused
    `graphql/*_tests.rs` modules without deleting coverage.

- `GQL-004`: `needs_human`
  - Evidence: `GraphqlMcpSetupTransport` is live and called by MCP create/setup
    flows; not removable.
  - Suggested slice: move MCP setup transport construction/fakes out of schema
    root into MCP setup-oriented GraphQL/module code.

- `GQL-005`: `needs_human`
  - Evidence: `graphql/mcp.rs` is live and used by frontend operations and
    schema root.
  - Suggested slice: mechanical split preserving generated API names:
    `mcp/types.rs`, `mcp/setup.rs`, `mcp/resolvers.rs`, `mcp/parsing.rs`.

- `GQL-006`: `needs_human`
  - Evidence: public GraphQL fields `entity_id`, `redacted`, and
    `fact_redacted` are currently always redundant/false, but frontend
    operations and generated types request/include them.
  - Suggested slice: API decision required. If approved, remove fields from
    Rust GraphQL objects, update operations, regenerate schema/types, and adjust
    memory graph types.

- `GQL-007`: `confirmed_cruft`
  - Evidence: two private memory graph lookup helpers have identical
    lookup/clone logic and differ only panic text; only same-file private callers.
  - Suggested slice: consolidate to one private helper and update edge/node call
    sites.

- `GQL-008`: `needs_human`
  - Evidence: `AssistantConnection::Codex` is public GraphQL schema/type
    surface and frontend still queries it, but resolver always returns Codex and
    multi-provider direction makes semantics stale.
  - Suggested slice: API decision needed. Remove/replace with provider-aware
    status or keep until clients no longer query it; regenerate frontend types
    if changed.

### verify-daemon-core

Verdicts:

- `DC-001`: `keep`
  - Evidence: `explicit_memory_request` is called when building live
    `ClaimRetrievalRequest`, and flag is consumed by graph claim policy for
    private/sensitive/secret access. It may violate project standard against
    English phrase intent authority, but it is live behavior, not deletable
    cruft.

- `DC-002`: `keep`
  - Evidence: `SearchMemoryArguments.purpose` is parsed/validated; live prompt
    emits `purpose`; `build_request` validates it before forcing answer mode.
    Over-broad but part of live tool-call contract.

- `DC-003`: `confirmed_cruft`
  - Evidence: `context_packet_id` is fabricated and returned in tool JSON, but
    `retrieve_claims_scoped` does not persist packets. Source refs are
    definition/tests/docs only; daemon test only asserts presence.
  - Suggested slice: remove `context_packet_id`,
    `sanitize_context_packet_fragment`, JSON field, and tests/docs implying this
    path records a context packet.

- `DC-004`: `resolved_by_task_1`
  - Current status: resolved by Task 1 deletion of the raw daemon socket
    client/server surface.
  - Evidence: `DaemonClient::{start_conversation, turn, turn_streaming}` have
    no in-repo product caller, but are public API/protocol. CLI chat uses
    GraphQL.
  - Superseded slice: Task 1 decided raw daemon socket clients are unsupported
    pre-V1 and removed the matching protocol branches/tests.

- `DC-005`: `resolved_by_task_1`
  - Current status: resolved by Task 1 deletion of the raw request/response
    protocol surface.
  - Evidence: `DaemonRequest::ConversationStart.instructions` has no meaningful
    in-repo use, but is public serialized protocol. Client sends `None`; server
    ignores it.
  - Suggested slice: if raw daemon protocol compatibility is not needed, remove
    field and update client/server protocol tests.

- `DC-006`: `resolved_by_task_1`
  - Current status: resolved by Task 1 deletion of public raw socket helpers.
  - Evidence: `socket_path_for_home` duplicates `NoemaPaths` socket
    construction and only in-repo call is a daemon test, but helper is publicly
    exported.
  - Suggested slice: if public helper stability is not required, remove helper
    and re-export; use `NoemaPaths` in tests.

- `DC-007`: `resolved_by_task_1`
  - Current status: resolved by Task 1 protocol/error cleanup.
  - Evidence: `DaemonError::Memory` has no in-repo construction outside variant;
    runtime memory paths use `Store`, `Provider`, or `Protocol`; but
    `DaemonError` is public.
  - Suggested slice: if public error-shape compatibility is not needed, remove
    `MemoryPersistenceError` import and `Memory` variant.

- `DC-008`: `confirmed_cruft`
  - Evidence: whole-file `#![allow(dead_code)]` suppresses a live
    `agent_name_tool` module integrated through local-tool dispatch.
  - Suggested slice: remove only `#![allow(dead_code)]`, then run focused Rust
    validation.

### verify-memory-persistence

Verdict:

- `memory-persistence` scope: `confirmed_cruft`
  - Status: already removed; no current in-scope deletion remains.
  - Evidence: no tracked `crates/noema-core/src/memory_persistence.rs` or
    `crates/noema-core/src/memory_persistence/*`; no filesystem path; no live
    module references in `crates/` or `Cargo.toml`; commit
    `94986f35806a22239efeb466e45ef131d4ea45f2` deleted the legacy files.
    Current context confirms memory writes/read paths use SurrealDB graph
    claims.
  - Removable LOC now: 0.
  - Routing: follow-up refs belong to other scopes:
    - `MemoryStore` / `MemoryItem`: `memory-001`
    - old memory DTO exports: `memory-002`
    - `MemoryPersistenceError`: `memory-008` / cross-cutting
    - legacy object names in schema/objects: `store-general-005` /
      cross-cutting

### verify-provider-core

Verdicts:

- `provider-core-001`: `confirmed_cruft`
  - Evidence: `parse_auth_method` / `parse_account_status` are only defined
    and re-exported. Store uses separate local parsers.
  - Suggested slice: remove provider parser functions and `lib.rs` re-exports.

- `provider-core-002`: `confirmed_cruft`
  - Evidence: `profiles_metadata` has no repo refs outside definition and audit
    docs. Foundation Local seed metadata is inline, not using helper.
  - Suggested slice: remove helper.

- `provider-core-003`: `confirmed_cruft`
  - Evidence: `default_reasoning_effort` and `input_modalities` are produced and
    unit-tested only. GraphQL metadata profiles map only `id`, `label`,
    `disabledReason`; generated frontend types expose only those fields.
  - Suggested slice: stop storing extras and remove matching test assertions.

- `provider-core-004`: `needs_human`
  - Evidence: compatibility branches are production-reachable through live
    adapters. Runtime requires structured responses, but static analysis cannot
    prove real provider outputs never use implicit output arrays or duplicate
    concatenated envelopes. Existing tests explicitly preserve both behaviors.
  - Suggested slice: separate parser-hardening decision only after confirming
    provider traces. Do not include in deletion slice.

- `provider-core-005`: `confirmed_cruft`
  - Evidence: `ProviderError::UnsupportedFeature` is never constructed
    repo-wide; it is only handled in exhaustive matches.
  - Suggested slice: remove enum variant and match arms.

- `provider-core-006`: `keep`
  - Evidence: `provider/contract.rs` is 762 lines but public trait/types,
    parser, error enum, and tests are live.
  - Suggested slice: optional later refactor only.

Suggested low-risk provider-core deletion slice: `provider-core-001`,
`provider-core-002`, `provider-core-003`, `provider-core-005`.

### verify-mcp

Verdicts:

- `MCP-001`: `confirmed_cruft`
  - Evidence: `McpToolSchema` is defined and publicly re-exported, but has no
    Rust production/test consumer. GraphQL/web use `McpToolRecord` to
    `GraphqlMcpTool` fields instead.
  - Suggested slice: delete `McpToolSchema` and remove its `lib.rs` re-export.

- `MCP-002`: `confirmed_cruft`
  - Evidence: `FakeMcpTransport::call_tool` is test-only,
    `#[allow(dead_code)]`, and has no callers. The live trait implementation
    already provides `call_tool`.
  - Suggested slice: delete the inherent helper and its `#[allow(dead_code)]`.

- `MCP-003`: `needs_human`
  - Evidence: RMCP tool-call argument/result helper duplication is real, but
    both stdio and HTTP/SSE copies are live; Gateway executes all transports.
  - Suggested slice: optional refactor extracting shared RMCP `tools/call`
    argument/result helpers while preserving SSE JSON-RPC behavior.

- `MCP-004`: `needs_human`
  - Evidence: JSON config parser duplication is real but live across setup and
    runtime transport construction; error types differ.
  - Suggested slice: optional shared config parser with caller-specific error
    mapping.

- `MCP-005`: `needs_human`
  - Evidence: `mcp/http.rs` is oversized but live. GraphQL setup and Capability
    Gateway construct/execute both SSE and Streamable HTTP; web setup and
    generated schema still expose `sse`.
  - Suggested slice: refactor only; split Streamable HTTP, SSE, OAuth/header
    helpers, and shared parsing. Do not remove SSE.

- `MCP-006`: `needs_human`
  - Evidence: `mcp/setup.rs` is oversized but production setup is live through
    GraphQL create/continue/OAuth paths and web/desktop callbacks.
  - Suggested slice: move inline tests to focused test module; no behavior
    deletion.

Suggested low-risk MCP deletion slice: `MCP-001`, `MCP-002`.

### verify-memory-pipeline

Verdicts:

- `MP-001`: `confirmed_cruft`
  - Evidence: `explicit_memory_claim_candidate` and
    `provider_memory_claim_candidate` are legacy wrappers used only by daemon
    tests. Runtime memory writes use proposal/canonical helpers.
  - Suggested slice: remove the wrappers and convert any wrapper-only tests
    that protect live behavior to proposal/canonical APIs, or rely on existing
    full-turn tests.

- `MP-002`: `confirmed_cruft`
  - Evidence: staged helpers `infer_chat_memory_type` and
    `title_from_memory_content` have no source callers.
  - Suggested slice: delete the helpers with their dead-code allowances.

- `MP-003`: `keep`
  - Evidence: duplicated/brittle prefix parsers are live through explicit and
    provider fallback claims.

- `MP-004`: `keep`
  - Evidence: `explicit_memory_content` is live behavior through the runtime
    turn path. Phrase matching is a design issue, not a deletion-safe slice.

- `MP-005`: `keep`
  - Evidence: the `provider_subject_entity` helper chain is live through
    `provider_memory_write_proposal`; optional annotation cleanup may become
    available after removing wrapper cruft.

- `MP-006`: `keep`
  - Evidence: `daemon/memory_pipeline.rs` is oversized but broadly imported by
    live code. Refactor only after pruning.

Suggested memory-pipeline deletion slice: `MP-001`, `MP-002`.

### verify-memory

Verdicts:

- `memory-001`: `confirmed_cruft`
  - Evidence: old `MemoryStore` retrieval path is only defined, re-exported,
    and test-used. Live retrieval is graph-claim based through
    `store/retrieval.rs` and `daemon/memory_tool.rs`.
  - Suggested slice: delete `MemoryStore`, `memory/store.rs`,
    `memory/store_helpers.rs`, old `MemoryRetrievalRequest`/`MemoryItem`
    model pieces, and tests that only cover the deleted store path.

- `memory-002`: `confirmed_cruft`
  - Evidence: old DTOs such as `NewMemoryCandidate`, `MemorySummary`,
    `MemoryAuthorityLevel`, `MemoryExtractionMethod`, and participant/subject
    DTOs are only defined in `memory/types.rs` and root-re-exported. No
    GraphQL/frontend/CLI refs were found.
  - Exception: `MemoryType` is live and must stay.

- `memory-003`: `keep`
  - Evidence: `is_explicit_memory_command` is private but live through
    validation/partition logic used by provider memory persistence.
  - Note: brittle English matching should be replaced only with structured
    runtime state or policy, not deleted alone.

- `memory-004`: `keep`
  - Evidence: local-human subject heuristics are live in extraction and graph
    write subject selection.
  - Note: violates the semantic-intent standard, but needs replacement policy
    or model path before removal.

- `memory-005`: `confirmed_cruft`
  - Evidence: duplicate closed-vocabulary serializers and label mappings exist
    for `Sensitivity` and `MemoryType` across extraction, consolidation,
    `types.rs`, and `daemon/memory_pipeline.rs`.
  - Suggested slice: consolidate shared vocab serde/label helpers while
    preserving wire behavior and tests.

- `memory-006`: `keep`
  - Evidence: inline `consolidation::tests` cover live public serde and prompt
    parsers. Moving tests may help file size, but not removal.

- `memory-007`: `keep`
  - Evidence: `memory/extraction.rs` is broad but live/public and is used by
    provider response parsing and daemon runtime persistence.

- `memory-008`: `keep`
  - Evidence: `MemoryPersistenceError` is misnamed but live across public
    parsing, daemon protocol, and CLI conversions. Rename/split later.

Suggested memory deletion slice: `memory-001`, `memory-002`. Treat
`memory-005` as a separate behavior-preserving refactor slice.

### verify-provider-adapters

Verdicts:

- `provider-adapters-001`: `confirmed_cruft`
  - Evidence: crate-private `sse_events` is only defined with a dead-code
    allowance. Live streaming uses the accumulator path.
  - Suggested slice: delete `sse_events` and its `#[allow(dead_code)]`.

- `provider-adapters-002`: `confirmed_cruft`
  - Evidence: `ResponsesTransport::send_stream` is an unused wrapper while
    `send_streaming` is live.
  - Suggested slice: remove `send_stream`; keep `send_streaming`.

- `provider-adapters-003`: `confirmed_cruft`
  - Evidence: `FoundationBridgeProcess::generate` one-shot wrapper is unused.
    Live Foundation Local uses sessions/`generate_in_session`, and CLI one-shot
    generation goes through provider-level `generate`.
  - Suggested slice: remove the wrapper.

- `provider-adapters-004`: `needs_human`
  - Evidence: `replay_turns` and `cancel_request` are only in definitions/tests,
    but Swift bridge support exists and current context mentions replay-based
    resume.
  - Decision needed: wire these lifecycle APIs or cut them with the matching
    bridge protocol surface.

- `provider-adapters-005`: `needs_human`
  - Evidence: `CloseSession` and `Shutdown` protocol variants are decoded and
    handled by Swift, but Rust never constructs them.
  - Decision needed: same lifecycle product decision as
    `provider-adapters-004`.

- `provider-adapters-006`: `keep`
  - Evidence: duplicated malformed logging helpers are live; refactor only.

- `provider-adapters-007`: `keep`
  - Evidence: duplicated HTTP test helpers are live; optional consolidation.

- `provider-adapters-008`: `keep`
  - Evidence: fake bridge fixtures support live tests; optional consolidation.

- `provider-adapters-009`: `keep`
  - Evidence: `codex_oauth` is live; refactor only.

- `provider-adapters-010`: `keep`
  - Evidence: `foundation_bridge_process` is live; refactor only.

Suggested provider-adapters deletion slice: `provider-adapters-001`,
`provider-adapters-002`, `provider-adapters-003`. Do not include staged bridge
lifecycle protocol decisions.

### verify-desktop

Verdicts:

- `desktop-cruft-001`: `confirmed_cruft`
  - Evidence: desktop MCP OAuth callback code is live, but duplicates daemon-web
    callback handling: request parsing, `attemptId` extraction, callback URL
    reconstruction, OAuth completion, and HTML responses.
  - Classification note: refactor-only, not deletion-safe. Extract shared
    callback completion/response logic; keep the desktop listener.

- `desktop-cruft-002`: `needs_human`
  - Evidence: desktop eagerly starts the MCP OAuth callback listener during
    runtime initialization, while the frontend only asks for callback URL when
    starting MCP OAuth.
  - Decision needed: lazy startup appears plausible, but changes failure timing
    and callback URL lifecycle.

- `desktop-cruft-003`: `confirmed_cruft`
  - Evidence: the frontend wire contract is `payload.subscriptionId`; Rust uses
    serde rename, but the current test only reads the Rust field directly and
    does not protect the serialized wire key.
  - Suggested slice: delete the ineffective test or replace it with a real JSON
    serialization contract test.

- `desktop-cruft-004`: `keep`
  - Evidence: `ExternalUrlError` is private, small, and live across invalid URL,
    unsupported scheme, and browser-launch failure paths. Display intentionally
    collapses messages while tests use variants for precision.

Suggested desktop deletion slice: replace or remove the ineffective
`desktop-cruft-003` serialization test. `desktop-cruft-001` is a refactor slice;
`desktop-cruft-002` needs a product decision.

### verify-cli

Verdicts:

- `cli-001`: `confirmed_cruft`
  - Evidence: `graphql_client.rs` is only a compatibility facade re-exporting
    `crate::graphql` symbols. The real exports already live in
    `graphql/mod.rs`; references are limited to CLI internals.
  - Suggested slice: delete `graphql_client.rs` after mechanically rewriting
    CLI imports to `crate::graphql`.

- `cli-002`: `resolved_by_task_1`
  - Current status: resolved/superseded by Task 1 deletion of `noema-cli`; this
    is no longer a live/user-visible surface.
  - Historical evidence: at audit time, `noema context` was live and
    user-visible, but execution was an unavailable stub. Graph memory inspection
    existed elsewhere through GraphQL `memory_graph` and the frontend memory
    graph page.
  - Superseded decision: Task 1 removed the CLI surface.

- `cli-003`: `confirmed_cruft`
  - Current status: resolved/superseded by Task 1 deletion of `noema-cli`; no
    CLI DTO consolidation remains.
  - Evidence: CLI list/detail GraphQL selections and DTOs repeat claim and
    proposal fields. Paths are live through `run_memory` and output writers.
  - Classification note: refactor-only, not deletion-safe. Proposal list
    intentionally omits heavy JSON fields.

Superseded note: the earlier suggested CLI deletion slice is obsolete. The
campaign decision removed the whole CLI, including `cli-001`, `cli-002`, and
`cli-003`.

### verify-frontend

Verdicts:

- `frontend-001`: `confirmed_cruft`
  - Evidence: `shouldRouteThroughAppShell` has no live source reference beyond
    its definition. Runtime routing uses explicit route branches into
    `AppShell`.
  - Suggested slice: remove the helper export.

- `frontend-002`: `confirmed_cruft`
  - Evidence: `settingsFallbackRoute` has no live source reference beyond its
    definition. Current back navigation uses `settingsBackNavigation`
    directly.
  - Suggested slice: remove the helper export.

- `frontend-003`: `confirmed_cruft`
  - Evidence: `components/transcript/index.ts` is not imported by source. Active
    chat imports the wrapper and direct transcript component elsewhere.
  - Suggested slice: remove the unused barrel file only.

- `frontend-004`: `keep`
  - Evidence: the Audit settings pane is live route-visible UI: route model,
    settings route, shell navigation, and settings page all expose/render it.
    It is placeholder-only and has no web GraphQL operation, but deletion needs
    an explicit product decision.

Suggested frontend deletion slice: `frontend-001`, `frontend-002`, and
`frontend-003`. Do not include the Audit settings surface without a product
decision.

### verify-cross-cutting-duplication

Verdicts:

- `ccd-001`: `needs_human`
  - Evidence: the old `MemoryStore` path is not live internally, and live
    `search_memory` uses graph claim retrieval. However, `memory` is public
    crate surface and `MemoryPersistenceError::MemoryStore` still exposes the
    old error type.
  - Decision needed: internally removable, but API-breaking.

- `ccd-002`: `confirmed_cruft`
  - Evidence: legacy memory candidate wrappers are only referenced by daemon
    tests and definitions; runtime writes use the live proposal path. The staged
    helpers `infer_chat_memory_type` and `title_from_memory_content` have no
    source refs outside definitions.
  - Suggested slice: remove wrappers, staged helpers, and tests that exist only
    to preserve those wrappers.

- `ccd-003`: `cli_portion_resolved_by_task_1`
  - Current status: CLI half resolved/superseded by Task 1 deletion of
    `noema-cli`; future work should consider active web/desktop GraphQL
    codegen only.
  - Historical evidence: duplicated CLI/web GraphQL operation and DTO surfaces
    were real while both clients were live.
  - Superseded decision: Task 1 removed the CLI half. Future codegen decisions
    should target active web/desktop clients.

- `ccd-004`: `confirmed_cruft`
  - Evidence: duplicate string vocabulary parsers are confirmed across MCP
    model, GraphQL MCP, store MCP, and sensitivity parsing locations.
  - Classification note: behavior-preserving refactor, not deletion-safe as-is.

- `ccd-005`: `needs_human`
  - Evidence: built web assets are emitted into Rust source and embedded for
    release builds; debug builds read the generated directory from disk. The dev
    daemon intentionally ignores that generated path.
  - Decision needed: replacement release packaging/build-from-clean flow before
    deleting tracked assets.

Suggested cross-cutting deletion slice: `ccd-002` only. Treat `ccd-004` as a
follow-up refactor slice, not a pure cut.

### verify-core-misc

Verdicts:

- `core-misc-001`: `confirmed_cruft`
  - Evidence: `evaluate_capability_policy` is defined, tested, and re-exported,
    with no production caller. The live gateway performs direct
    server/auth/calibration checks and never calls the policy matrix.
  - Decision needed: current-behavior deletion is safe, but this encodes
    unlanded capability policy, so delete-vs-wire needs product/security intent.

- `core-misc-002`: `confirmed_cruft`
  - Evidence: `examine_read_result` is defined, unit-tested, and re-exported,
    with no runtime caller. The gateway releases successful MCP payloads
    directly.
  - Decision needed: current-behavior deletion is safe, but this is a
    security/quarantine policy decision.

- `core-misc-003`: `confirmed_cruft`
  - Evidence: `resolve_owner_from_json` and `ResolvedOwner` are only defined,
    tested, and re-exported. No gateway/runtime caller applies persisted owner
    extractors.
  - Decision needed: owner extractors themselves are live data/API through MCP
    store, GraphQL, and autofill. Delete-vs-wire is a product/security
    decision.

- `core-misc-004`: `confirmed_cruft`
  - Evidence: `GatewayToolProposal.agent_id` and `scope_ids` are passed by the
    active runtime and immediately discarded by the gateway.
  - Suggested slice: remove fields and constructor args as a behavior-preserving
    refactor, separate from policy wiring.

- `core-misc-005`: `confirmed_cruft`
  - Evidence: `Config::load_codex` has no caller. Live callers use
    `Config::load_daemon` or `Config::load`, while Codex provider resolution
    happens through `RawConfig::resolve`.
  - Suggested slice: delete the unused method.

- `core-misc-006`: `confirmed_cruft`
  - Evidence: `NoemaRuntimeHost::for_tests_with_store_and_runtime` has no source
    refs outside its definition. Live host construction uses
    `NoemaRuntimeHost::start`.
  - Suggested slice: delete the unused constructor.

Suggested core-misc deletion slice: remove `Config::load_codex`,
`NoemaRuntimeHost::for_tests_with_store_and_runtime`, and optionally the unused
`GatewayToolProposal.agent_id` / `scope_ids` fields plus constructor args. Keep
`core-misc-001` through `core-misc-003` out of the first low-risk slice until
the gateway policy/quarantine decision is made.

## Final Synthesis

### Low-risk deletion slices ready to implement

1. Memory-pipeline legacy wrappers
   - Scope IDs: `MP-001`, `MP-002`, `ccd-002`
   - Scope: `daemon/memory_pipeline.rs`, wrapper-only tests in
     `daemon/tests.rs`
   - Safe because: runtime writes use proposal/canonical helpers; wrappers and
     staged helpers are definition/test-only.
   - Validation: full Rust validation; focused daemon memory-write and turn
     tests.

2. Schema cruft and aliases
   - Scope IDs: `store-general-001`, `store-general-002`,
     `store-general-005`
   - Scope: `store/schema.rs`, `store/objects.rs`, schema-only MCP/evidence
     tests
   - Safe because: tables/aliases/legacy evidence object values have no
     production callers or writes.
   - Validation: full Rust validation; focused store schema/MCP/claims tests.

3. Small runtime cleanup
   - Scope IDs: `DR-001`, `DR-002`, `DR-006`
   - Scope: `daemon/runtime/turn.rs`, `daemon/runtime/handle.rs`
   - Safe because: removes inert cache state, a forwarding wrapper, and an
     ignored parameter while keeping live lookups/rendering.
   - Validation: full Rust validation; focused daemon runtime/tool rendering
     tests.

4. GraphQL private cruft
   - Scope IDs: `GQL-001`, `GQL-007`
   - Scope: `graphql/types.rs`, `graphql/memory.rs`
   - Safe because: `types.rs` is unused re-export glue; memory graph helpers
     are private duplicate logic.
   - Validation: full Rust validation; GraphQL memory/schema tests.

5. Daemon-core fabricated output cleanup
   - Scope IDs: `DC-003`, `DC-008`
   - Scope: `daemon/memory_tool.rs`, `daemon/agent_name_tool.rs`
   - Safe because: `context_packet_id` is fabricated and not backed by
     persisted packets; dead-code allow is stale.
   - Validation: full Rust validation; focused memory tool and daemon
     local-tool tests.

6. Provider-core unused exports and enum pieces
   - Scope IDs: `provider-core-001`, `provider-core-002`,
     `provider-core-003`, `provider-core-005`
   - Scope: `provider/accounts.rs`, `provider/model_catalog.rs`,
     `provider/contract.rs`, root re-exports
   - Safe because: items are definition/re-export/test-only; live GraphQL
     profile mapping ignores removed metadata extras.
   - Validation: full Rust validation; provider catalog/account/contract tests.

7. MCP unused schema/test helper
   - Scope IDs: `MCP-001`, `MCP-002`
   - Scope: `mcp.rs`, `mcp/client.rs`, root re-export
   - Safe because: GraphQL/web use other MCP record shapes; test helper has no
     callers.
   - Validation: full Rust validation; MCP client/setup tests.

8. Provider-adapter unused wrappers
   - Scope IDs: `provider-adapters-001`, `provider-adapters-002`,
     `provider-adapters-003`
   - Scope: `provider/adapters/sse.rs`, `responses.rs`,
     `foundation_bridge_process.rs`
   - Safe because: live streaming/session paths use other methods; these
     wrappers are uncalled.
   - Validation: full Rust validation; provider adapter streaming and
     Foundation Local tests.

9. CLI GraphQL facade (resolved by Task 1 `noema-cli` removal)
   - Scope ID: `cli-001`
   - Scope: `noema-cli/src/graphql_client.rs`, CLI imports
   - Safe because: facade only re-exports `crate::graphql`; callers are
     internal.
   - Validation: full Rust validation; CLI chat/memory/inspection unit tests.

10. Frontend unused helpers/barrel
    - Scope IDs: `frontend-001`, `frontend-002`, `frontend-003`
    - Scope: `web/src/App.tsx`, `web/src/routes.ts`,
      `web/src/components/transcript/index.ts`
    - Safe because: no live source imports; routing uses direct route
      branches/back navigation.
    - Validation: frontend typecheck/build if available; no browser inspection
      unless explicitly requested.

11. Core misc unused helpers
    - Scope IDs: `core-misc-005`, `core-misc-006`
    - Scope: `config.rs`, `runtime_host.rs`
    - Safe because: `Config::load_codex` and test runtime-host constructor have
      no callers.
    - Validation: full Rust validation; config/runtime-host tests.

12. Ineffective desktop serialization test
    - Scope ID: `desktop-cruft-003`
    - Scope: `noema-desktop/src/graphql_ipc.rs`
    - Safe because: existing test does not validate the frontend wire key it
      claims to protect.
    - Validation: replace with real JSON serialization test or delete; run
      desktop/unit Rust validation.

### Behavior-preserving refactor slices

- Store claims query/write cleanup: `store-claims-001`, `store-claims-002`
- Store MCP module split and prefixed-hex helper: `store-general-003`,
  `store-general-004`
- Daemon-web GraphQL glue relocation/test split: `DW-001`, `DW-002`, `DW-003`,
  `DW-005`
- GraphQL schema/MCP module split: `GQL-003`, `GQL-004`, `GQL-005`
- MCP transport/config parser consolidation: `MCP-003`, `MCP-004`, `MCP-005`,
  `MCP-006`
- Memory vocabulary serde/label consolidation: `memory-005`
- Provider adapter duplicated logging/test fixtures: `provider-adapters-006`,
  `provider-adapters-007`, `provider-adapters-008`
- Desktop OAuth callback shared handling: `desktop-cruft-001`
- CLI GraphQL DTO/selection consolidation: `cli-003` (resolved/superseded by
  Task 1 `noema-cli` removal)
- Cross-client GraphQL/codegen strategy: `ccd-003` (CLI portion
  resolved/superseded by Task 1; active-client strategy remains separate)
- Cross-module string vocabulary parsers: `ccd-004`
- Gateway discarded fields: `core-misc-004`

### Product/security/API decision points

- Old public memory API removal: `memory-001`, `memory-002`, `ccd-001`
- Capability policy/quarantine/owner extraction: `core-misc-001`,
  `core-misc-002`, `core-misc-003`
- Provider selection/privacy semantics: `DR-003`, `DR-004`
- Resolved raw daemon protocol/public API: `DC-004`, `DC-005`, `DC-006`,
  `DC-007` by Task 1 raw socket removal.
- Evidence authority variants: `store-claims-004`
- GraphQL memory/status fields: `GQL-006`, `GQL-008`
- Provider response compatibility parser: `provider-core-004`
- Foundation bridge lifecycle APIs: `provider-adapters-004`,
  `provider-adapters-005`
- Desktop OAuth listener startup timing: `desktop-cruft-002`
- CLI `noema context graph` placeholder: `cli-002` (resolved/superseded by Task
  1 `noema-cli` removal)
- Frontend Audit settings placeholder: `frontend-004`
- Embedded generated web assets packaging: `ccd-005`

### Keep / no action

- `memory-persistence`: already removed; no current files to cut.
- `store-claims-003`: keep; tests guard live related-claim persistence.
- `DR-005`: keep; Codex-named runtime types are live internals.
- `GQL-002`: keep coverage; move tests only if refactoring.
- `DC-001`, `DC-002`: keep; live memory-tool behavior despite design concerns.
- `MP-003`, `MP-004`, `MP-005`, `MP-006`: keep until replacement/refactor
  exists.
- `memory-003`, `memory-004`, `memory-006`, `memory-007`, `memory-008`: keep;
  live behavior or coverage.
- `provider-core-006`: keep; file is live despite size.
- `provider-adapters-009`, `provider-adapters-010`: keep; live modules,
  refactor only.
- `desktop-cruft-004`: keep; private error variants are small and live.
