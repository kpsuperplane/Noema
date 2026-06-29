# Agent Naming Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agents start unnamed, prompt construction asks unnamed agents to request a name through an `onboarding_prompt`, and the local `update_own_name` tool updates the current agent only when the user explicitly names or renames it.

**Architecture:** Add focused agent identity APIs in the embedded store, then thread `AgentRecord` into daemon prompt construction. Implement `update_own_name` as a Noema-local tool beside `search_memory`, with prompt-level instructions and runtime-side validation against trusted current-turn text.

**Tech Stack:** Rust, Tokio, Embedded SurrealDB v3, serde/serde_json, existing Noema daemon runtime and provider-neutral structured output items.

---

## File Structure

- Create `crates/noema-core/src/store/agents.rs`
  - Owns `AgentRecord`, `NewAgent`, `NoemaStore::create_agent`, `NoemaStore::get_agent`, and `NoemaStore::update_agent_display_name`.
- Modify `crates/noema-core/src/store.rs`
  - Registers the new store module and exports `AgentRecord`/`NewAgent`.
- Modify `crates/noema-core/src/lib.rs`
  - Re-exports `AgentRecord`/`NewAgent` with other store domain types.
- Modify `crates/noema-core/src/store/schema.rs`
  - Changes `agents.display_name` to `option<string>`.
- Modify `crates/noema-core/src/store/provider_accounts.rs`
  - Stops seeding `agent:primary` with `Noema`, while preserving existing names.
- Modify `crates/noema-core/src/store/error.rs`
  - Adds `AgentNotFound`.
- Modify `crates/noema-core/src/store/tests.rs`
  - Covers unnamed defaults, generic agent creation, and display-name update.
- Create `crates/noema-core/src/daemon/agent_onboarding.rs`
  - Owns `AgentPromptIdentity` and `agent_identity_prompt`.
- Create `crates/noema-core/src/daemon/agent_name_tool.rs`
  - Owns `update_own_name` parsing, explicit-user-instruction validation, and store update execution.
- Modify `crates/noema-core/src/daemon.rs`
  - Registers new daemon modules.
- Modify `crates/noema-core/src/daemon/runtime.rs`
  - Reads the current conversation agent identity, includes onboarding prompt text in initial and continuation prompts, advertises `update_own_name`, executes local naming tool calls, and persists local tool results through the existing action-output path.
- Modify `crates/noema-core/src/daemon/tests.rs`
  - Covers prompt identity/onboarding, successful name update, invalid/ambiguous update failures, and subsequent prompt identity.

Do not touch frontend files in this implementation plan. Existing dirty web files are unrelated and must stay untouched.

---

### Task 1: Store Agent Identity

**Files:**
- Create: `crates/noema-core/src/store/agents.rs`
- Modify: `crates/noema-core/src/store.rs`
- Modify: `crates/noema-core/src/lib.rs`
- Modify: `crates/noema-core/src/store/schema.rs`
- Modify: `crates/noema-core/src/store/provider_accounts.rs`
- Modify: `crates/noema-core/src/store/error.rs`
- Test: `crates/noema-core/src/store/tests.rs`

- [ ] **Step 1: Write failing store tests**

Add these tests near the existing provider/default-actor tests in `crates/noema-core/src/store/tests.rs`:

```rust
#[tokio::test]
async fn default_primary_agent_starts_unnamed() {
    let store = test_store().await;

    store.ensure_default_actors().await.expect("actors");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("primary agent exists");
    assert_eq!(agent.agent_id, "agent:primary");
    assert_eq!(agent.display_name, None);
}

#[tokio::test]
async fn ensure_default_actors_preserves_existing_agent_name() {
    let store = test_store().await;

    store.ensure_default_actors().await.expect("actors");
    store
        .update_agent_display_name("agent:primary", "Mira")
        .await
        .expect("update name");
    store.ensure_default_actors().await.expect("actors again");

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("primary agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Mira"));
}

#[tokio::test]
async fn agent_display_name_updates_trim_and_preserve_casing() {
    let store = test_store().await;

    let agent = store
        .create_agent(crate::NewAgent {
            agent_id: "agent:test-naming".to_string(),
            display_name: None,
        })
        .await
        .expect("create agent");
    assert_eq!(agent.display_name, None);

    let updated = store
        .update_agent_display_name("agent:test-naming", "  Mira Sol  ")
        .await
        .expect("update name");

    assert_eq!(updated.display_name.as_deref(), Some("Mira Sol"));
}
```

- [ ] **Step 2: Run store tests and verify they fail**

Run:

```bash
cargo test -p noema-core store::tests::default_primary_agent_starts_unnamed store::tests::ensure_default_actors_preserves_existing_agent_name store::tests::agent_display_name_updates_trim_and_preserve_casing -- --nocapture
```

Expected: compile failure for missing `NewAgent`, `get_agent`, and `update_agent_display_name`.

