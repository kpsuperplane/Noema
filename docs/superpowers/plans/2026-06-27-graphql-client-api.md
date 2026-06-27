# GraphQL Client API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make GraphQL the only first-party client-facing API for Noema's existing onboarding and chat slice, with web and CLI clients consuming generated/typed GraphQL operations.

**Architecture:** Add GraphQL beside the current web protocol, move existing product reads/actions/streams onto GraphQL, then retire direct product HTTP and WebSocket endpoints after parity. GraphQL resolvers stay thin: queries call read models, mutations call runtime/command paths, and subscriptions adapt existing turn stream events while leaving room for durable event cursors.

**Tech Stack:** Rust 2024, Tokio, async-graphql, manual local HTTP/WebSocket server, React 19, Apollo Client, graphql-ws, Vite, Bun, GraphQL Code Generator, Postgres-backed Noema repositories.

---

## Scope

This plan implements the first GraphQL slice only:

- Local status.
- Onboarding status.
- Provider auth start/check/cancel.
- Primary conversation start/replay.
- Chat turn send.
- Transcript item and agent status stream.
- Web UI migration to GraphQL.
- CLI chat migration to GraphQL product operations.
- Transitional native product endpoint retirement.

It does not implement future workspace/task/memory GraphQL surfaces, true offline-first sync, or durable cross-client event cursors. It creates the seams those later plans will use.

## File Structure

- Create `crates/noema-core/src/graphql.rs`: public GraphQL module root and schema builder.
- Create `crates/noema-core/src/graphql/schema.rs`: query, mutation, subscription root definitions.
- Create `crates/noema-core/src/graphql/types.rs`: GraphQL object/input/enum types mapped from current Rust product structs.
- Create `crates/noema-core/src/graphql/resolvers.rs`: resolver helpers that call `WebState`, runtime, repositories, and provider auth.
- Create `crates/noema-core/src/graphql/subscriptions.rs`: subscription connection registry and turn-event bridge.
- Modify `crates/noema-core/src/daemon/web/mod.rs`: route `/graphql`, `/graphql/ws`, and keep static assets/callback-like endpoints.
- Modify `crates/noema-core/src/daemon/server.rs`: construct GraphQL schema/subscription state alongside web state.
- Modify `crates/noema-core/src/lib.rs`: export the GraphQL module.
- Modify `Cargo.toml` and `crates/noema-core/Cargo.toml`: add GraphQL and stream dependencies.
- Create `crates/noema-core/web/codegen.ts`: GraphQL codegen config for Apollo React hooks.
- Create `crates/noema-core/web/src/graphql/operations.ts`: GraphQL operation documents.
- Create `crates/noema-core/web/src/graphql/client.ts`: Apollo Client setup, normalized cache policy, and subscription link.
- Modify `crates/noema-core/web/package.json`: replace `gen:types` with GraphQL codegen and add dependencies.
- Modify `crates/noema-core/web/src/api.ts`, `App.tsx`, and `transcript.ts`: consume GraphQL operations instead of generated daemon protocol types.
- Create `crates/noema-cli/src/graphql_client.rs`: typed CLI GraphQL client helpers.
- Modify `crates/noema-cli/src/main.rs`: use GraphQL for chat product operations while keeping local daemon lifecycle commands direct.
- Update `docs/frontend/current-contract.md`, `docs/frontend/README.md`, and `docs/context/current.md`: document GraphQL as the first-party client API.

## Task 1: Add GraphQL Dependencies And Schema Skeleton

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/noema-core/Cargo.toml`
- Modify: `crates/noema-core/src/lib.rs`
- Create: `crates/noema-core/src/graphql.rs`
- Create: `crates/noema-core/src/graphql/schema.rs`
- Create: `crates/noema-core/src/graphql/types.rs`
- Test: `crates/noema-core/src/graphql/schema.rs`

- [ ] **Step 1: Add the failing schema export test**

Add this test to the bottom of `crates/noema-core/src/graphql/schema.rs` when the file is created:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_sdl_exposes_initial_noema_fields() {
        let schema = build_schema(GraphqlState::for_tests());
        let sdl = schema.sdl();

        assert!(sdl.contains("type Query"));
        assert!(sdl.contains("localStatus"));
        assert!(sdl.contains("onboardingStatus"));
        assert!(sdl.contains("type Mutation"));
        assert!(sdl.contains("startProviderAuthAttempt"));
        assert!(sdl.contains("startPrimaryConversation"));
        assert!(sdl.contains("sendConversationTurn"));
        assert!(sdl.contains("type Subscription"));
        assert!(sdl.contains("conversationEvents"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::schema_sdl_exposes_initial_noema_fields
```

Expected: FAIL because `noema_core::graphql` and `build_schema` do not exist.

- [ ] **Step 3: Add workspace dependencies**

In the root `Cargo.toml`, add:

```toml
async-graphql = "7"
async-stream = "0.3"
futures-util = "0.3"
```

In `crates/noema-core/Cargo.toml`, add:

```toml
async-graphql.workspace = true
async-stream.workspace = true
futures-util.workspace = true
```

- [ ] **Step 4: Export the GraphQL module**

In `crates/noema-core/src/lib.rs`, add:

```rust
/// GraphQL client API facade.
pub mod graphql;
```

- [ ] **Step 5: Create the module root**

Create `crates/noema-core/src/graphql.rs`:

```rust
//! GraphQL client API facade.
//!
//! The GraphQL layer is the first-party client API. Resolvers must stay thin:
//! they call Noema read models, command/runtime paths, provider auth, and
//! repository methods instead of owning product behavior.

mod schema;
mod types;

pub use schema::{GraphqlSchema, GraphqlState, build_schema};
```

- [ ] **Step 6: Create initial GraphQL status types**

Create `crates/noema-core/src/graphql/types.rs`:

```rust
use async_graphql::{Enum, SimpleObject};

/// Local service status shown by clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlLocalServiceStatus {
    /// The local Noema service is running.
    Running,
}

/// Assistant connection exposed to clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlAssistantConnection {
    /// The daemon is using Codex for chat.
    Codex,
}

/// Memory storage readiness shown by clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlMemoryStorageStatus {
    /// The canonical memory store is ready.
    Ready,
    /// The canonical memory store is initializing.
    Initializing,
}

/// Local status returned by `Query.localStatus`.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlLocalStatus {
    /// Local service status.
    pub local_service: GraphqlLocalServiceStatus,
    /// Assistant connection status.
    pub assistant_connection: GraphqlAssistantConnection,
    /// Memory storage status.
    pub memory_storage: GraphqlMemoryStorageStatus,
}
```

- [ ] **Step 7: Create the initial schema roots**

Create `crates/noema-core/src/graphql/schema.rs`:

```rust
use async_graphql::{Context, EmptyMutation, Object, Schema, Subscription};
use futures_util::{Stream, stream};

use super::types::{
    GraphqlAssistantConnection, GraphqlLocalServiceStatus, GraphqlLocalStatus,
    GraphqlMemoryStorageStatus,
};

/// Concrete GraphQL schema type used by the web server.
pub type GraphqlSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Shared state available to GraphQL resolvers.
#[derive(Clone, Debug)]
pub struct GraphqlState {
    memory_storage_ready: bool,
}

impl GraphqlState {
    /// Build test state with ready memory storage.
    #[must_use]
    pub const fn for_tests() -> Self {
        Self {
            memory_storage_ready: true,
        }
    }
}

/// Build the Noema GraphQL schema.
#[must_use]
pub fn build_schema(state: GraphqlState) -> GraphqlSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(state)
        .finish()
}

/// Root GraphQL query object.
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Return local Noema status.
    async fn local_status(&self, ctx: &Context<'_>) -> GraphqlLocalStatus {
        let state = ctx.data_unchecked::<GraphqlState>();
        let memory_storage = if state.memory_storage_ready {
            GraphqlMemoryStorageStatus::Ready
        } else {
            GraphqlMemoryStorageStatus::Initializing
        };

        GraphqlLocalStatus {
            local_service: GraphqlLocalServiceStatus::Running,
            assistant_connection: GraphqlAssistantConnection::Codex,
            memory_storage,
        }
    }

    /// Return onboarding status.
    async fn onboarding_status(&self) -> bool {
        true
    }
}

/// Root GraphQL mutation object.
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Start a provider auth attempt.
    async fn start_provider_auth_attempt(&self) -> bool {
        true
    }

    /// Start or resume the primary conversation.
    async fn start_primary_conversation(&self) -> bool {
        true
    }

    /// Send a conversation turn.
    async fn send_conversation_turn(&self) -> bool {
        true
    }
}

/// Root GraphQL subscription object.
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Stream conversation events.
    async fn conversation_events(&self) -> impl Stream<Item = bool> {
        stream::once(async { true })
    }
}
```

