# Custom Reasoning Model Config Design

## Goal

Noema should let users set custom reasoning effort anywhere they set a custom
model, while preserving provider-owned defaults when they do not customize a
model.

The generated default config should not write a model id. Provider setup,
provider account metadata, and provider adapters own default model/profile
selection.

## Current State

Noema currently has three store-backed model preference surfaces:

- Primary agent runtime preference.
- `web.fetch` summarizer auxiliary preference.
- Tool progress audit auxiliary preference.

Each persisted preference stores only provider kind, provider account id, and
model profile. Runtime requests pass that profile as `GenerateRequest.model`.
`GenerateOptions` has no reasoning field, so OpenAI and Codex Responses calls
always rely on provider/model defaults for reasoning effort.

The generated `config.yaml` currently includes:

```yaml
codex:
  model: gpt-5.5
```

That makes Noema's starter config look like an explicit model override even
though the intended behavior is provider-owned defaults.

## External API Facts

OpenAI Responses supports `reasoning: { effort }` for reasoning-capable models.
The API reference documents the current effort values as `none`, `minimal`,
`low`, `medium`, `high`, and `xhigh`, with supported values and defaults varying
by model. The same docs state that reducing effort can reduce latency and
reasoning-token usage.

OpenAI also documents encrypted reasoning items as a separate stateless
Responses concern: when using `store: false`, requests must include
`reasoning.encrypted_content` if the caller wants encrypted reasoning output
items that can be replayed on later turns.

Sources:

- <https://platform.openai.com/docs/api-reference/responses/create>
- <https://platform.openai.com/docs/guides/reasoning>

OpenAI Responses and Codex Responses must be verified independently. Noema must
not assume Codex accepts a field only because OpenAI does.

## Product Rule

Noema model configs follow an override-only rule:

1. If no custom model/profile is saved or configured, Noema sends no
   `reasoning_effort` and no explicit model override. The provider owns both the
   default model/profile and default reasoning behavior.
2. If a custom model/profile is saved or configured and that selected
   provider/profile advertises reasoning effort support, the config must also
   include a custom reasoning effort.
3. If a custom model/profile is saved or configured and that selected
   provider/profile does not advertise reasoning effort support, no reasoning
   effort is required or sent.

This rule applies to every model configuration surface:

- Primary agent model preference.
- Web fetch summarizer preference.
- Tool progress audit preference.
- Explicit file-based provider model override.

## Non-Goals

- Do not add a global default reasoning level.
- Do not infer reasoning support or defaults from English model id matching.
- Do not expose raw or summarized reasoning in the UI.
- Do not change encrypted reasoning persistence or replay semantics except to
  keep them compatible with explicit effort.
- Do not add compatibility migrations; this is pre-V1 and schema changes may
  rewrite table definitions directly.

## Data Model

Introduce a provider-neutral reasoning effort enum:

```rust
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}
```

The serialized store/API values should be snake-compatible lowercase strings:

```text
none | minimal | low | medium | high | xhigh
```

Extend model preference records with:

```rust
reasoning_effort: Option<ReasoningEffort>
```

This field belongs beside `model_profile` in:

- `agent_runtime_preferences`
- `auxiliary_model_preferences`

The field means "the user's explicit effort for this explicit model/profile
selection." It is not a default and should be absent when no explicit
model/profile preference exists.

## Provider Metadata

Provider account profile metadata should expose reasoning support per profile:

```json
{
  "profiles": [
    {
      "id": "gpt-5.5",
      "label": "GPT-5.5",
      "reasoning_efforts": ["minimal", "low", "medium", "high", "xhigh"],
      "default_reasoning_effort": "medium"
    }
  ]
}
```

`reasoning_efforts` is the authoritative product/runtime signal. If it is empty
or missing, the profile is treated as not supporting configurable reasoning.

`default_reasoning_effort` is display-only guidance. It must not be used to
silently fill a saved custom preference. When a user saves a reasoning-capable
custom profile, the saved preference must include one of the advertised efforts.

Provider profile refresh is responsible for populating this metadata when a
provider can report it. For static or test metadata, fixtures may include it
directly. For providers that cannot report support precisely, Noema should
prefer no configurable reasoning support over guessing.

## Config File

The generated default config should omit provider model fields:

```yaml
provider: codex

codex:
  base_url: https://chatgpt.com/backend-api/codex
  timeout_seconds: 300
```

Config parsing should continue accepting existing explicit model override
shapes, but explicit overrides become complete model configs. The current
portable shape is the top-level selected-provider model:

```yaml
provider: codex
model: gpt-5.5
reasoning_effort: medium
```

The current Codex-specific model override should also stay supported:

```yaml
provider: codex
codex:
  model: gpt-5.5
  reasoning_effort: medium
```

Do not introduce a new `openai.model` shape as part of this work unless the
config system is deliberately redesigned; OpenAI currently uses the top-level
`model` field.

Validation rules:

- Explicit `model` without `reasoning_effort` is invalid when the configured
  provider/model support configurable reasoning.
- Explicit `reasoning_effort` without explicit `model` is invalid because there
  is no custom model config to attach it to.
- Unsupported effort strings are invalid.
- Providers without configurable reasoning reject `reasoning_effort`.

The validation may initially be capability-based rather than fully
profile-specific for file config, because file config is loaded before dynamic
account metadata refresh. If exact model support is not available at config load
time, defer profile-specific validation to provider construction or first use,
but keep the user-facing rule explicit.

## Provider Contract

Extend `GenerateOptions`:

```rust
pub struct GenerateOptions {
    pub reasoning_effort: Option<ReasoningEffort>,
    ...
}
```