- [ ] **Step 3: Add agent store module**

Create `crates/noema-core/src/store/agents.rs`:

```rust
use serde::Deserialize;

use super::{
    NoemaStore, StoreError,
    ids::record_fragment,
};

/// Input for creating a durable agent row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAgent {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
}

/// Persisted agent identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    /// Durable concrete agent id.
    pub agent_id: String,
    /// Optional human-visible agent name.
    pub display_name: Option<String>,
}

impl NoemaStore {
    /// Create one durable agent row.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store write or read fails.
    pub async fn create_agent(&self, agent: NewAgent) -> Result<AgentRecord, StoreError> {
        self.db
            .query(
                r#"
                CREATE type::record('agents', $record_id) SET
                  agent_id = $agent_id,
                  display_name = $display_name,
                  updated_at = time::now();
                "#,
            )
            .bind(("record_id", record_fragment(&agent.agent_id)))
            .bind(("agent_id", agent.agent_id.clone()))
            .bind(("display_name", agent.display_name))
            .await?
            .check()?;
        self.get_agent(&agent.agent_id)
            .await?
            .ok_or_else(|| StoreError::AgentNotFound {
                agent_id: agent.agent_id,
            })
    }

    /// Return one agent by durable id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails.
    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<AgentRecord>, StoreError> {
        let mut response = self
            .db
            .query(
                r#"
                SELECT agent_id, display_name
                FROM agents
                WHERE agent_id = $agent_id
                LIMIT 1;
                "#,
            )
            .bind(("agent_id", agent_id.to_string()))
            .await?;
        let rows: Vec<AgentRow> = response.take(0)?;
        Ok(rows.into_iter().next().map(agent_from_row))
    }

    /// Update one agent's canonical display name.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the agent is missing or the embedded store
    /// write/read fails.
    pub async fn update_agent_display_name(
        &self,
        agent_id: &str,
        display_name: &str,
    ) -> Result<AgentRecord, StoreError> {
        self.require_agent(agent_id).await?;
        let trimmed = display_name.trim();
        self.db
            .query(
                r#"
                UPDATE agents SET
                  display_name = $display_name,
                  updated_at = time::now()
                WHERE agent_id = $agent_id;
                "#,
            )
            .bind(("agent_id", agent_id.to_string()))
            .bind(("display_name", trimmed.to_string()))
            .await?
            .check()?;
        self.get_agent(agent_id)
            .await?
            .ok_or_else(|| StoreError::AgentNotFound {
                agent_id: agent_id.to_string(),
            })
    }

    pub(crate) async fn require_agent(&self, agent_id: &str) -> Result<(), StoreError> {
        if self.get_agent(agent_id).await?.is_some() {
            Ok(())
        } else {
            Err(StoreError::AgentNotFound {
                agent_id: agent_id.to_string(),
            })
        }
    }
}

#[derive(Debug, Deserialize)]
struct AgentRow {
    agent_id: String,
    display_name: Option<String>,
}

fn agent_from_row(row: AgentRow) -> AgentRecord {
    AgentRecord {
        agent_id: row.agent_id,
        display_name: row.display_name,
    }
}
```

- [ ] **Step 4: Wire store module and exports**

Modify `crates/noema-core/src/store.rs`:

```rust
mod agents;
mod claims;
```

Add to the public exports:

```rust
pub use agents::{AgentRecord, NewAgent};
```

Modify `crates/noema-core/src/lib.rs` store re-export block so it includes the new types:

```rust
pub use store::{
    AgentRecord, ClaimRetrievalResult, ClaimStatus, ClaimSummary, ClaimWriteOutcome,
    ConsolidationMatch, ConsolidationMatchRequest, EntityCandidate, EntityType,
    EvidenceAuthority, EvidenceCandidate, MemoryClaimDetail, MemoryClaimEvidence,
    MemoryClaimFilter, MemoryClaimRecord, MemoryGraph, MemoryGraphEdge, MemoryGraphFilter,
    MemoryGraphNode, MemoryGraphSummary, NewAgent, NewClaimCandidate, NoemaStore,
    PredicateProposalCandidate, PredicateProposalFilter, PredicateProposalRecord, PredicateRecord,
    RelatedClaimCandidate, RelatedClaimRecord, RetrievedClaim, StoreConfig, StoreError,
};
```

- [ ] **Step 5: Make agent display names optional and preserve existing names**

In `crates/noema-core/src/store/schema.rs`, change the agent display-name field:

```rust
DEFINE FIELD IF NOT EXISTS display_name ON TABLE agents TYPE option<string>;
```

In `crates/noema-core/src/store/provider_accounts.rs`, remove the default `Noema` assignment from the agent UPSERT:

```rust
UPSERT type::record('agents', 'agent_primary') SET
  agent_id = 'agent:primary',
  updated_at = time::now();
```

This preserves an existing `display_name` because the UPSERT no longer writes that field.