This is intentionally small but compilable. Later tasks replace the temporary boolean fields with real typed fields and resolver calls.

- [ ] **Step 8: Run the schema test**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::schema_sdl_exposes_initial_noema_fields
```

Expected: PASS.

- [ ] **Step 9: Run formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS. If it fails, run `cargo fmt --all`, then rerun the check.

- [ ] **Step 10: Checkpoint**

Commit only if this execution run is explicitly in commit/ship mode. Otherwise leave changes unstaged and report them.

## Task 2: Add Real GraphQL Types For Existing Web Protocol Parity

**Files:**
- Modify: `crates/noema-core/src/graphql/types.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Test: `crates/noema-core/src/graphql/types.rs`

- [ ] **Step 1: Add conversion tests**

Add tests to `crates/noema-core/src/graphql/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentStatus, TurnActivityStatus, TurnTranscriptItem};

    #[test]
    fn agent_status_converts_to_graphql() {
        assert_eq!(
            GraphqlAgentStatus::from(AgentStatus::Thinking),
            GraphqlAgentStatus::Thinking
        );
    }

    #[test]
    fn transcript_item_converts_to_graphql_user_text() {
        let item = GraphqlTranscriptItem::from(TurnTranscriptItem::UserText {
            text: "hello".to_string(),
        });

        match item {
            GraphqlTranscriptItem::UserText(value) => assert_eq!(value.text, "hello"),
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn activity_status_converts_to_graphql() {
        assert_eq!(
            GraphqlTurnActivityStatus::from(TurnActivityStatus::Completed),
            GraphqlTurnActivityStatus::Completed
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p noema-core graphql::types::tests
```

Expected: FAIL because the GraphQL protocol types do not exist yet.

- [ ] **Step 3: Replace `types.rs` with complete existing-slice types**

Replace `crates/noema-core/src/graphql/types.rs` with:

