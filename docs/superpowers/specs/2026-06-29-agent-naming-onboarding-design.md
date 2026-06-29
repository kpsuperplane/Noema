# Agent Naming Onboarding Design

## Goal

Fresh agents should feel like distinct beings from the beginning instead of
booting with a borrowed product label. Every newly created agent starts without
a canonical name, asks the user to name it through its own generated voice, and
can later update its own name only when the user explicitly instructs it to do
so.

The first slice should solve naming while introducing a lightweight
`onboarding_prompt` apparatus that can later ask for other setup information
when Noema supports it.

## Decisions

- Agents are born unnamed. `agents.display_name` is optional canonical state.
- Noema does not seed the primary agent as `Noema`.
- UI and transcript surfaces may render neutral fallback labels such as
  `Unnamed agent` when a label is required, but fallbacks are not written to
  canonical agent state.
- The model generates the first naming request. Noema supplies the prompt
  condition that the agent has no name and should ask the user to give it one.
- The rename tool remains available after onboarding, but Noema only honors it
  when the user explicitly names or renames the current agent.
- The design applies to any newly created agent, even if implementation begins
  with the existing primary agent creation path.

## Recommended Approach

Use an agent-lifecycle-centered design.

Agent identity belongs in the `agents` table and in small store/runtime APIs
that create, read, and update agent records. Conversation prompts should include
the current agent identity state. When that state is incomplete, the prompt
builder emits an `onboarding_prompt` block that describes what the agent should
ask for next.

This keeps naming out of memory inference, avoids primary-agent special cases,
and gives future onboarding questions a single place to live.

Alternatives considered:

- Primary-chat special case: faster for the current product surface, but it
  hard-codes `agent:primary` assumptions and would need to be unwound later.
- Memory-only identity: evocative, but too fuzzy for canonical identity. A
  name is direct structured state, not a graph claim to derive opportunistically.

## Data Model

`agents.display_name` should change from required string state to optional
string state. Existing pre-V1 local stores may be rewritten directly; no
migration or compatibility layer is needed.

An agent record needs enough identity data for prompt construction:

```text
agent_id: string
display_name: option<string>
created_at: datetime
updated_at: datetime
```

Name updates should trim surrounding whitespace and reject empty names after
trimming. The first slice should cap stored names at 80 Unicode scalar values
after trimming. The stored name should preserve the user's intentional casing
and internal spacing unless validation later decides to normalize pathological
forms.

## Onboarding Prompt

Prompt construction should include an explicit agent identity section. When the
agent has no display name, it should also include an `onboarding_prompt` block.

Initial naming-only shape:

```text
Agent identity:
- agent_id: agent:...
- display_name: none

Onboarding prompt:
- You do not have a name yet.
- Your first priority is to ask the user to give you one.
- Do not invent, assume, or sign off with a name.
- If the user gives you a name, call update_own_name with that name.
```

When the agent has a name, the identity section includes it and no naming
onboarding item is emitted:

```text
Agent identity:
- agent_id: agent:...
- display_name: Mira
```

The apparatus should be represented in code as a small prompt builder, not as
scattered string concatenation inside the turn prompt. Future onboarding items
can be appended from missing structured agent state, for example preferred
voice, areas of responsibility, or setup preferences. The first slice should
only implement the missing-name item.

## Runtime Flow

On agent creation:

1. Noema creates the agent with `display_name = NONE`.
2. The agent is attached to its opening conversation according to the normal
   conversation lifecycle.
3. On the first generated turn, prompt construction sees the missing name and
   emits the naming `onboarding_prompt`.
4. The model asks the user for a name in its own voice.

On naming:

1. The user explicitly names or renames the current agent.
2. The model emits an `update_own_name` tool call with a payload such as
   `{"name":"Mira"}`.
3. Noema validates the tool payload and trusted runtime context.
4. Noema verifies that the current user message explicitly authorizes naming or
   renaming the current agent.
5. Noema updates `agents.display_name`.
6. Noema persists a normal tool call and tool result transcript pair.
7. The model continues and acknowledges the new name naturally.

Subsequent turns include the stored name in the identity section and omit the
missing-name onboarding prompt.

## Local Tool Contract

The tool name is `update_own_name`.

The model-supplied payload is untrusted:

```json
{
  "name": "Mira"
}
```

Trusted fields come from runtime state:

- current agent id
- current conversation id
- current turn id
- current human/user message

The tool must not accept an arbitrary target agent id from the model. It always
updates the current agent only.

Successful result shape:

```json
{
  "agent_id": "agent:primary",
  "display_name": "Mira"
}
```

Failed results should be safe and user-actionable without exposing internals,
for example:

```json
{
  "error": "name update requires explicit user instruction"
}
```

## Authorization Policy

Noema should honor `update_own_name` only when the current user message contains
an explicit naming or renaming instruction for the current agent.

Accepted examples:

- `Your name is Mira.`
- `Call yourself Orin.`
- `Rename yourself to Halcyon.`
- `I want to call you Tess.`

Rejected or confirmation-needed examples:

- `What name do you like?`
- `Maybe you could be Mira?`
- `Mira is a nice name.`
- Any tool call with no corresponding user naming instruction in the current
  turn.

If the evidence is ambiguous, the tool should fail and the model should ask the
user to confirm the name explicitly.

## Error Handling

Tool validation rejects:

- missing `name`
- non-string `name`
- empty or whitespace-only names
- names over 80 Unicode scalar values after trimming
- attempts to target another agent
- ambiguous or absent user authorization

If first-contact generation fails, the agent remains unnamed. The next normal
turn can retry because prompt construction will still see missing name state and
emit the same onboarding prompt.

If the name update succeeds but the continuation response fails, the canonical
name remains updated and the persisted tool result provides the durable audit
trail.

## Testing

Focused tests should cover:

- New agents are created with no canonical display name.
- The default primary agent is no longer seeded as `Noema`.
- Prompt construction includes the missing-name `onboarding_prompt` when
  `display_name` is empty.
- Prompt construction includes the agent's actual name when present and omits
  the missing-name onboarding item.
- `update_own_name` succeeds for explicit user naming or renaming instructions.
- Invalid or ambiguous rename attempts return failed local tool results without
  changing the stored name.
- A successful rename persists the normal tool call/result transcript pair and
  updates subsequent prompt identity.

## Non-Goals

- Do not build a full agent profile editor in this slice.
- Do not add migrations or compatibility layers for pre-V1 local data.
- Do not infer canonical names from memory claims.
- Do not let the model choose or change its name without explicit user
  instruction.
- Do not implement future onboarding questions yet; only create the lightweight
  apparatus that can hold them later.

## Self-Review

- No placeholders or open requirements remain.
- The design keeps canonical identity in structured agent state.
- The `onboarding_prompt` mechanism is narrow for this slice but gives future
  setup prompts a clear extension point.
- Authorization is explicit and does not trust model-supplied target ids.
- The scope is small enough for one implementation plan.