In `crates/noema-core/src/store/error.rs`, add:

```rust
/// An agent expected to exist was not found.
#[error("agent not found: {agent_id}")]
AgentNotFound {
    /// Missing agent id.
    agent_id: String,
},
```

- [ ] **Step 6: Run store tests and verify they pass**

Run:

```bash
cargo test -p noema-core store::tests::default_primary_agent_starts_unnamed store::tests::ensure_default_actors_preserves_existing_agent_name store::tests::agent_display_name_updates_trim_and_preserve_casing -- --nocapture
```

Expected: all three tests pass.

- [ ] **Step 7: Commit store identity slice**

Run:

```bash
git status --short --branch
git add crates/noema-core/src/store/agents.rs crates/noema-core/src/store.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/provider_accounts.rs crates/noema-core/src/store/error.rs crates/noema-core/src/store/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: add agent identity store"
```

Expected staged files: only the files listed in this task.

---

### Task 2: Agent Onboarding Prompt Builder

**Files:**
- Create: `crates/noema-core/src/daemon/agent_onboarding.rs`
- Modify: `crates/noema-core/src/daemon.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Test: `crates/noema-core/src/daemon/agent_onboarding.rs`
- Test: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write failing prompt-builder unit tests**

Create `crates/noema-core/src/daemon/agent_onboarding.rs` with tests first:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentPromptIdentity {
    pub agent_id: String,
    pub display_name: Option<String>,
}

pub(super) fn agent_identity_prompt(_identity: &AgentPromptIdentity) -> String {
    unimplemented!("agent identity prompt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unnamed_agent_prompt_includes_onboarding_prompt() {
        let prompt = agent_identity_prompt(&AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: None,
        });

        assert!(prompt.contains("Agent identity:"));
        assert!(prompt.contains("agent_id: agent:primary"));
        assert!(prompt.contains("display_name: none"));
        assert!(prompt.contains("Onboarding prompt:"));
        assert!(prompt.contains("You do not have a name yet."));
        assert!(prompt.contains("Your first priority is to ask the user to give you one."));
        assert!(prompt.contains("call update_own_name"));
    }

    #[test]
    fn named_agent_prompt_omits_missing_name_onboarding() {
        let prompt = agent_identity_prompt(&AgentPromptIdentity {
            agent_id: "agent:primary".to_string(),
            display_name: Some("Mira".to_string()),
        });

        assert!(prompt.contains("Agent identity:"));
        assert!(prompt.contains("agent_id: agent:primary"));
        assert!(prompt.contains("display_name: Mira"));
        assert!(!prompt.contains("Onboarding prompt:"));
        assert!(!prompt.contains("You do not have a name yet."));
    }
}
```

- [ ] **Step 2: Run prompt-builder tests and verify they fail**

Run:

```bash
cargo test -p noema-core daemon::agent_onboarding::tests -- --nocapture
```

Expected: tests panic at `unimplemented!("agent identity prompt")`.

- [ ] **Step 3: Implement prompt builder**

Replace the `agent_identity_prompt` body in `crates/noema-core/src/daemon/agent_onboarding.rs`:

```rust
pub(super) fn agent_identity_prompt(identity: &AgentPromptIdentity) -> String {
    let mut prompt = String::new();
    prompt.push_str("Agent identity:\n");
    prompt.push_str(&format!("- agent_id: {}\n", identity.agent_id));
    match identity.display_name.as_deref().map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            prompt.push_str(&format!("- display_name: {name}\n"));
        }
        None => {
            prompt.push_str("- display_name: none\n\n");
            prompt.push_str("Onboarding prompt:\n");
            prompt.push_str("- You do not have a name yet.\n");
            prompt.push_str("- Your first priority is to ask the user to give you one.\n");
            prompt.push_str("- Do not invent, assume, or sign off with a name.\n");
            prompt.push_str("- If the user gives you a name, call update_own_name with that name.\n");
        }
    }
    prompt
}
```

Register the module in `crates/noema-core/src/daemon.rs`:

```rust
mod agent_onboarding;
mod client;
```

- [ ] **Step 4: Run prompt-builder tests and verify they pass**

Run:

```bash
cargo test -p noema-core daemon::agent_onboarding::tests -- --nocapture
```

Expected: both tests pass.

- [ ] **Step 5: Add failing runtime prompt test**

In `crates/noema-core/src/daemon/tests.rs`, add a new fake scenario:

```rust
IdentityPromptCheck,
```

In `FakeCodexProvider::generate_response`, add a match arm:

```rust
FakeCodexScenario::IdentityPromptCheck => {
    let saw_identity = instructions.contains("Agent identity:")
        && instructions.contains("agent_id: agent:primary")
        && instructions.contains("display_name: none")
        && instructions.contains("Onboarding prompt:")
        && instructions.contains("update_own_name");
    assistant_with_no_memories(if saw_identity {
        "saw unnamed identity"
    } else {
        "missing unnamed identity"
    })
}
```