```rust
use async_graphql::{Enum, InputObject, SimpleObject, Union};
use serde_json::Value;

use crate::{
    AgentStatus, OnboardingStatus, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
    ProviderAuthMethod, ProviderAccountStatus, TurnActivityStatus, TurnTranscriptItem,
};

/// Local service status shown by clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlLocalServiceStatus {
    /// The local Noema service is running.
    Running,
}

/// Assistant connection exposed to clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlAssistantConnection {
    /// The daemon is using Codex for chat.
    Codex,
}

/// Memory storage readiness shown by clients.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlMemoryStorageStatus {
    /// The canonical memory store is ready.
    Ready,
    /// The canonical memory store is initializing.
    Initializing,
}

/// Local status returned by `Query.localStatus`.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlLocalStatus {
    /// Local service status.
    pub local_service: GraphqlLocalServiceStatus,
    /// Assistant connection status.
    pub assistant_connection: GraphqlAssistantConnection,
    /// Memory storage status.
    pub memory_storage: GraphqlMemoryStorageStatus,
}

/// Provider auth method exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlProviderAuthMethod {
    /// OAuth device-code flow.
    OauthDeviceCode,
    /// Secret input flow.
    SecretInput,
    /// External manual flow.
    ExternalManual,
    /// No auth required.
    None,
}

impl From<ProviderAuthMethod> for GraphqlProviderAuthMethod {
    fn from(method: ProviderAuthMethod) -> Self {
        match method {
            ProviderAuthMethod::OauthDeviceCode => Self::OauthDeviceCode,
            ProviderAuthMethod::SecretInput => Self::SecretInput,
            ProviderAuthMethod::ExternalManual => Self::ExternalManual,
            ProviderAuthMethod::None => Self::None,
        }
    }
}

/// Provider account status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlProviderAccountStatus {
    /// Status has not been checked.
    Unknown,
    /// Status is being checked.
    Checking,
    /// Account is authenticated.
    Authenticated,
    /// Account is unauthenticated.
    Unauthenticated,
    /// Provider is unavailable.
    Unavailable,
}

impl From<ProviderAccountStatus> for GraphqlProviderAccountStatus {
    fn from(status: ProviderAccountStatus) -> Self {
        match status {
            ProviderAccountStatus::Unknown => Self::Unknown,
            ProviderAccountStatus::Checking => Self::Checking,
            ProviderAccountStatus::Authenticated => Self::Authenticated,
            ProviderAccountStatus::Unauthenticated => Self::Unauthenticated,
            ProviderAccountStatus::Unavailable => Self::Unavailable,
        }
    }
}

/// Onboarding step status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlOnboardingStepStatus {
    /// Step is complete.
    Complete,
    /// Step requires user action.
    NeedsAction,
    /// Step is blocked by an earlier step.
    Blocked,
}

/// One onboarding step.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlOnboardingStep {
    /// Stable step id.
    pub id: String,
    /// Human-facing step label.
    pub label: String,
    /// Step status.
    pub status: GraphqlOnboardingStepStatus,
    /// Provider family when the step is provider-backed.
    pub provider_kind: Option<String>,
    /// Provider account id when the step is provider-backed.
    pub provider_account_id: Option<String>,
    /// Auth method when the step can start auth.
    pub auth_method: Option<GraphqlProviderAuthMethod>,
}

/// Onboarding status.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlOnboardingStatus {
    /// Whether chat can start.
    pub is_user_onboarded: bool,
    /// Ordered onboarding steps.
    pub steps: Vec<GraphqlOnboardingStep>,
}

impl From<OnboardingStatus> for GraphqlOnboardingStatus {
    fn from(status: OnboardingStatus) -> Self {
        Self {
            is_user_onboarded: status.is_user_onboarded,
            steps: status
                .steps
                .into_iter()
                .map(|step| GraphqlOnboardingStep {
                    id: step.id,
                    label: step.label,
                    status: match step.status {
                        crate::OnboardingStepStatus::Complete => {
                            GraphqlOnboardingStepStatus::Complete
                        }
                        crate::OnboardingStepStatus::NeedsAction => {
                            GraphqlOnboardingStepStatus::NeedsAction
                        }
                        crate::OnboardingStepStatus::Blocked => GraphqlOnboardingStepStatus::Blocked,
                    },
                    provider_kind: step.provider_kind,
                    provider_account_id: step.provider_account_id,
                    auth_method: step.auth_method.map(Into::into),
                })
                .collect(),
        }
    }
}

/// Input for starting a provider auth attempt.
#[derive(Clone, Debug, InputObject)]
pub struct GraphqlStartProviderAuthAttemptInput {
    /// Provider family, such as `codex`.
    pub provider_kind: String,
    /// Stable provider account id.
    pub provider_account_id: String,
    /// Requested authentication method.
    pub method: GraphqlProviderAuthMethod,
}

/// Provider auth attempt status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlProviderAuthAttemptStatus {
    /// Attempt is starting.
    Starting,
    /// Waiting for the user.
    WaitingForUser,
    /// Attempt completed.
    Completed,
    /// Attempt failed.
    Failed,
    /// Attempt expired.
    Expired,
    /// Attempt was cancelled.
    Cancelled,
}

impl From<ProviderAuthAttemptStatus> for GraphqlProviderAuthAttemptStatus {
    fn from(status: ProviderAuthAttemptStatus) -> Self {
        match status {
            ProviderAuthAttemptStatus::Starting => Self::Starting,
            ProviderAuthAttemptStatus::WaitingForUser => Self::WaitingForUser,
            ProviderAuthAttemptStatus::Completed => Self::Completed,
            ProviderAuthAttemptStatus::Failed => Self::Failed,
            ProviderAuthAttemptStatus::Expired => Self::Expired,
            ProviderAuthAttemptStatus::Cancelled => Self::Cancelled,
        }
    }
}

/// Provider auth attempt view.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlProviderAuthAttempt {
    /// Short-lived auth attempt id.
    pub attempt_id: String,
    /// Provider family.
    pub provider_kind: String,
    /// Provider account id.
    pub provider_account_id: String,
    /// Attempt status.
    pub status: GraphqlProviderAuthAttemptStatus,
    /// Verification uri when available.
    pub verification_uri: Option<String>,
    /// User code when available.
    pub user_code: Option<String>,
    /// Failure message when available.
    pub error_message: Option<String>,
}

impl From<ProviderAuthAttemptView> for GraphqlProviderAuthAttempt {
    fn from(view: ProviderAuthAttemptView) -> Self {
        Self {
            attempt_id: view.attempt_id,
            provider_kind: view.provider_kind,
            provider_account_id: view.provider_account_id,
            status: view.status.into(),
            verification_uri: view.verification_uri,
            user_code: view.user_code,
            error_message: view.error_message,
        }
    }
}

/// Agent status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlAgentStatus {
    /// No agent work is active.
    Idle,
    /// Input has been accepted.
    InputReceived,
    /// Agent is producing or planning.
    Thinking,
    /// Agent is waiting on a tool invocation.
    ToolRunning,
    /// A newer turn is waiting for a previous turn.
    WaitingForPreviousTurnCompletion,
    /// Agent is interrupting a previous turn.
    Interrupting,
    /// Conversation is in an error state.
    Error,
}

impl From<AgentStatus> for GraphqlAgentStatus {
    fn from(status: AgentStatus) -> Self {
        match status {
            AgentStatus::Idle => Self::Idle,
            AgentStatus::InputReceived => Self::InputReceived,
            AgentStatus::Thinking => Self::Thinking,
            AgentStatus::ToolRunning => Self::ToolRunning,
            AgentStatus::WaitingForPreviousTurnCompletion => {
                Self::WaitingForPreviousTurnCompletion
            }
            AgentStatus::Interrupting => Self::Interrupting,
            AgentStatus::Error => Self::Error,
        }
    }
}

/// Turn activity status exposed through GraphQL.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GraphqlTurnActivityStatus {
    /// Activity started.
    Started,
    /// Activity completed.
    Completed,
    /// Activity failed.
    Failed,
}

impl From<TurnActivityStatus> for GraphqlTurnActivityStatus {
    fn from(status: TurnActivityStatus) -> Self {
        match status {
            TurnActivityStatus::Started => Self::Started,
            TurnActivityStatus::Completed => Self::Completed,
            TurnActivityStatus::Failed => Self::Failed,
        }
    }
}

/// User text transcript item.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlUserText {
    /// Text authored by the user.
    pub text: String,
}

/// Assistant text transcript item.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAssistantText {
    /// Text to render as the assistant response.
    pub text: String,
}

/// Activity transcript item.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlActivity {
    /// Stable activity id.
    pub id: String,
    /// Activity kind.
    pub activity_kind: String,
    /// Activity status.
    pub status: GraphqlTurnActivityStatus,
    /// Short display title.
    pub title: String,
    /// Optional summary.
    pub summary: Option<String>,
    /// Structured metadata as JSON.
    pub metadata: Value,
}

/// Structured card transcript item.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlA2uiCard {
    /// Stable card id.
    pub id: String,
    /// Card schema.
    pub schema: String,
    /// Card payload as JSON.
    pub payload: Value,
}

/// Error notice transcript item.
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlErrorNotice {
    /// Human-readable error message.
    pub message: String,
    /// Whether the chat turn can continue.
    pub recoverable: bool,
}

/// Transcript item union.
#[derive(Clone, Debug, Union)]
pub enum GraphqlTranscriptItem {
    /// User text.
    UserText(GraphqlUserText),
    /// Assistant text.
    AssistantText(GraphqlAssistantText),
    /// Activity row.
    Activity(GraphqlActivity),
    /// Structured card.
    A2uiCard(GraphqlA2uiCard),
    /// Error notice.
    ErrorNotice(GraphqlErrorNotice),
}

impl From<TurnTranscriptItem> for GraphqlTranscriptItem {
    fn from(item: TurnTranscriptItem) -> Self {
        match item {
            TurnTranscriptItem::UserText { text } => Self::UserText(GraphqlUserText { text }),
            TurnTranscriptItem::AssistantText { text } => {
                Self::AssistantText(GraphqlAssistantText { text })
            }
            TurnTranscriptItem::Activity {
                id,
                activity_kind,
                status,
                title,
                summary,
                metadata,
            } => Self::Activity(GraphqlActivity {
                id,
                activity_kind,
                status: status.into(),
                title,
                summary,
                metadata,
            }),
            TurnTranscriptItem::A2uiCard {
                id,
                schema,
                payload,
            } => Self::A2uiCard(GraphqlA2uiCard {
                id,
                schema,
                payload,
            }),
            TurnTranscriptItem::ErrorNotice {
                message,
                recoverable,
            } => Self::ErrorNotice(GraphqlErrorNotice {
                message,
                recoverable,
            }),
        }
    }
}
```

- [ ] **Step 4: Update schema roots to use real type names**

In `crates/noema-core/src/graphql/schema.rs`, replace the temporary boolean fields with signatures that return the real types but still use test-safe values:

```rust
async fn onboarding_status(&self) -> GraphqlOnboardingStatus
```

```rust
async fn start_provider_auth_attempt(
    &self,
    _input: GraphqlStartProviderAuthAttemptInput,
) -> GraphqlProviderAuthAttempt
```

```rust
async fn start_primary_conversation(&self) -> GraphqlConversationStarted
```

```rust
async fn send_conversation_turn(&self, _input: GraphqlSendConversationTurnInput) -> GraphqlTurnAccepted
```

Add `GraphqlConversationStarted`, `GraphqlSendConversationTurnInput`, and `GraphqlTurnAccepted` in `types.rs` with fields matching the current web protocol:

```rust
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlConversationStarted {
    pub conversation_id: String,
    pub provider: String,
    pub replay: Vec<GraphqlConversationItem>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlConversationItem {
    pub item_id: String,
    pub turn_id: Option<String>,
    pub item: GraphqlTranscriptItem,
}

#[derive(Clone, Debug, InputObject)]
pub struct GraphqlSendConversationTurnInput {
    pub conversation_id: String,
    pub input: String,
    pub client_message_id: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlTurnAccepted {
    pub conversation_id: String,
    pub client_message_id: Option<String>,
}
```

- [ ] **Step 5: Run conversion tests**

Run:

```bash
cargo test -p noema-core graphql::types::tests
```

Expected: PASS.

- [ ] **Step 6: Run schema SDL test**

Run:

```bash
cargo test -p noema-core graphql::schema::tests::schema_sdl_exposes_initial_noema_fields
```

Expected: PASS with real GraphQL type names in the SDL.

## Task 3: Wire Queries And Mutations To Existing Noema State