Provider adapters should serialize the field only when set and supported.

OpenAI Responses:

```json
"reasoning": { "effort": "medium" }
```

Codex Responses:

- Add a provider capability flag only after verifying the Codex API accepts the
  same request shape.
- Until verified, do not send the field to Codex.
- If Codex support is unknown, Codex profiles should not advertise configurable
  reasoning efforts in product metadata.

Foundation Local:

- Leave unset unless a Foundation-native equivalent is introduced.

Encrypted reasoning remains separate. Providers that support encrypted
reasoning should continue requesting/replaying encrypted reasoning items even
when `reasoning_effort` is unset, because provider defaults may still produce
reasoning items.

## Runtime Flow

Primary agent:

1. Conversation hydration resolves provider/model selection from the primary
   agent preference.
2. If no preference exists, runtime uses the default provider and sends no
   explicit model or reasoning effort.
3. If a preference exists, runtime sends both the saved `model_profile` and the
   saved `reasoning_effort` when present.
4. GraphQL validation prevents saving a reasoning-capable custom profile without
   an effort.

Web fetch summarizer:

1. Without a saved preference, use the provider-owned default summarizer model
   path and send no user-configured reasoning effort.
2. With a saved preference, pass both model profile and reasoning effort to the
   summarizer generation request.

Tool progress audit:

1. Without a saved preference, use the provider's default tool-classification
   model and send no user-configured reasoning effort.
2. With a saved preference, pass both model profile and reasoning effort to the
   audit request.

Context compaction:

- Compaction requests should use the active conversation's resolved model
  config. If an explicit primary-agent model preference carries reasoning
  effort, compaction requests for that conversation should carry the same
  effort. If no explicit preference exists, compaction sends no effort.

## GraphQL API

Extend shared model preference output:

```graphql
type AgentModelPreference {
  providerKind: String!
  providerAccountId: String!
  modelProfile: String!
  reasoningEffort: ReasoningEffort
}
```

Add:

```graphql
enum ReasoningEffort {
  NONE
  MINIMAL
  LOW
  MEDIUM
  HIGH
  XHIGH
}
```

Extend profile options:

```graphql
type AgentModelProfileOption {
  id: String!
  label: String!
  disabledReason: String
  reasoningEfforts: [ReasoningEffort!]!
  defaultReasoningEffort: ReasoningEffort
}
```

Extend save inputs for all model preference mutations:

```graphql
reasoningEffort: ReasoningEffort
```

Validation:

- If the selected profile has non-empty `reasoningEfforts`,
  `reasoningEffort` is required and must be in that list.
- If the selected profile has empty `reasoningEfforts`, `reasoningEffort` must
  be null.

## Frontend Settings

`ModelPreferenceSelect` should become a small model-config editor rather than a
single select-only control.

Behavior:

- Model dropdown remains the first control.
- If the selected profile advertises reasoning efforts, show a required
  reasoning dropdown beside or below it.
- When a user changes to a reasoning-capable profile, do not autosave until a
  reasoning level is selected.
- The dropdown may preselect the profile's `defaultReasoningEffort` as a UI
  convenience, but saving still writes an explicit value. This avoids treating
  provider defaults as Noema defaults while keeping the interaction efficient.
- If the selected profile has no reasoning support, hide or disable the
  reasoning dropdown and save `reasoningEffort: null`.

Copy should make the rule clear without overexplaining:

```text
Reasoning
Required for this custom model.
```

No UI is needed for "default provider model" because no saved model preference
exists in that state.

## Prompt Cache Implications

Changing model profile or reasoning effort is a model-config change and may
reset prompt-cache effectiveness. That is acceptable. New user messages should
still have stable-prefix cache behavior when provider, model profile, reasoning
effort, prompt contract, tools, and compaction state are unchanged.

Reasoning effort must not be injected into prompt text. It belongs only in
provider request options.

## Testing

Backend tests:

- Generated default config omits `codex.model`.
- Config validation rejects `reasoning_effort` without `model`.
- Config validation rejects unsupported effort strings.
- Store round-trips `reasoning_effort` for agent runtime preferences.
- Store round-trips `reasoning_effort` for auxiliary model preferences.
- GraphQL exposes profile reasoning efforts from provider metadata.
- GraphQL rejects reasoning-capable model saves without reasoning effort.
- GraphQL rejects reasoning effort for non-reasoning profiles.
- GraphQL persists valid reasoning effort for agent, web fetch summarizer, and
  progress audit preferences.
- Runtime sends saved reasoning effort on primary agent requests.
- Runtime sends saved reasoning effort on web fetch summarizer requests.
- Runtime sends saved reasoning effort on progress audit requests.
- Runtime sends no reasoning effort when no explicit preference exists.
- OpenAI Responses serializes `reasoning: { effort }` when set.
- Codex has an independent test proving either "field is sent only when verified
  supported" or "field is not sent while unsupported."

Frontend tests:

- Model config editor requires reasoning for reasoning-capable profiles.
- Model config editor omits reasoning for non-reasoning profiles.
- Save input includes reasoning effort when required.
- Generated GraphQL types include the new enum and fields.

## Rollout

1. Add provider-neutral `ReasoningEffort` and request serialization support.
2. Extend provider profile metadata parsing and GraphQL profile options.
3. Extend store preference records and schema definitions.
4. Extend GraphQL model preference inputs/outputs and validation.
5. Thread reasoning effort through runtime request construction.
6. Update generated default config to omit provider model.
7. Update Settings model config UI.
8. Verify OpenAI and Codex adapters independently.
