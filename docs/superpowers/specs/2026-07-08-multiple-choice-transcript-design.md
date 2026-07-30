# Multiple Choice Transcript Design

## Goal

Support first-class multiple-choice agent responses in chat, including durable
human selections, without treating choices as ordinary assistant text or A2UI
cards.

## Context

Noema's provider response contract currently returns `responses[]` items as
assistant text or generic structured payloads. Structured payloads persist as
`A2UICard` transcript items, which should remain reserved for richer app/card
surfaces. Multiple-choice prompts are a narrower conversation primitive: the
agent is asking the human to choose from explicit options, and the human's
selection is itself a transcript item that can drive the next turn.

The existing durable chat path already separates provider response parsing,
conversation item persistence, GraphQL transcript unions, and web rendering.
Multiple-choice should follow that path as its own semantic item type instead
of hiding intent in text, metadata, or A2UI schema names.

## Non-Goals

- Do not implement arbitrary A2UI or form rendering.
- Do not add migrations or compatibility layers for pre-V1 transcript schemas.
- Do not infer user intent from English option text.
- Do not add UI tests in this slice.
- Do not support nested option groups, free-form "other" answers, ranking, or
  weighted choices yet.

## Response Contract

Add a provider response item variant for multiple-choice prompts:

```json
{
  "kind": "multiple_choice",
  "phase": "final_answer",
  "prompt": "Which direction should we take?",
  "selection_mode": "pick_one",
  "options": [
    { "id": "ship_now", "label": "Ship now" },
    { "id": "polish_first", "label": "Polish first" }
  ]
}
```

Fields:

- `phase`: assistant phase, using the same `commentary` and `final_answer`
  semantics as text.
- `prompt`: user-visible question or instruction.
- `selection_mode`: `pick_one` or `pick_many`.
- `options`: ordered stable choices. Each option has an `id` and `label`.
  Option ids are the durable semantic selection values. Labels are display copy.

Validation rules:

- `prompt` must be non-empty.
- `selection_mode` must be `pick_one` or `pick_many`.
- `options` must contain at least two entries.
- option ids must be non-empty and unique within the prompt.
- option labels must be non-empty.
- final responses may contain text and multiple-choice items.
- `needs_tools` responses must not contain multiple-choice items in the initial
  implementation. The agent should ask for a choice only after required tool
  results are available.

The OpenAI/Codex structured-output JSON schema should include this new response
item variant so model outputs can be validated at the provider boundary.

## Durable Transcript Model

Add two conversation item kinds:

- `MultipleChoicePrompt`: authored by the assistant.
- `MultipleChoiceSelection`: authored by the human.

The assistant prompt item stores:

```json
{
  "prompt": "Which direction should we take?",
  "selection_mode": "pick_many",
  "options": [
    { "id": "backend", "label": "Backend" },
    { "id": "frontend", "label": "Frontend" }
  ]
}
```

The human selection item stores:

```json
{
  "prompt_item_id": "item_prompt_123",
  "selection_mode": "pick_many",
  "selected_options": [
    { "id": "backend", "label": "Backend" },
    { "id": "frontend", "label": "Frontend" }
  ]
}
```

`content_text` for the prompt may be the prompt string for search and compact
inspection. `content_text` for the selection may be a concise deterministic
rendering such as `Backend, Frontend`; the structured payload remains the
authority for model context and UI replay.

The selection item should use the prompt item as its parent item when possible.
The next agent turn is triggered by the selection item, just as a user text item
triggers a normal turn.

## GraphQL Contract

Extend the `TranscriptItem` union with:

```graphql
type MultipleChoicePrompt {
  prompt: String!
  selectionMode: MultipleChoiceSelectionMode!
  options: [MultipleChoiceOption!]!
}

type MultipleChoiceSelection {
  promptItemId: String!
  selectionMode: MultipleChoiceSelectionMode!
  selectedOptions: [MultipleChoiceOption!]!
}

type MultipleChoiceOption {
  id: String!
  label: String!
}

enum MultipleChoiceSelectionMode {
  PICK_ONE
  PICK_MANY
}
```

Conversation replay and live events use the same union members. The frontend
does not parse prompt/selection payload JSON directly once GraphQL exposes these
typed items.

## Frontend Behavior

Assistant multiple-choice prompts render as transcript controls in the assistant
lane.

For `pick_one`:

- clicking an option immediately submits one `MultipleChoiceSelection`;
- the prompt becomes disabled once its selection is present in the transcript;
- replay shows the settled prompt and the user's selection.

For `pick_many`:

- clicking options toggles local checked state;
- a `Done` action submits one `MultipleChoiceSelection` containing all selected
  options;
- `Done` is disabled until at least one option is selected;
- the prompt becomes disabled once its selection is present in the transcript;
- replay shows the settled prompt and the user's selected set.

The frontend should derive settled state by finding a later
`MultipleChoiceSelection` with the prompt item id. This keeps state durable and
works across reloads, live events, and paged replay.

## Sending Selections

Add a GraphQL mutation for selecting options instead of overloading text input:

```graphql
input SendMultipleChoiceSelectionInput {
  conversationId: String!
  promptItemId: String!
  selectedOptionIds: [String!]!
  clientMessageId: String
}
```

The backend validates that:

- the prompt item exists in the same conversation;
- the prompt item is a multiple-choice prompt;
- the prompt has no prior visible selection;
- selected ids are present in the prompt options;
- `pick_one` receives exactly one selected id;
- `pick_many` receives at least one selected id.

After validation, the backend appends a `MultipleChoiceSelection` user item and
starts the next turn with model-visible context that includes the structured
selection. The provider should see enough context to know which prompt the
selection answered without relying on English phrase matching.

## Model Context

Prompt context assembly should include both prompt and selection items as typed
conversation history. Providers with structured item replay should receive a
compact JSON representation. Text-only fallback providers may receive a stable
rendering such as:

```text
assistant multiple_choice: Which direction should we take?
options: ship_now=Ship now; polish_first=Polish first
user selected: polish_first=Polish first
```

The structured payload remains authoritative. The fallback text is only for
providers that cannot replay typed items directly.

## Error Handling

Invalid provider multiple-choice payloads are rejected as malformed provider
responses at the provider parser boundary.

Invalid user selections return GraphQL errors and do not append transcript
items. Duplicate selections should be rejected with a clear already-selected
error. If a live prompt and replay race, frontend deduplication by durable item
id remains the ordering authority.

If the agent emits a multiple-choice prompt but the frontend does not yet
support it, GraphQL replay should still expose typed data and the web fallback
can render a compact unsupported-card notice during development. Once the web
renderer lands, no generic A2UI fallback is needed.

## Testing

Rust tests:

- provider contract parses and validates `multiple_choice` response items;
- invalid multiple-choice payloads are rejected;
- persistence maps provider multiple-choice items to durable prompt items;
- replay maps durable prompt and selection items into typed transcript items;
- selection mutation validates prompt existence, option ids, selection mode, and
  duplicate selections;
- prompt context includes typed prompt and selection history.

Frontend validation:

- type generation and frontend build cover the GraphQL union changes;
- manual interaction checks cover immediate submit for `pick_one`;
- manual interaction checks cover toggle plus `Done` submit for `pick_many`.