**Files:**
- Create: `crates/noema-core/src/graphql/resolvers.rs`
- Modify: `crates/noema-core/src/graphql.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Test: `crates/noema-core/src/graphql/resolvers.rs`

- [ ] **Step 1: Add resolver tests for status and onboarding**

Create `crates/noema-core/src/graphql/resolvers.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use crate::graphql::{GraphqlState, build_schema};
    use async_graphql::{Request, Value};

    #[tokio::test]
    async fn local_status_query_returns_running_codex() {
        let schema = build_schema(GraphqlState::for_tests());
        let response = schema
            .execute(Request::new(
                "{ localStatus { localService assistantConnection memoryStorage } }",
            ))
            .await
            .into_result()
            .expect("query should succeed");

        assert_eq!(
            response.data,
            Value::from_json(serde_json::json!({
                "localStatus": {
                    "localService": "RUNNING",
                    "assistantConnection": "CODEX",
                    "memoryStorage": "READY"
                }
            }))
            .expect("valid json")
        );
    }
}
```

- [ ] **Step 2: Run resolver test**

Run:

```bash
cargo test -p noema-core graphql::resolvers::tests::local_status_query_returns_running_codex
```

Expected: FAIL until `graphql.rs` includes the `resolvers` module or until schema names are aligned.

- [ ] **Step 3: Add resolver module and shared state**

In `crates/noema-core/src/graphql.rs`, add:

```rust
mod resolvers;
```

In `crates/noema-core/src/graphql/schema.rs`, expand `GraphqlState`:

```rust
#[derive(Clone)]
pub struct GraphqlState {
    pub(crate) web_state: Option<crate::daemon::web::WebState>,
    pub(crate) memory_storage_ready: bool,
}

impl GraphqlState {
    #[must_use]
    pub const fn for_tests() -> Self {
        Self {
            web_state: None,
            memory_storage_ready: true,
        }
    }

    #[must_use]
    pub fn from_web_state(web_state: crate::daemon::web::WebState) -> Self {
        Self {
            web_state: Some(web_state),
            memory_storage_ready: true,
        }
    }

    pub(crate) fn web_state(&self) -> async_graphql::Result<&crate::daemon::web::WebState> {
        self.web_state
            .as_ref()
            .ok_or_else(|| async_graphql::Error::new("Noema web state is unavailable"))
    }
}
```

Make `WebState` visible to the GraphQL module by changing `crates/noema-core/src/daemon/web/mod.rs`:

```rust
pub(crate) struct WebState {
```

and expose narrow accessor methods rather than fields:

```rust
impl WebState {
    pub(crate) fn runtime(&self) -> &CodexRuntimeHandle {
        &self.runtime
    }

    pub(crate) fn memory_repository(&self) -> &PostgresMemoryRepository {
        &self.memory_repository
    }

    pub(crate) fn provider_auth(&self) -> &ProviderAuthManager {
        &self.provider_auth
    }

    pub(crate) fn paths(&self) -> &crate::NoemaPaths {
        &self.paths
    }