Add a constructor near other fake provider helpers:

```rust
fn fake_codex_provider_with_identity_prompt_check() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::IdentityPromptCheck)
}
```

Add the test:

```rust
#[tokio::test]
async fn runtime_prompt_includes_unnamed_agent_onboarding() {
    let handle = test_runtime_handle(fake_codex_provider_with_identity_prompt_check()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id, "hello".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw unnamed identity"
    )));
}
```

- [ ] **Step 6: Run runtime prompt test and verify it fails**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_prompt_includes_unnamed_agent_onboarding -- --nocapture
```

Expected: test fails with assistant text `missing unnamed identity`.

- [ ] **Step 7: Thread agent identity into prompt construction**

Modify the existing grouped `use super` block in
`crates/noema-core/src/daemon/runtime.rs` by adding `agent_onboarding` before
the existing `memory_pipeline` import:

```rust
agent_onboarding::{AgentPromptIdentity, agent_identity_prompt},
```

After loading recent transcript in `CodexRuntimeActor::turn`, read the primary agent:

```rust
let agent_identity = self
    .agent_identity_for_conversation(&conversation_id)
    .await?;
```

Add this helper method inside `impl CodexRuntimeActor`:

```rust
async fn agent_identity_for_conversation(
    &self,
    _conversation_id: &str,
) -> Result<AgentPromptIdentity, DaemonError> {
    let agent_id = "agent:primary".to_string();
    let agent = self
        .store
        .get_agent(&agent_id)
        .await?
        .ok_or_else(|| DaemonError::Protocol(format!("unknown agent id: {agent_id}")))?;
    Ok(AgentPromptIdentity {
        agent_id: agent.agent_id,
        display_name: agent.display_name,
    })
}
```

Change the initial prompt call:

```rust
let structured_instructions = build_structured_turn_system_prompt(
    &conversation_id,
    turn_index,
    conversation.cwd.as_deref(),
    &recent_transcript,
    &agent_identity,
);
```

Change the function signature:

```rust
fn build_structured_turn_system_prompt(
    conversation_id: &str,
    turn_index: u64,
    cwd: Option<&str>,
    recent_transcript: &str,
    agent_identity: &AgentPromptIdentity,
) -> String {
```

Inside the function before `format!`, compute:

```rust
let agent_identity_prompt = agent_identity_prompt(agent_identity);
```

Add this block after `{AGENT_PERSONALITY_PROMPT}` in the formatted prompt:

```text

{agent_identity_prompt}
```

Update `build_local_tool_result_continuation_system_prompt` to accept `agent_identity: &AgentPromptIdentity` and pass it through to `build_structured_turn_system_prompt`.

- [ ] **Step 8: Run runtime prompt test and verify it passes**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_prompt_includes_unnamed_agent_onboarding -- --nocapture
```

Expected: test passes.

- [ ] **Step 9: Commit onboarding prompt slice**

Run:

```bash
git status --short --branch
git add crates/noema-core/src/daemon/agent_onboarding.rs crates/noema-core/src/daemon.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: add agent onboarding prompt"
```

Expected staged files: only the files listed in this task.

---

### Task 3: Local update_own_name Tool

**Files:**
- Create: `crates/noema-core/src/daemon/agent_name_tool.rs`
- Modify: `crates/noema-core/src/daemon.rs`
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Test: `crates/noema-core/src/daemon/agent_name_tool.rs`

- [ ] **Step 1: Write failing tool unit tests**

Create `crates/noema-core/src/daemon/agent_name_tool.rs` with tests first:

```rust
use crate::{NoemaStore, store::StoreError};
use serde::Deserialize;
use serde_json::{Value, json};

const UPDATE_OWN_NAME_TOOL: &str = "update_own_name";
const MAX_AGENT_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentNameToolRuntimeContext {
    pub agent_id: String,
    pub user_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentNameToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum AgentNameToolError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateOwnNameArguments {
    name: String,
}

pub(super) fn is_update_own_name_tool(name: &str) -> bool {
    name == UPDATE_OWN_NAME_TOOL
}

pub(super) async fn execute_update_own_name(
    _store: &NoemaStore,
    _context: &AgentNameToolRuntimeContext,
    call_id: Option<String>,
    _payload: &Value,
) -> AgentNameToolResult {
    AgentNameToolResult {
        call_id,
        name: UPDATE_OWN_NAME_TOOL.to_string(),
        success: false,
        payload: json!({"error": "not implemented"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_name_arguments_and_trims() {
        let payload = json!({"arguments": {"name": "  Mira  "}});

        let arguments = parse_arguments(&payload).expect("parse arguments");

        assert_eq!(arguments.name, "Mira");
    }

    #[test]
    fn rejects_empty_name() {
        let payload = json!({"name": "   "});

        let error = parse_arguments(&payload).expect_err("empty name rejected");

        assert_eq!(safe_error_message(&error), "name is required");
    }

    #[test]
    fn rejects_overlong_name() {
        let payload = json!({"name": "a".repeat(MAX_AGENT_NAME_CHARS + 1)});

        let error = parse_arguments(&payload).expect_err("long name rejected");

        assert_eq!(safe_error_message(&error), "name must be 80 characters or fewer");
    }

    #[test]
    fn accepts_explicit_user_naming_instruction() {
        assert!(user_explicitly_names_agent("Your name is Mira.", "Mira"));
        assert!(user_explicitly_names_agent("Call yourself Orin.", "Orin"));
        assert!(user_explicitly_names_agent("Rename yourself to Halcyon.", "Halcyon"));
        assert!(user_explicitly_names_agent("I want to call you Tess.", "Tess"));
    }

    #[test]
    fn rejects_ambiguous_or_unrelated_mentions() {
        assert!(!user_explicitly_names_agent("What name do you like?", "Mira"));
        assert!(!user_explicitly_names_agent("Maybe you could be Mira?", "Mira"));
        assert!(!user_explicitly_names_agent("Mira is a nice name.", "Mira"));
    }
}
```

- [ ] **Step 2: Run tool unit tests and verify they fail**

Run:

```bash
cargo test -p noema-core daemon::agent_name_tool::tests -- --nocapture
```

Expected: compile failure for missing `parse_arguments`, `safe_error_message`, and `user_explicitly_names_agent`.

- [ ] **Step 3: Implement argument parsing and explicit-instruction policy**

Add these functions to `crates/noema-core/src/daemon/agent_name_tool.rs`:

```rust
fn parse_arguments(payload: &Value) -> Result<UpdateOwnNameArguments, AgentNameToolError> {
    let argument_value = payload.get("arguments").unwrap_or(payload).clone();
    let mut arguments: UpdateOwnNameArguments =
        serde_json::from_value(argument_value).map_err(|error| {
            AgentNameToolError::InvalidArguments(format!("invalid arguments: {error}"))
        })?;
    arguments.name = arguments.name.trim().to_string();
    if arguments.name.is_empty() {
        return Err(AgentNameToolError::InvalidArguments(
            "name is required".to_string(),
        ));
    }
    if arguments.name.chars().count() > MAX_AGENT_NAME_CHARS {
        return Err(AgentNameToolError::InvalidArguments(
            "name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(arguments)
}

fn user_explicitly_names_agent(user_input: &str, name: &str) -> bool {
    let input = normalize_for_name_match(user_input);
    let name = normalize_for_name_match(name);
    let patterns = [
        format!("your name is {name}"),
        format!("call yourself {name}"),
        format!("rename yourself to {name}"),
        format!("i want to call you {name}"),
        format!("i'll call you {name}"),
        format!("ill call you {name}"),
    ];
    patterns.iter().any(|pattern| input.contains(pattern))
}

fn normalize_for_name_match(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn safe_error_message(error: &AgentNameToolError) -> String {
    match error {
        AgentNameToolError::InvalidArguments(message) => message.clone(),
        AgentNameToolError::Store(_) => "name update failed".to_string(),
    }
}
```

- [ ] **Step 4: Implement store-backed tool execution**

Replace `execute_update_own_name` with:

```rust
pub(super) async fn execute_update_own_name(
    store: &NoemaStore,
    context: &AgentNameToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> AgentNameToolResult {
    match execute_update_own_name_inner(store, context, payload).await {
        Ok(payload) => AgentNameToolResult {
            call_id,
            name: UPDATE_OWN_NAME_TOOL.to_string(),
            success: true,
            payload,
        },
        Err(error) => AgentNameToolResult {
            call_id,
            name: UPDATE_OWN_NAME_TOOL.to_string(),
            success: false,
            payload: json!({
                "error": safe_error_message(&error),
            }),
        },
    }
}

async fn execute_update_own_name_inner(
    store: &NoemaStore,
    context: &AgentNameToolRuntimeContext,
    payload: &Value,
) -> Result<Value, AgentNameToolError> {
    let arguments = parse_arguments(payload)?;
    if !user_explicitly_names_agent(&context.user_input, &arguments.name) {
        return Err(AgentNameToolError::InvalidArguments(
            "name update requires explicit user instruction".to_string(),
        ));
    }
    let agent = store
        .update_agent_display_name(&context.agent_id, &arguments.name)
        .await?;
    Ok(json!({
        "agent_id": agent.agent_id,
        "display_name": agent.display_name,
    }))
}
```

Register the module in `crates/noema-core/src/daemon.rs`:

```rust
mod agent_name_tool;
mod agent_onboarding;
```

- [ ] **Step 5: Run tool unit tests and verify they pass**

Run:

```bash
cargo test -p noema-core daemon::agent_name_tool::tests -- --nocapture
```

Expected: all tool unit tests pass.

- [ ] **Step 6: Commit local name tool slice**

Run:

```bash
git status --short --branch
git add crates/noema-core/src/daemon/agent_name_tool.rs crates/noema-core/src/daemon.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: add agent name local tool"
```

Expected staged files: only the files listed in this task.

---

### Task 4: Runtime Name Tool Integration

**Files:**
- Modify: `crates/noema-core/src/daemon/runtime.rs`
- Modify: `crates/noema-core/src/daemon/tests.rs`

- [ ] **Step 1: Write failing successful-rename runtime test**

In `crates/noema-core/src/daemon/tests.rs`, add a fake scenario:

```rust
NameToolContinuation,
```

Add this match arm in `FakeCodexProvider::generate_response`:

```rust
FakeCodexScenario::NameToolContinuation => {
    if input.contains("Your name is Mira.") {
        vec![
            GenerateOutputItem::ToolCall {
                id: Some("call_name_1".to_string()),
                name: "update_own_name".to_string(),
                payload: json!({"arguments": {"name": "Mira"}}),
            },
            GenerateOutputItem::AssistantText {
                text: "I'll update that.".to_string(),
            },
            GenerateOutputItem::MemoryProposals { proposals: vec![] },
        ]
    } else if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
        assistant_with_no_memories("Mira it is.")
    } else {
        assistant_with_no_memories("fake answer")
    }
}
```

Add constructor:

```rust
fn fake_codex_provider_with_name_tool_continuation() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::NameToolContinuation)
}
```

Add the test:

```rust
#[tokio::test]
async fn update_own_name_tool_updates_agent_and_continues_turn() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_name_tool_continuation()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(&handle, conversation_id.clone(), "Your name is Mira.".to_string())
        .await
        .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name.as_deref(), Some("Mira"));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: update_own_name"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Completed,
            title,
            metadata,
            ..
        } if activity_kind == "tool_result"
            && title == "Tool result: update_own_name"
            && metadata["action"]["success"] == true
            && metadata["action"]["payload"]["display_name"] == "Mira"
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "Mira it is."
    )));
}
```

- [ ] **Step 2: Run successful-rename runtime test and verify it fails**

Run:

```bash
cargo test -p noema-core daemon::tests::update_own_name_tool_updates_agent_and_continues_turn -- --nocapture
```

Expected: test fails because `update_own_name` is persisted as a provider tool call but not executed as a local tool result.

- [ ] **Step 3: Generalize local tool result type in runtime**

In `crates/noema-core/src/daemon/runtime.rs`, add `agent_name_tool` to the same
existing grouped `use super` block:

```rust
agent_name_tool::{
    AgentNameToolRuntimeContext, execute_update_own_name, is_update_own_name_tool,
},
```

Create a local enum near `ProviderAssistantResponse` helper structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalToolResult {
    Memory(MemoryToolResult),
    AgentName(crate::daemon::agent_name_tool::AgentNameToolResult),
}

impl LocalToolResult {
    fn call_id(&self) -> Option<String> {
        match self {
            Self::Memory(result) => result.call_id.clone(),
            Self::AgentName(result) => result.call_id.clone(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Memory(result) => &result.name,
            Self::AgentName(result) => &result.name,
        }
    }

    fn success(&self) -> bool {
        match self {
            Self::Memory(result) => result.success,
            Self::AgentName(result) => result.success,
        }
    }

    fn payload(&self) -> &Value {
        match self {
            Self::Memory(result) => &result.payload,
            Self::AgentName(result) => &result.payload,
        }
    }
}
```

Replace `execute_local_search_memory_tools` with `execute_local_tools`:

```rust
async fn execute_local_tools(
    &self,
    turn: &SuccessfulProviderTurn,
    agent_identity: &AgentPromptIdentity,
) -> Vec<LocalToolResult> {
    let mut results = Vec::new();
    for (index, output) in turn.response.output.iter().enumerate() {
        let GenerateOutputItem::ToolCall { id, name, payload } = output else {
            continue;
        };
        if is_search_memory_tool(name) {
            let context = MemoryToolRuntimeContext {
                conversation_id: turn.conversation_id.clone(),
                turn_id: turn.turn_id.clone(),
                turn_index: turn.turn_index,
                call_site_id: format!("output_{index}"),
                cwd: turn.cwd.clone(),
                user_input: turn.user_input.clone(),
            };
            results.push(LocalToolResult::Memory(
                execute_search_memory(&self.store, &context, id.clone(), payload).await,
            ));
            continue;
        }
        if is_update_own_name_tool(name) {
            let context = AgentNameToolRuntimeContext {
                agent_id: agent_identity.agent_id.clone(),
                user_input: turn.user_input.clone(),
            };
            results.push(LocalToolResult::AgentName(
                execute_update_own_name(&self.store, &context, id.clone(), payload).await,
            ));
        }
    }
    results
}
```

Update `persist_successful_provider_turn` to receive `agent_identity: AgentPromptIdentity`, call `execute_local_tools(&turn, &agent_identity)`, and use `local_tool_results` for both memory and name results.

- [ ] **Step 4: Generalize local tool result continuation helpers**

Replace `local_tool_result_continuation_input`, `local_tool_result_payload`, and `local_tool_result_output_item` with `LocalToolResult` versions:

```rust
fn local_tool_result_continuation_input(results: &[LocalToolResult]) -> Value {
    json!({
        "type": "NOEMA_LOCAL_TOOL_RESULT",
        "results": results.iter().map(local_tool_result_payload).collect::<Vec<_>>(),
    })
}

fn local_tool_result_payload(result: &LocalToolResult) -> Value {
    json!({
        "call_id": result.call_id(),
        "name": result.name(),
        "success": result.success(),
        "payload": result.payload(),
    })
}

fn local_tool_result_output_item(result: &LocalToolResult) -> GenerateOutputItem {
    GenerateOutputItem::ToolResult {
        call_id: result.call_id(),
        name: Some(result.name().to_string()),
        success: Some(result.success()),
        payload: result.payload().clone(),
    }
}
```

Pass `&agent_identity` into `build_local_tool_result_continuation_system_prompt`.

- [ ] **Step 5: Advertise update_own_name in structured prompt**

In `build_structured_turn_system_prompt`, after the `search_memory` tool block, add:

```text

You may emit an update_own_name tool call only when the current user explicitly names or renames you.
Use this output item shape:
{{"kind":"tool_call","id":"call_name_1","name":"update_own_name","payload":{{"name":"Mira"}}}}
Never call update_own_name because you prefer a name or because the user's wording is ambiguous.
If the user suggests a possible name without explicitly naming you, ask for confirmation instead.
```

- [ ] **Step 6: Run successful-rename runtime test and verify it passes**

Run:

```bash
cargo test -p noema-core daemon::tests::update_own_name_tool_updates_agent_and_continues_turn -- --nocapture
```

Expected: test passes.

- [ ] **Step 7: Write failing ambiguous-rename runtime test**

Add this fake scenario:

```rust
AmbiguousNameTool,
```

Add match arm:

```rust
FakeCodexScenario::AmbiguousNameTool => vec![
    GenerateOutputItem::ToolCall {
        id: Some("call_name_ambiguous".to_string()),
        name: "update_own_name".to_string(),
        payload: json!({"arguments": {"name": "Mira"}}),
    },
    GenerateOutputItem::AssistantText {
        text: "I'll try to update that.".to_string(),
    },
    GenerateOutputItem::MemoryProposals { proposals: vec![] },
],
```

Add constructor:

```rust
fn fake_codex_provider_with_ambiguous_name_tool() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::AmbiguousNameTool)
}
```

Add test:

```rust
#[tokio::test]
async fn update_own_name_tool_rejects_ambiguous_user_instruction() {
    let (handle, store) =
        test_runtime_handle_with_store(fake_codex_provider_with_ambiguous_name_tool()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    let items = collect_turn(
        &handle,
        conversation_id,
        "Maybe you could be Mira?".to_string(),
    )
    .await
    .expect("turn");
    handle.shutdown().await;

    let agent = store
        .get_agent("agent:primary")
        .await
        .expect("get agent")
        .expect("agent exists");
    assert_eq!(agent.display_name, None);
    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::Activity {
            activity_kind,
            status: TurnActivityStatus::Failed,
            title,
            metadata,
            ..
        } if activity_kind == "tool_result"
            && title == "Tool result: update_own_name"
            && metadata["action"]["success"] == false
            && metadata["action"]["payload"]["error"]
                == "name update requires explicit user instruction"
    )));
}
```

- [ ] **Step 8: Run ambiguous-rename runtime test and verify it passes**

Run:

```bash
cargo test -p noema-core daemon::tests::update_own_name_tool_rejects_ambiguous_user_instruction -- --nocapture
```

Expected: test passes.

- [ ] **Step 9: Write subsequent-prompt identity test**

Add a fake scenario:

```rust
NamedIdentityPromptCheck,
```

Add match arm:

```rust
FakeCodexScenario::NamedIdentityPromptCheck => {
    if input.contains("Your name is Mira.") {
        vec![
            GenerateOutputItem::ToolCall {
                id: Some("call_name_1".to_string()),
                name: "update_own_name".to_string(),
                payload: json!({"arguments": {"name": "Mira"}}),
            },
            GenerateOutputItem::AssistantText {
                text: "I'll update that.".to_string(),
            },
            GenerateOutputItem::MemoryProposals { proposals: vec![] },
        ]
    } else if input.contains("NOEMA_LOCAL_TOOL_RESULT") {
        assistant_with_no_memories("Mira it is.")
    } else {
        let saw_named_identity = instructions.contains("Agent identity:")
            && instructions.contains("display_name: Mira")
            && !instructions.contains("You do not have a name yet.");
        assistant_with_no_memories(if saw_named_identity {
            "saw named identity"
        } else {
            "missing named identity"
        })
    }
}
```

Add constructor:

```rust
fn fake_codex_provider_with_named_identity_prompt_check() -> FakeCodexProvider {
    FakeCodexProvider::new(FakeCodexScenario::NamedIdentityPromptCheck)
}
```

Add test:

```rust
#[tokio::test]
async fn runtime_prompt_includes_stored_agent_name_after_update() {
    let handle = test_runtime_handle(fake_codex_provider_with_named_identity_prompt_check()).await;

    let conversation_id = handle
        .start_conversation(None, None)
        .await
        .expect("conversation")
        .conversation_id;
    collect_turn(&handle, conversation_id.clone(), "Your name is Mira.".to_string())
        .await
        .expect("name turn");
    let items = collect_turn(&handle, conversation_id, "hello again".to_string())
        .await
        .expect("second turn");
    handle.shutdown().await;

    assert!(items.iter().any(|item| matches!(
        item,
        TurnTranscriptItem::AssistantText { text } if text == "saw named identity"
    )));
}
```

- [ ] **Step 10: Run subsequent-prompt identity test and verify it passes**

Run:

```bash
cargo test -p noema-core daemon::tests::runtime_prompt_includes_stored_agent_name_after_update -- --nocapture
```

Expected: test passes.

- [ ] **Step 11: Commit runtime integration slice**

Run:

```bash
git status --short --branch
git add crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: wire agent name tool into runtime"
```

Expected staged files: only the files listed in this task.

---

### Task 5: Focused Regression And Final Validation

**Files:**
- Modify only if previous validation exposes an issue in files already changed by Tasks 1-4.

- [ ] **Step 1: Run focused store, tool, and runtime tests**

Run:

```bash
cargo test -p noema-core store::tests::default_primary_agent_starts_unnamed store::tests::ensure_default_actors_preserves_existing_agent_name store::tests::agent_display_name_updates_trim_and_preserve_casing daemon::agent_onboarding::tests daemon::agent_name_tool::tests daemon::tests::runtime_prompt_includes_unnamed_agent_onboarding daemon::tests::update_own_name_tool_updates_agent_and_continues_turn daemon::tests::update_own_name_tool_rejects_ambiguous_user_instruction daemon::tests::runtime_prompt_includes_stored_agent_name_after_update -- --nocapture
```

Expected: all listed tests pass.

- [ ] **Step 2: Run format check**

Run:

```bash
cargo fmt --all --check
```

Expected: pass. If it fails, run `cargo fmt --all`, inspect the formatting diff, and commit formatting with the affected slice if not already committed.

- [ ] **Step 3: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: pass.

- [ ] **Step 4: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: pass.

- [ ] **Step 5: Run unit tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: pass. Do not run smoke tests or fixture tests unless explicitly requested.

- [ ] **Step 6: Check final worktree and staged state**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: no whitespace errors. Any unrelated dirty files from before the work should remain unstaged and be reported.

- [ ] **Step 7: Commit validation fixes if any were needed**

If Steps 1-5 required code fixes, stage only the changed implementation/test files:

```bash
git add crates/noema-core/src/store/agents.rs crates/noema-core/src/store.rs crates/noema-core/src/lib.rs crates/noema-core/src/store/schema.rs crates/noema-core/src/store/provider_accounts.rs crates/noema-core/src/store/error.rs crates/noema-core/src/store/tests.rs crates/noema-core/src/daemon/agent_onboarding.rs crates/noema-core/src/daemon/agent_name_tool.rs crates/noema-core/src/daemon.rs crates/noema-core/src/daemon/runtime.rs crates/noema-core/src/daemon/tests.rs
git diff --cached --stat
git diff --cached --name-status
git commit -m "fix: complete agent naming onboarding validation"
```

Expected: this step is skipped if there were no validation fixes.

---

## Self-Review

- Spec coverage: Tasks 1-4 cover unnamed canonical agent state, no seeded `Noema` name, prompt-level `onboarding_prompt`, `update_own_name`, explicit user authorization, persisted tool call/result rows, and subsequent prompts with the stored name.
- Red-flag scan: The plan has no undecided markers or unspecified implementation steps. Every code-changing step names exact files and provides concrete code or exact text to insert.
- Type consistency: Store types are `NewAgent` and `AgentRecord`; daemon prompt identity is `AgentPromptIdentity`; local tool context/result types are `AgentNameToolRuntimeContext` and `AgentNameToolResult`; runtime wraps local tool results in `LocalToolResult`.
- Scope check: The plan does not build a profile editor, frontend agent settings, migrations, or future onboarding questions. It adds only the naming prompt apparatus and local rename tool.
