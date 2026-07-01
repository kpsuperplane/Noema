# Apple Foundation Local Provider Design

## Status

Approved design for adding Apple Foundation Models as a supported Noema
provider. No implementation has been done in this spec. The next step is an
implementation plan.

## Context

Noema currently has a provider-neutral generation contract, provider account
metadata, provider authentication state, GraphQL provider listing, and concrete
Codex/OpenAI adapters. The daemon runtime still has Codex-shaped naming in a few
places, but internally it already uses a provider-neutral runtime trait for
generation.

Apple's macOS 27 Foundation Models framework exposes a native Swift API for the
on-device Apple Intelligence model. Apple's macOS 27 overview also describes
continuous sessions, Dynamic Profiles, multimodal prompts, and support for model
providers that conform to the Foundation Models language model protocol. The
`fm` command line tool is useful for scripting, but the desired Noema behavior
requires a stateful session that can resume from Noema-owned transcript state.

## Goals

- Add an Apple Foundation Models provider for local, on-device generation.
- Use a stateful Apple Foundation Models session while the daemon is running.
- Resume conversations after restart by replaying Noema's persisted transcript
  into a fresh Apple session.
- Keep the Rust daemon responsible for starting, monitoring, restarting, and
  stopping the Swift bridge process.
- Keep Noema buildable and deployable on Linux, Windows, and non-macOS targets
  by omitting Apple Foundation Models runtime support on those platforms.
- Surface provider availability through existing provider account, onboarding,
  and Settings paths.
- Let the user choose which provider and model/profile each agent uses from the
  web Settings dashboard.
- Keep Noema's structured store as the source of truth for conversations,
  transcript items, memory, and provider account metadata.

## Non-Goals

- Do not use Private Cloud Compute in this slice.
- Do not make `fm respond` the primary generation path.
- Do not require the user to manually start or stop a Swift bridge.
- Do not persist or rely on a serialized Apple session object.
- Do not add Apple-native tool calling, approvals, or memory writes in V1.
- Do not make Swift, Xcode, or Foundation Models requirements leak into Linux or
  Windows builds.
- Do not put agent model selection controls in Provider Settings. Provider
  Settings remains about backend/account availability.
- Do not add backwards compatibility layers for pre-V1 provider schema changes.

## Product Decisions

- The provider kind is `foundation_local`.
- The provider account uses `auth_method = none`.
- Local provider readiness is derived from platform support, bridge binary
  availability, bridge handshake success, and Foundation Models availability.
- The bridge is an implementation detail owned by the daemon. Users configure a
  provider, not a helper process.
- Conversation continuity remains owned by Noema structured state. The bridge
  owns live Apple session objects only while the current daemon process runs.
- Provider accounts answer whether a backend can be used. Agent runtime
  preferences answer which backend and model/profile an agent uses.
- Apple Foundation Models should expose user-facing model profiles, not raw
  framework internals. The first profile is `default`.
- Agent model selection belongs in Settings -> Agents, on each agent's settings
  row or detail panel.
- On unsupported platforms, `foundation_local` is allowed in portable metadata
  and config code, but runtime initialization returns a clean unavailable
  status.

## Architecture

Add a portable Rust `foundation_local` provider surface and a macOS-only Swift
bridge executable.

Portable Rust owns:

- provider kind parsing and config
- provider account metadata
- agent runtime preference storage/read models
- GraphQL provider listing and onboarding status
- bridge protocol types
- transcript replay shaping
- non-macOS unavailable stubs
- tests with a fake bridge process

macOS-only code owns:

- locating or launching the Swift bridge executable
- child process lifecycle management
- JSON protocol transport over the child process stdio pipes
- bridge handshake and health checks
- session creation, replay, generation, cancellation, and close calls

The Swift bridge owns Apple `LanguageModelSession` instances, keyed by Noema
conversation ids or runtime session ids supplied by Rust. The bridge does not
own durable state. If it exits, Noema can create a new bridge process and replay
the conversation into a fresh session.

## Cross-Platform Boundary

Noema must compile on every supported Rust target. Apple-specific imports,
Swift build steps, Foundation Models symbols, and bridge packaging must be
guarded so they only apply on macOS targets that can support the framework.

The Rust module shape should expose the same adapter-facing API everywhere:

- On macOS, the adapter can launch and use the Swift bridge.
- On non-macOS targets, the adapter returns `ProviderAccountStatus::Unavailable`
  or a provider error with a stable unsupported-platform code.

This boundary keeps Linux and Windows CI focused on portable Rust validation and
prevents optional Apple support from becoming a global build dependency.

## Daemon-Owned Bridge Lifecycle

When `foundation_local` is the selected provider, the Rust daemon starts the
bridge during provider initialization. Startup performs:

1. Resolve config and provider account metadata.
2. Verify the current platform can attempt Foundation Models support.
3. Locate the bundled or configured bridge binary.
4. Spawn the bridge as a daemon-owned child process.
5. Run a versioned handshake.
6. Request a health/capability check from the bridge.
7. Mark the provider available or unavailable from the sanitized result.

The daemon stops the bridge on normal shutdown and cancels in-flight requests
when conversations end or the daemon exits. If the bridge crashes, the adapter
may restart it for recoverable failures, then rebuild live sessions by replaying
Noema transcript state. Restart attempts should be bounded so repeated crashes
become a clear provider-unavailable state.

## Bridge Protocol

Use a small versioned JSON message protocol over child process stdio. The first
contract should include:

```text
handshake
health
create_session
replay_turns
generate
cancel
close_session
shutdown
```

Messages should carry stable request ids so Rust can match responses to
in-flight operations. The protocol should support assistant text deltas if the
Foundation Models API exposes streaming for the chosen call path, plus a final
response message containing the full assistant text and any safe usage metadata
available from Apple.

The bridge protocol is not a general local RPC surface. It should expose only
the operations needed to manage Foundation Models sessions for Noema.

## Conversation Resume

Noema does not persist Apple session internals. To resume a conversation after
daemon restart:

1. Load the persisted Noema conversation and transcript items.
2. Convert the transcript into a provider-replayable sequence.
3. Start or reuse the bridge.
4. Create a fresh bridge session with current system/developer instructions.
5. Replay eligible prior user and assistant turns.
6. Send the new user turn.
7. Persist the assistant response through the existing Noema conversation item
   path.

Replay shaping should remain explicit and conservative. Tool markers, approval
events, memory markers, and other Noema-specific transcript items should be
converted only when the provider should actually see them. Otherwise they remain
Noema-owned state and are omitted or summarized through an explicit future
policy.

## Components

### Provider Config

Add `foundation_local` to provider kind parsing and raw config resolution. The
provider-level config should include an optional bridge path override and
default profile catalog settings. It should not require secrets.

Example shape:

```yaml
provider: foundation_local
foundation_local:
  default_profile: default
```

This config is a backend default, not the long-term source of per-agent model
selection. The active provider/profile for chat should come from the selected
agent's runtime preferences when they exist.

### Provider Accounts

Allow `foundation_local` in provider account storage. The default account is:

```text
provider_account:foundation_local:default
provider_kind = foundation_local
account_key = default
display_name = Apple Foundation Models
auth_method = none
```

The status starts as `unknown` and is refreshed by runtime readiness checks.

### Agent Runtime Preferences

Add an agent-owned runtime preference model so each agent can choose a provider
account and model/profile independently of provider account setup. The minimal
shape is:

```text
agent_id
provider_kind
provider_account_id
model_profile
updated_at
```

For OpenAI or Codex, `model_profile` can hold the model id already used by the
provider. For `foundation_local`, `model_profile` is a stable profile id
reported by the bridge capability handshake, starting with `default`.

Runtime provider selection should resolve in this order:

1. Conversation-level temporary override, when a future UI adds one.
2. Agent runtime preference.
3. System default provider config.

The implementation should start with the primary agent but keep the data model
agent-scoped so additional agents do not require a schema redesign.

### Runtime Adapter

Add a Foundation Local adapter that implements Noema's provider-neutral
generation contract. It owns the bridge lifecycle manager and maps provider
errors into existing `ProviderError` variants with stable messages.

The current runtime can keep its provider-neutral trait, but Codex-specific
runtime naming should be cleaned up where it blocks selecting a non-Codex
provider for daemon chat.

### Swift Bridge

The bridge should be a narrow executable with one purpose: provide stateful
access to Apple Foundation Models sessions on macOS. It handles session objects,
Foundation Models calls, cancellation, and availability checks. It does not
write Noema state and does not decide policy.

### GraphQL And UI

Provider listing should return all active default provider accounts instead of
hard-coding Codex only. Settings should show Apple Foundation Models as a local
provider with no auth action. On unsupported platforms, Settings should show a
clear unavailable status and a safe reason.

GraphQL should also expose a Settings-focused agent model preference read/write
surface. The read model should include each agent's current provider kind,
provider account id, selected model/profile, available compatible provider
accounts, available model/profile options for each provider, and availability
state. The mutation should update only the selected agent's runtime preference
after validating that the provider account exists and the selected model/profile
is known or explicitly allowed for that provider.

## Web Dashboard UI

The web Settings dashboard should make model choice an agent setting, not a
provider setting.

### Providers

The Providers section should remain an infrastructure/status surface:

- list configured provider accounts
- show provider kind, display name, auth method, status, active/default flags,
  and safe last error metadata
- show Apple Foundation Models as a local provider with `auth_method = none`
- show clear unavailable states for unsupported platform, bridge missing, or
  Foundation Models unavailable
- do not offer a model/profile picker here

### Agents

The Agents section should become the user's primary model-selection surface.
Each agent row or detail panel should show:

- agent display name and id
- current provider display name, such as `Apple Foundation Models`, `Codex`, or
  `OpenAI`
- current model/profile label, such as `Default on-device`
- provider availability status
- an edit control for provider/model selection

The edit control should be a compact form inside the Agents settings surface:

1. Provider/account select.
2. Model/profile select whose options update for the selected provider.
3. Save and Cancel actions.

Expected option behavior:

- Codex/OpenAI options show configured or known model ids.
- Apple Foundation Models options show bridge-reported profile labels, starting
  with `Default on-device`.
- Unavailable provider accounts remain visible but disabled, with a concise
  reason.
- If the currently selected provider/profile becomes unavailable, the current
  choice remains visible with a warning so the user understands why chat is
  blocked or degraded.

The Agents section should not expose provider secrets, credential paths, raw
bridge stderr, prompt internals, memory internals, or tool grants. The UI copy
should describe the selection as the model this agent uses for chat, while
leaving provider setup and backend health details in Providers.

## Data Flow

Fresh conversation:

```text
Noema chat turn
  -> runtime resolves the agent's provider/model preference
  -> selected provider is foundation_local
  -> Rust adapter starts or reuses bridge
  -> create_session
  -> generate(user turn)
  -> assistant deltas/final response
  -> existing Noema persistence path
```

Resumed conversation:

```text
Noema chat turn after restart
  -> runtime resolves the agent's provider/model preference
  -> load conversation transcript
  -> start bridge
  -> create_session
  -> replay_turns(history)
  -> generate(new user turn)
  -> assistant deltas/final response
  -> existing Noema persistence path
```

Bridge crash recovery:

```text
generation or health detects bridge exit
  -> mark in-flight request failed or retry if safe
  -> restart bridge within bounded policy
  -> recreate sessions from Noema transcript replay
  -> continue or surface provider unavailable
```

## Error Handling

Errors should use stable internal codes and sanitized user-facing messages:

- `unsupported_platform`
- `bridge_missing`
- `bridge_launch_failed`
- `bridge_handshake_failed`
- `bridge_protocol_mismatch`
- `foundation_models_unavailable`
- `session_create_failed`
- `session_replay_failed`
- `generation_failed`
- `bridge_crashed`
- `bridge_restart_exhausted`

Raw stderr, Apple framework diagnostics, and local paths should stay in logs,
not GraphQL or web UI responses.

## Testing

Portable Rust tests should cover:

- `foundation_local` config parsing and provider kind round trips.
- Provider account schema/storage accepts the new provider kind.
- Agent runtime preference storage validates provider account and model/profile
  selections.
- Provider listing returns the configured active default provider accounts.
- Agent settings GraphQL exposes current provider/model preference and allowed
  provider/profile options.
- Agent settings mutation updates one agent's model preference and rejects
  unknown provider accounts or invalid profile ids.
- Non-macOS runtime stubs return unavailable without requiring Swift.
- Bridge protocol serialization and parsing.
- Replay shaping from Noema transcript items.
- Lifecycle manager startup, health, cancellation, crash, bounded restart, and
  shutdown behavior using a fake bridge process.
- Onboarding and Settings read models for unavailable local providers.

Frontend tests should cover:

- Providers Settings shows Apple Foundation Models as a local provider with no
  auth action.
- Providers Settings does not render model/profile controls.
- Agents Settings shows each agent's current provider and model/profile.
- Agents Settings lets the user open an edit control with provider/account and
  model/profile selects.
- Changing provider updates the model/profile options.
- Unavailable providers/profiles render disabled options or warnings.
- Saving calls the agent model preference mutation and updates the displayed
  current choice.
- The UI does not render provider secrets, credential paths, raw bridge stderr,
  prompt internals, memory internals, or tool grants.

macOS-only tests should be opt-in and skipped by default:

- Swift bridge builds on a supported macOS/Xcode toolchain.
- Bridge handshake and health check work against Foundation Models.
- A minimal session can generate text.
- Restart plus replay reconstructs enough context for a continued conversation.

Default validation should continue to pass on Linux and Windows without Apple
Foundation Models support.

## Sources

- Apple Foundation Models documentation:
  <https://developer.apple.com/documentation/FoundationModels>
- macOS 27 Foundation Models overview:
  <https://developer.apple.com/macos/whats-new/>