    pub(crate) fn codex_command(&self) -> &str {
        &self.codex_command
    }
}
```

- [ ] **Step 4: Move reusable web helpers to `pub(crate)`**

In `crates/noema-core/src/daemon/web/mod.rs`, make these helpers `pub(crate)` so GraphQL resolvers can reuse behavior without duplicating policy or auth logic:

```rust
pub(crate) async fn reconcile_onboarding_provider_account(
    status_store: &impl ProviderAccountStatusStore,
    paths: &crate::NoemaPaths,
    account: Option<crate::ProviderAccountRecord>,
) -> Result<Option<crate::ProviderAccountRecord>, DaemonError>
```

```rust
pub(crate) fn is_user_onboarded_for_chat(
    account: Option<crate::ProviderAccountRecord>,
) -> bool
```

```rust
pub(crate) async fn visible_conversation_replay(
    repo: &PostgresMemoryRepository,
    conversation_id: &str,
) -> Result<Vec<ConversationItemRecord>, DaemonError>
```

```rust
pub(crate) fn web_conversation_item_from_record(
    record: ConversationItemRecord,
) -> Result<Option<WebConversationItem>, DaemonError>
```

Do not expose raw fields. Keep behavior centralized.

- [ ] **Step 5: Implement query resolvers**

In `QueryRoot`, implement:

```rust
async fn local_status(&self, ctx: &Context<'_>) -> GraphqlLocalStatus
```

using current `web_status_from_state` behavior:

```rust
GraphqlLocalStatus {
    local_service: GraphqlLocalServiceStatus::Running,
    assistant_connection: GraphqlAssistantConnection::Codex,
    memory_storage: GraphqlMemoryStorageStatus::Ready,
}
```

Implement onboarding:

```rust
async fn onboarding_status(&self, ctx: &Context<'_>) -> async_graphql::Result<GraphqlOnboardingStatus> {
    let state = ctx.data_unchecked::<GraphqlState>();
    let web = state.web_state()?;
    let account = web
        .memory_repository()
        .active_provider_account("codex")
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let account = crate::daemon::web::reconcile_onboarding_provider_account(
        web.memory_repository(),
        web.paths(),
        account,
    )
    .await
    .map_err(|error| async_graphql::Error::new(error.to_string()))?;

    Ok(crate::onboarding_status_from_account(account).into())
}
```

- [ ] **Step 6: Implement mutation resolvers**

Implement provider auth start/check/cancel as GraphQL mutations or query/mutation pair:

```rust
async fn start_provider_auth_attempt(
    &self,
    ctx: &Context<'_>,
    input: GraphqlStartProviderAuthAttemptInput,
) -> async_graphql::Result<GraphqlProviderAuthAttempt>
```

Map `GraphqlProviderAuthMethod` back to `ProviderAuthMethod` using a `From` or `TryFrom` impl. Reuse the existing `start_provider_auth_attempt` logic by extracting the body of the HTTP handler into a reusable helper that accepts `StartProviderAuthAttemptRequest` and returns `ProviderAuthAttemptView`.

Implement primary conversation:

```rust
async fn start_primary_conversation(
    &self,
    ctx: &Context<'_>,
    model: Option<String>,
    cwd: Option<String>,
) -> async_graphql::Result<GraphqlConversationStarted>
```

Call `web.runtime().start_primary_conversation(model, cwd).await`, then call `visible_conversation_replay`, convert each item with `web_conversation_item_from_record`, and return `GraphqlConversationStarted`.

Implement send turn acceptance:

```rust
async fn send_conversation_turn(
    &self,
    ctx: &Context<'_>,
    input: GraphqlSendConversationTurnInput,
) -> async_graphql::Result<GraphqlTurnAccepted>
```

This mutation will enqueue/execute the turn through the stream bridge built in Task 5. In this task, keep the mutation unavailable with a typed GraphQL error:

```rust
Err(async_graphql::Error::new(
    "conversation turn streaming is not connected yet",
))
```

Task 5 replaces this error with the stream bridge.

- [ ] **Step 7: Run resolver tests**

Run:

```bash
cargo test -p noema-core graphql
```

Expected: PASS for status/onboarding tests. The turn mutation test should not exist until Task 5.

## Task 4: Add GraphQL HTTP Endpoint And Schema Export

**Files:**
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/daemon/server.rs`
- Modify: `crates/noema-core/src/bin/export_frontend_types.rs`
- Test: `crates/noema-core/src/daemon/web/mod.rs`

- [ ] **Step 1: Add HTTP GraphQL tests**

In `crates/noema-core/src/daemon/web/mod.rs`, add unit tests for the request router helpers:

```rust
#[test]
fn graphql_endpoint_accepts_post_path() {
    assert!(is_graphql_http_route("POST", "/graphql"));
}

#[test]
fn graphql_schema_endpoint_accepts_get_path() {
    assert!(is_graphql_schema_route("GET", "/graphql/schema.graphql"));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p noema-core daemon::web::tests::graphql_endpoint_accepts_post_path daemon::web::tests::graphql_schema_endpoint_accepts_get_path
```

Expected: FAIL because route helpers do not exist.

- [ ] **Step 3: Add route helpers**

In `crates/noema-core/src/daemon/web/mod.rs`, add:

```rust
fn is_graphql_http_route(method: &str, path: &str) -> bool {
    method == "POST" && path == "/graphql"
}

fn is_graphql_schema_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/graphql/schema.graphql"
}
```

- [ ] **Step 4: Store GraphQL schema in web state**

Split `WebState` into an `Arc`-backed core plus schema so GraphQL state can hold the same core without a self-reference:

```rust
#[derive(Clone)]
pub(crate) struct WebState {
    core: std::sync::Arc<WebStateCore>,
    graphql_schema: crate::graphql::GraphqlSchema,
}

pub(crate) struct WebStateCore {
    runtime: CodexRuntimeHandle,
    memory_repository: PostgresMemoryRepository,
    provider_auth: ProviderAuthManager,
    paths: crate::NoemaPaths,
    codex_command: String,
}
```

Change `WebState::new` to build the core first:

```rust
let core = std::sync::Arc::new(WebStateCore {
    runtime,
    memory_repository,
    provider_auth,
    paths,
    codex_command,
});
let graphql_schema = crate::graphql::build_schema(crate::graphql::GraphqlState::from_web_core(
    std::sync::Arc::clone(&core),
));
Self {
    core,
    graphql_schema,
}
```

Update `GraphqlState` to store `Option<Arc<WebStateCore>>` instead of `Option<WebState>`.

- [ ] **Step 5: Implement `/graphql/schema.graphql`**

In `handle_connection`, before legacy product routes:

```rust
if is_graphql_schema_route(&request.method, &request.path) {
    write_response(
        &mut stream,
        "200 OK",
        "text/plain; charset=utf-8",
        state.graphql_schema().sdl().as_bytes(),
    )
    .await?;
    return Ok(());
}
```

Add accessor:

```rust
pub(crate) fn graphql_schema(&self) -> &crate::graphql::GraphqlSchema {
    &self.graphql_schema
}
```

- [ ] **Step 6: Implement `/graphql` POST**

In `handle_connection`:

```rust
if is_graphql_http_route(&request.method, &request.path) {
    let graphql_request: async_graphql::Request =
        serde_json::from_slice(&request.body).map_err(|source| {
            DaemonError::Protocol(format!("invalid GraphQL request body: {source}"))
        })?;
    let response = state.graphql_schema().execute(graphql_request).await;
    write_json(&mut stream, "200 OK", &response).await?;
    return Ok(());
}
```

- [ ] **Step 7: Change frontend type export binary to export schema**

Replace `crates/noema-core/src/bin/export_frontend_types.rs` with:

```rust
//! Export GraphQL schema for frontend code generation.

use std::{env, fs, path::PathBuf};

fn main() -> std::io::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_path = manifest_dir.join("web/src/generated/schema.graphql");
    let schema = noema_core::graphql::build_schema(noema_core::graphql::GraphqlState::for_tests());

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, schema.sdl())?;
    println!("wrote {}", output_path.display());
    Ok(())
}
```

- [ ] **Step 8: Run endpoint route tests**

Run:

```bash
cargo test -p noema-core daemon::web::tests::graphql_endpoint_accepts_post_path daemon::web::tests::graphql_schema_endpoint_accepts_get_path
```

Expected: PASS.

- [ ] **Step 9: Run schema export**

Run:

```bash
cargo run --manifest-path crates/noema-core/Cargo.toml --bin export_frontend_types
```

Expected: writes `crates/noema-core/web/src/generated/schema.graphql`.

## Task 5: Implement Conversation Subscription Bridge

**Files:**
- Create: `crates/noema-core/src/graphql/subscriptions.rs`
- Modify: `crates/noema-core/src/graphql.rs`
- Modify: `crates/noema-core/src/graphql/schema.rs`
- Modify: `crates/noema-core/src/graphql/types.rs`
- Test: `crates/noema-core/src/graphql/subscriptions.rs`

- [ ] **Step 1: Add subscription registry tests**

Create `crates/noema-core/src/graphql/subscriptions.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentStatus, daemon::protocol::TurnStreamEvent};

    #[tokio::test]
    async fn registry_delivers_events_to_subscriber() {
        let registry = ConversationSubscriptionRegistry::default();
        let mut rx = registry.subscribe("conversation_1");

        registry.publish(TurnStreamEvent::AgentStatusChanged {
            conversation_id: "conversation_1".to_string(),
            status: AgentStatus::Thinking,
        });

        let event = rx.recv().await.expect("event should be delivered");
        assert_eq!(event.conversation_id(), "conversation_1");
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test -p noema-core graphql::subscriptions::tests::registry_delivers_events_to_subscriber
```

Expected: FAIL because the registry does not exist.

- [ ] **Step 3: Implement registry**

Create the registry:

```rust
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use crate::daemon::protocol::TurnStreamEvent;

/// In-process conversation event registry for GraphQL subscriptions.
#[derive(Clone, Default)]
pub struct ConversationSubscriptionRegistry {
    inner: Arc<Mutex<HashMap<String, broadcast::Sender<TurnStreamEvent>>>>,
}

impl ConversationSubscriptionRegistry {
    /// Subscribe to one conversation's live turn events.
    pub fn subscribe(&self, conversation_id: &str) -> broadcast::Receiver<TurnStreamEvent> {
        self.sender(conversation_id).subscribe()
    }

    /// Publish a live event to subscribers.
    pub fn publish(&self, event: TurnStreamEvent) {
        let sender = self.sender(event.conversation_id());
        let _ = sender.send(event);
    }

    fn sender(&self, conversation_id: &str) -> broadcast::Sender<TurnStreamEvent> {
        let mut inner = self.inner.lock().expect("subscription registry poisoned");
        inner
            .entry(conversation_id.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }
}
```

Add a method to `TurnStreamEvent` in `crates/noema-core/src/daemon/protocol.rs`:

```rust
impl TurnStreamEvent {
    pub(crate) fn conversation_id(&self) -> &str {
        match self {
            Self::ConversationItem { conversation_id, .. }
            | Self::AgentStatusChanged { conversation_id, .. } => conversation_id,
        }
    }
}
```

- [ ] **Step 4: Add GraphQL event union**

In `types.rs`, add:

```rust
#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlConversationItemEvent {
    pub conversation_id: String,
    pub client_message_id: Option<String>,
    pub item_id: String,
    pub turn_id: Option<String>,
    pub item: GraphqlTranscriptItem,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlAgentStatusEvent {
    pub conversation_id: String,
    pub status: GraphqlAgentStatus,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GraphqlTurnCompletedEvent {
    pub conversation_id: String,
    pub client_message_id: Option<String>,
}

#[derive(Clone, Debug, Union)]
pub enum GraphqlConversationEvent {
    ConversationItem(GraphqlConversationItemEvent),
    AgentStatusChanged(GraphqlAgentStatusEvent),
    TurnCompleted(GraphqlTurnCompletedEvent),
}
```

- [ ] **Step 5: Wire subscription resolver**

In `schema.rs`, replace the boolean subscription:

```rust
async fn conversation_events(
    &self,
    ctx: &Context<'_>,
    conversation_id: String,
) -> impl Stream<Item = GraphqlConversationEvent> {
    let state = ctx.data_unchecked::<GraphqlState>();
    let mut rx = state.subscriptions().subscribe(&conversation_id);

    async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            match event {
                TurnStreamEvent::ConversationItem {
                    conversation_id,
                    item_id,
                    turn_id,
                    item,
                } => {
                    yield GraphqlConversationEvent::ConversationItem(GraphqlConversationItemEvent {
                        conversation_id,
                        client_message_id: None,
                        item_id,
                        turn_id,
                        item: item.into(),
                    });
                }
                TurnStreamEvent::AgentStatusChanged {
                    conversation_id,
                    status,
                } => {
                    yield GraphqlConversationEvent::AgentStatusChanged(GraphqlAgentStatusEvent {
                        conversation_id,
                        status: status.into(),
                    });
                }
            }
        }
    }
}
```

Add the registry to `GraphqlState`:

```rust
subscriptions: crate::graphql::subscriptions::ConversationSubscriptionRegistry,
```

and accessor:

```rust
pub(crate) fn subscriptions(&self) -> &ConversationSubscriptionRegistry {
    &self.subscriptions
}
```

- [ ] **Step 6: Make `sendConversationTurn` publish events**

In the mutation resolver, replace the temporary error with:

```rust
let (item_tx, mut item_rx) = tokio::sync::mpsc::unbounded_channel();
let completion = web
    .runtime()
    .turn(input.conversation_id.clone(), input.input, item_tx);
let registry = state.subscriptions().clone();
let conversation_id = input.conversation_id.clone();
let client_message_id = input.client_message_id.clone();

tokio::spawn(async move {
    tokio::pin!(completion);
    loop {
        tokio::select! {
            Some(event) = item_rx.recv() => registry.publish(event),
            result = &mut completion => {
                while let Ok(event) = item_rx.try_recv() {
                    registry.publish(event);
                }
                if result.is_ok() {
                    registry.publish_turn_completed(conversation_id, client_message_id);
                }
                break;
            }
        }
    }
});

Ok(GraphqlTurnAccepted {
    conversation_id: input.conversation_id,
    client_message_id: input.client_message_id,
})
```

Add `publish_turn_completed` to the registry by storing a separate GraphQL event enum internally if `TurnStreamEvent` cannot carry completion cleanly. Prefer a small local enum:

```rust
pub enum ConversationLiveEvent {
    Turn(TurnStreamEvent),
    Completed {
        conversation_id: String,
        client_message_id: Option<String>,
    },
}
```

Use `ConversationLiveEvent` in the broadcast channel, and convert it in the subscription resolver.

- [ ] **Step 7: Run subscription tests**

Run:

```bash
cargo test -p noema-core graphql::subscriptions
```

Expected: PASS.

## Task 6: Add GraphQL WebSocket Route

**Files:**
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Test: `crates/noema-core/src/daemon/web/mod.rs`

- [ ] **Step 1: Add route test**

Add:

```rust
#[test]
fn graphql_ws_endpoint_accepts_get_path() {
    assert!(is_graphql_ws_route("GET", "/graphql/ws"));
}
```

- [ ] **Step 2: Add helper**

```rust
fn is_graphql_ws_route(method: &str, path: &str) -> bool {
    method == "GET" && path == "/graphql/ws"
}
```

- [ ] **Step 3: Implement minimal GraphQL WebSocket protocol**

Add a new branch before the legacy `/api/chat/ws` branch:

```rust
if is_graphql_ws_route(&request.method, &request.path) {
    upgrade_graphql_websocket(stream, &request, state).await?;
    return Ok(());
}
```

Implement `upgrade_graphql_websocket` using the existing `upgrade_websocket` handshake code, but route text frames to `async_graphql` subscription execution. Support these client messages:

```json
{"type":"connection_init"}
{"id":"1","type":"subscribe","payload":{"query":"subscription ConversationEvents($conversationId: String!) { conversationEvents(conversationId: $conversationId) { __typename } }","variables":{"conversationId":"conversation_1"}}}
{"id":"1","type":"complete"}
```

Emit:

```json
{"type":"connection_ack"}
{"id":"1","type":"next","payload":{"data":{"conversationEvents":{"__typename":"GraphqlAgentStatusEvent"}}}}
{"id":"1","type":"error","payload":[{"message":"GraphQL subscription failed"}]}
{"id":"1","type":"complete"}
```

This is the `graphql-transport-ws` shape. Keep implementation narrow: one active operation per connection is acceptable for the current web UI.

- [ ] **Step 4: Run route test**

Run:

```bash
cargo test -p noema-core daemon::web::tests::graphql_ws_endpoint_accepts_get_path
```

Expected: PASS.

- [ ] **Step 5: Run Rust checks**

Run:

```bash
cargo check -p noema-core
```

Expected: PASS.

## Task 7: Generate Web GraphQL Types

**Files:**
- Modify: `crates/noema-core/web/package.json`
- Create: `crates/noema-core/web/codegen.ts`
- Create: `crates/noema-core/web/src/graphql/operations.ts`
- Generate: `crates/noema-core/web/src/generated/graphql.ts`

- [ ] **Step 1: Add GraphQL web dependencies**

Run in `crates/noema-core/web`:

```bash
bun add @apollo/client graphql graphql-ws
bun add -d @graphql-codegen/cli @graphql-codegen/typescript @graphql-codegen/typescript-operations @graphql-codegen/typescript-react-apollo
```

Expected: `package.json` and lockfile update.

- [ ] **Step 2: Replace scripts**

Update scripts in `crates/noema-core/web/package.json`:

```json
{
  "gen:schema": "cargo run --manifest-path ../Cargo.toml --bin export_frontend_types",
  "gen:types": "bun run gen:schema && graphql-codegen --config codegen.ts",
  "dev": "bun run gen:types && vite build --watch",
  "dev:assets": "vite build --watch",
  "build": "bun run gen:types && vite build",
  "lint": "bun run gen:types && tsc --noEmit && eslint src --max-warnings=0"
}
```

- [ ] **Step 3: Add codegen config**

Create `crates/noema-core/web/codegen.ts`:

```ts
import type { CodegenConfig } from "@graphql-codegen/cli";

const config: CodegenConfig = {
  schema: "src/generated/schema.graphql",
  documents: ["src/graphql/**/*.ts"],
  generates: {
    "src/generated/graphql.ts": {
      plugins: ["typescript", "typescript-operations", "typescript-react-apollo"],
      config: {
        withHooks: true,
        withComponent: false,
        withHOC: false,
        avoidOptionals: false,
        maybeValue: "T | null",
        apolloReactCommonImportFrom: "@apollo/client",
        apolloReactHooksImportFrom: "@apollo/client/react",
        scalars: {
          JSON: "unknown"
        }
      }
    }
  }
};

export default config;
```

- [ ] **Step 4: Add operation documents**

Create `crates/noema-core/web/src/graphql/operations.ts`:

```ts
import { gql } from "@apollo/client";

export const LocalStatusDocument = gql`
  query LocalStatus {
    localStatus {
      localService
      assistantConnection
      memoryStorage
    }
  }
`;

export const OnboardingStatusDocument = gql`
  query OnboardingStatus {
    onboardingStatus {
      isUserOnboarded
      steps {
        id
        label
        status
        providerKind
        providerAccountId
        authMethod
      }
    }
  }
`;

export const StartProviderAuthAttemptDocument = gql`
  mutation StartProviderAuthAttempt($input: GraphqlStartProviderAuthAttemptInput!) {
    startProviderAuthAttempt(input: $input) {
      attemptId
      providerKind
      providerAccountId
      status
      verificationUri
      userCode
      errorMessage
    }
  }
`;

export const StartPrimaryConversationDocument = gql`
  mutation StartPrimaryConversation {
    startPrimaryConversation {
      conversationId
      provider
      replay {
        itemId
        turnId
        item {
          __typename
          ... on GraphqlUserText { text }
          ... on GraphqlAssistantText { text }
          ... on GraphqlActivity { id activityKind status title summary metadata }
          ... on GraphqlA2uiCard { id schema payload }
          ... on GraphqlErrorNotice { message recoverable }
        }
      }
    }
  }
`;

export const SendConversationTurnDocument = gql`
  mutation SendConversationTurn($input: GraphqlSendConversationTurnInput!) {
    sendConversationTurn(input: $input) {
      conversationId
      clientMessageId
    }
  }
`;

export const ConversationEventsDocument = gql`
  subscription ConversationEvents($conversationId: String!) {
    conversationEvents(conversationId: $conversationId) {
      __typename
      ... on GraphqlConversationItemEvent {
        conversationId
        clientMessageId
        itemId
        turnId
        item {
          __typename
          ... on GraphqlUserText { text }
          ... on GraphqlAssistantText { text }
          ... on GraphqlActivity { id activityKind status title summary metadata }
          ... on GraphqlA2uiCard { id schema payload }
          ... on GraphqlErrorNotice { message recoverable }
        }
      }
      ... on GraphqlAgentStatusEvent {
        conversationId
        status
      }
      ... on GraphqlTurnCompletedEvent {
        conversationId
        clientMessageId
      }
    }
  }
`;
```

- [ ] **Step 5: Generate types**

Run:

```bash
bun run gen:types
```

Expected: `src/generated/schema.graphql` and `src/generated/graphql.ts` are created.

## Task 8: Migrate Web UI To GraphQL

**Files:**
- Create: `crates/noema-core/web/src/graphql/client.ts`
- Modify: `crates/noema-core/web/src/api.ts`
- Modify: `crates/noema-core/web/src/main.tsx`
- Modify: `crates/noema-core/web/src/transcript.ts`
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/types.ts`

- [ ] **Step 1: Add GraphQL client helper**

Create `crates/noema-core/web/src/graphql/client.ts`:

```ts
import {
  ApolloLink,
  ApolloClient,
  HttpLink,
  InMemoryCache
} from "@apollo/client";
import { GraphQLWsLink } from "@apollo/client/link/subscriptions";
import { OperationTypeNode } from "graphql";
import { createClient } from "graphql-ws";

function graphqlWsUrl() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/graphql/ws`;
}

const httpLink = new HttpLink({
  uri: "/graphql"
});

const wsLink = new GraphQLWsLink(
  createClient({
    url: graphqlWsUrl(),
    lazy: true,
    retryAttempts: 5
  })
);

const link = ApolloLink.split(
  ({ operationType }) => {
    return operationType === OperationTypeNode.SUBSCRIPTION;
  },
  wsLink,
  httpLink
);

export const apolloClient = new ApolloClient({
  link,
  cache: new InMemoryCache({
    typePolicies: {
      GraphqlConversationItem: {
        keyFields: ["itemId"]
      },
      GraphqlConversationItemEvent: {
        keyFields: ["itemId"]
      },
      GraphqlAgentStatusEvent: {
        keyFields: false
      },
      GraphqlTurnCompletedEvent: {
        keyFields: false
      }
    }
  })
});
```

This cache policy keeps normalized records for durable conversation items and avoids inventing identities for transient status/completion events. Future task, conversation, memory, workspace, agent, and human types should use stable GraphQL `id` fields and can rely on Apollo's default `__typename:id` identity.

- [ ] **Step 2: Wrap the app in `ApolloProvider`**

In `crates/noema-core/web/src/main.tsx`, import Apollo:

```ts
import { ApolloProvider } from "@apollo/client/react";
import { apolloClient } from "./graphql/client";
```

Wrap the app:

```tsx
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ApolloProvider client={apolloClient}>
      <App />
    </ApolloProvider>
  </React.StrictMode>
);
```

- [ ] **Step 3: Replace status/onboarding API helpers**

Delete `refreshStatus`, `fetchOnboardingStatus`, `startProviderAuthAttempt`, and `fetchProviderAuthAttempt` from `crates/noema-core/web/src/api.ts`. In `App.tsx`, use the generated Apollo hooks instead:

```ts
const localStatus = useLocalStatusQuery();
const onboardingStatus = useOnboardingStatusQuery();
const [startProviderAuthAttempt] = useStartProviderAuthAttemptMutation();
const [startPrimaryConversation] = useStartPrimaryConversationMutation();
const [sendConversationTurn] = useSendConversationTurnMutation();
```

For provider-auth polling, add this operation to `operations.ts` before regenerating if it is missing:

```ts
export const ProviderAuthAttemptDocument = gql`
  query ProviderAuthAttempt($attemptId: String!) {
    providerAuthAttempt(attemptId: $attemptId) {
      attemptId
      providerKind
      providerAccountId
      status
      verificationUri
      userCode
      errorMessage
    }
  }
`;
```

Then use `useProviderAuthAttemptLazyQuery` for the `Check again` action.

- [ ] **Step 4: Convert transcript mapping**

Update `crates/noema-core/web/src/transcript.ts` to map GraphQL union `__typename` values:

```ts
function entryFromGraphqlItem(itemId: string, turnId: string | null | undefined, item: GraphqlTranscriptItem): TranscriptEntry | null {
  if (item.__typename === "GraphqlUserText") {
    return { id: itemId, itemId, turnId: turnId ?? undefined, type: "user", text: item.text };
  }
  if (item.__typename === "GraphqlAssistantText") {
    return { id: itemId, itemId, turnId: turnId ?? undefined, type: "assistant", text: item.text };
  }
  if (item.__typename === "GraphqlActivity") {
    return { id: itemId, itemId, turnId: turnId ?? undefined, type: "activity", item: graphqlActivityToTranscriptItem(item) };
  }
  if (item.__typename === "GraphqlA2uiCard") {
    return { id: itemId, itemId, turnId: turnId ?? undefined, type: "card", item: graphqlCardToTranscriptItem(item) };
  }
  if (item.__typename === "GraphqlErrorNotice") {
    return { id: itemId, itemId, turnId: turnId ?? undefined, type: "error", message: item.message, recoverable: item.recoverable };
  }
  return null;
}
```

- [ ] **Step 5: Replace WebSocket lifecycle in `App.tsx`**

Change startup flow:

1. Call `startPrimaryConversation` mutation after onboarding is complete.
2. Set `conversationId`.
3. Render replay items.
4. Start `conversationEvents` with the generated `useConversationEventsSubscription` hook for that conversation id.
5. Send chat turns with `sendConversationTurn` mutation.

Keep the optimistic user message behavior and replace it when a `GraphqlConversationItemEvent` with the same client message id arrives.

- [ ] **Step 6: Run web checks**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: PASS.

## Task 9: Move CLI Chat Product Operations To GraphQL

**Files:**
- Create: `crates/noema-cli/src/graphql_client.rs`
- Modify: `crates/noema-cli/src/main.rs`
- Modify: `crates/noema-cli/Cargo.toml`
- Test: `crates/noema-cli/src/graphql_client.rs`

- [ ] **Step 1: Add CLI dependencies**

In `crates/noema-cli/Cargo.toml`, add:

```toml
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 2: Add GraphQL client unit test**

Create `crates/noema-cli/src/graphql_client.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_body_includes_query_and_variables() {
        let body = GraphqlRequest::new(
            "query Ping { localStatus { localService } }",
            serde_json::json!({}),
        );

        assert!(body.query.contains("localStatus"));
        assert_eq!(body.variables, serde_json::json!({}));
    }
}
```

- [ ] **Step 3: Implement small GraphQL client**

In `graphql_client.rs`, add:

```rust
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(crate) struct GraphqlRequest {
    pub(crate) query: &'static str,
    pub(crate) variables: Value,
}

impl GraphqlRequest {
    pub(crate) const fn new(query: &'static str, variables: Value) -> Self {
        Self { query, variables }
    }
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

pub(crate) async fn execute<T: DeserializeOwned>(
    base_url: &str,
    request: GraphqlRequest,
) -> Result<T, String> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}/graphql"))
        .json(&request)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let body = response
        .json::<GraphqlResponse<T>>()
        .await
        .map_err(|error| error.to_string())?;
    if let Some(error) = body.errors.and_then(|mut errors| errors.pop()) {
        return Err(error.message);
    }
    body.data.ok_or_else(|| "GraphQL response did not include data".to_string())
}
```

- [ ] **Step 4: Use GraphQL in `run_chat`**

Keep `ConnectedDaemon::connect_or_start` for local lifecycle. After the daemon is running, use the daemon web URL from resolved config as the GraphQL base URL. Replace direct `DaemonClient::start_conversation` and `conversation_turn` calls with GraphQL `startPrimaryConversation` and `sendConversationTurn`.

For interactive streaming, keep the existing daemon client path until Task 10 adds a CLI subscription client. Mark the direct path as temporary in code with:

```rust
// Temporary: CLI streaming uses the daemon socket until the GraphQL subscription
// client lands in the next task. Product start/send operations already use GraphQL.
```

- [ ] **Step 5: Run CLI tests**

Run:

```bash
cargo test -p noema-cli graphql_client
```

Expected: PASS.

## Task 10: Add CLI GraphQL Subscription Streaming

**Files:**
- Modify: `crates/noema-cli/src/graphql_client.rs`
- Modify: `crates/noema-cli/src/main.rs`

- [ ] **Step 1: Add WebSocket client dependency**

Add to root `Cargo.toml`:

```toml
tokio-tungstenite = { version = "0.26", default-features = false, features = ["rustls-tls-webpki-roots"] }
url = "2"
```

Add to `crates/noema-cli/Cargo.toml`:

```toml
tokio-tungstenite.workspace = true
url.workspace = true
```

- [ ] **Step 2: Implement subscription client**

In `graphql_client.rs`, implement a narrow `graphql-transport-ws` client:

```rust
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub(crate) async fn subscribe_conversation_events(
    base_url: &str,
    conversation_id: &str,
) -> Result<mpsc::UnboundedReceiver<Result<Value, String>>, String> {
    let ws_url = base_url
        .replace("http://", "ws://")
        .replace("https://", "wss://")
        + "/graphql/ws";
    let (socket, _) = connect_async(&ws_url)
        .await
        .map_err(|error| error.to_string())?;
    let (mut write, mut read) = socket.split();
    write
        .send(Message::Text(
            serde_json::json!({ "type": "connection_init" }).to_string(),
        ))
        .await
        .map_err(|error| error.to_string())?;

    while let Some(message) = read.next().await {
        let text = message.map_err(|error| error.to_string())?.into_text().map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        if value.get("type").and_then(Value::as_str) == Some("connection_ack") {
            break;
        }
    }

    let subscribe = serde_json::json!({
        "id": "conversation-events",
        "type": "subscribe",
        "payload": {
            "query": CONVERSATION_EVENTS_QUERY,
            "variables": { "conversationId": conversation_id }
        }
    });
    write
        .send(Message::Text(subscribe.to_string()))
        .await
        .map_err(|error| error.to_string())?;

    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(message) = read.next().await {
            let result = match message {
                Ok(message) => match message.into_text() {
                    Ok(text) => serde_json::from_str::<Value>(&text)
                        .map_err(|error| error.to_string())
                        .and_then(|value| {
                            if value.get("type").and_then(Value::as_str) == Some("next") {
                                Ok(value["payload"]["data"].clone())
                            } else if value.get("type").and_then(Value::as_str) == Some("error") {
                                Err(value["payload"].to_string())
                            } else {
                                Ok(Value::Null)
                            }
                        }),
                    Err(error) => Err(error.to_string()),
                },
                Err(error) => Err(error.to_string()),
            };
            if !matches!(result, Ok(Value::Null)) {
                let _ = tx.send(result);
            }
        }
    });

    Ok(rx)
}
```

Add the query string next to the helper:

```rust
const CONVERSATION_EVENTS_QUERY: &str = r#"
subscription ConversationEvents($conversationId: String!) {
  conversationEvents(conversationId: $conversationId) {
    __typename
    ... on GraphqlConversationItemEvent {
      conversationId
      clientMessageId
      itemId
      turnId
      item {
        __typename
        ... on GraphqlUserText { text }
        ... on GraphqlAssistantText { text }
        ... on GraphqlActivity { id activityKind status title summary metadata }
        ... on GraphqlA2uiCard { id schema payload }
        ... on GraphqlErrorNotice { message recoverable }
      }
    }
    ... on GraphqlAgentStatusEvent {
      conversationId
      status
    }
    ... on GraphqlTurnCompletedEvent {
      conversationId
      clientMessageId
    }
  }
}
"#;
```

- [ ] **Step 3: Replace temporary daemon streaming path**

In `run_chat`, subscribe to conversation events after `startPrimaryConversation`. Print transcript events using the existing `print_transcript_item` helpers.

- [ ] **Step 4: Run CLI chat smoke test**

Run with a local daemon:

```bash
cargo run -p noema-cli -- chat "Say hello in one sentence."
```

Expected: CLI starts or connects to Noema, sends the turn through GraphQL, streams assistant output through GraphQL subscription, and exits cleanly for one-shot chat.

## Task 11: Retire Legacy Client-Facing Product Endpoints

**Files:**
- Modify: `crates/noema-core/src/daemon/web/mod.rs`
- Modify: `crates/noema-core/src/frontend_protocol.rs`
- Modify: `crates/noema-core/src/bin/export_frontend_types.rs`
- Modify: `crates/noema-core/web/src/generated/noema.ts`
- Test: `crates/noema-core/src/daemon/web/mod.rs`

- [ ] **Step 1: Add negative route tests**

Add tests:

```rust
#[test]
fn legacy_chat_ws_is_no_longer_client_product_api() {
    assert!(!is_supported_product_route("GET", "/api/chat/ws"));
}

#[test]
fn legacy_status_is_no_longer_client_product_api() {
    assert!(!is_supported_product_route("GET", "/api/status"));
}
```

- [ ] **Step 2: Remove or internalize legacy routes**

Remove these client-facing branches from `handle_connection` after web/CLI are migrated:

- `GET /api/chat/ws`
- `GET /api/status`
- `GET /api/onboarding/status`
- `GET /api/provider-accounts`
- `POST /api/provider-auth/attempts`
- `GET /api/provider-auth/attempts/:id`
- `POST /api/provider-auth/attempts/:id/cancel`

Keep:

- `POST /graphql`
- `GET /graphql/ws`
- `GET /graphql/schema.graphql`
- static asset routes
- future provider callback routes when needed

- [ ] **Step 3: Remove old frontend protocol type export**

Keep Rust daemon protocol types that the local daemon socket still needs. Remove web-only `WebClientMessage`, `WebServerMessage`, and `write_frontend_typescript` exports after no Rust code references them.

- [ ] **Step 4: Run reference search**

Run:

```bash
rg -n "/api/chat/ws|/api/status|WebClientMessage|WebServerMessage|generated/noema" crates/noema-core crates/noema-cli docs
```

Expected: no remaining product-client references. If docs mention the old routes as historical/transitional, update them to GraphQL.

## Task 12: Documentation And Current Context Update

**Files:**
- Modify: `docs/frontend/current-contract.md`
- Modify: `docs/frontend/README.md`
- Modify: `docs/context/current.md`
- Modify: `docs/superpowers/specs/2026-06-27-graphql-client-api-design.md` if implementation discovers a design correction

- [ ] **Step 1: Update frontend contract**

Replace current web protocol wording with:

```markdown
The current web frontend uses Noema's GraphQL client API. Queries provide
scoped read models, mutations execute explicit Noema commands, and
subscriptions stream conversation and activity events. Static assets are served
over ordinary HTTP; product state and product actions go through GraphQL.
```

- [ ] **Step 2: Update current context**

Add to `docs/context/current.md` under Settled Decisions:

```markdown
- GraphQL is the first-party client API for Noema web, CLI, future desktop, and
  future mobile clients. Internal Rust modules continue to use command,
  runtime, repository, policy, provenance, audit, and event interfaces directly.
```

- [ ] **Step 3: Run docs search**

Run:

```bash
rg -n "native `/api/chat/ws`|/api/status|WebSocket served by the daemon|WebClientMessage|WebServerMessage" docs crates/noema-core/web/src
```

Expected: no stale current-contract claims remain.

## Task 13: Full Validation

**Files:**
- No planned file edits.

- [ ] **Step 1: Check worktree**

Run:

```bash
git status --short --branch
```

Expected: only files from this GraphQL migration are changed, plus generated GraphQL files and lockfiles.

- [ ] **Step 2: Rust formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS.

- [ ] **Step 3: Rust check**

Run:

```bash
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 4: Rust clippy**

Run:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Rust tests**

Run:

```bash
cargo test --workspace --no-fail-fast
```

Expected: PASS. If tests that bind local sockets fail with sandbox `PermissionDenied`, rerun the same command with socket permissions and report the distinction.

- [ ] **Step 6: Web validation**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
```

Expected: PASS.

- [ ] **Step 7: Diff check**

Run:

```bash
git diff --check
```

Expected: PASS.

- [ ] **Step 8: Final status**

Run:

```bash
git status --short --branch
```

Expected: report all modified, generated, and untracked files.

- [ ] **Step 9: Commit checkpoint**

Commit only if explicitly requested by the user or if the execution mode is `ship`.
