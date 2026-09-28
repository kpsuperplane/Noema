# Provider Generation Sessions

Local replay input is the correctness authority for each generation.

Chat keeps one warm provider session across turns when its conversation, model assignment, credentials, base context, and saved history remain compatible.
A change to these inputs or compaction closes that session. Task runs open their own sessions.

Each session receives complete replay input. It can also receive input added after the prior response.

The session owns connection state and provider response identifiers. The runtime does not retry provider continuation requests.
Every provider wrapper forwards the session's active-continuation state. The
runtime uses that state to avoid replay compaction during a live continuation.

## Responses Transport

Codex and OpenAI Responses share one WebSocket transport and one response accumulator.

The session opens its WebSocket connection when the first request starts. Closing the session closes its connection.

WebSocket requests use the provider's configured request timeout. A timeout after output does not replay the request.

Incremental input requires an earlier response identifier. Each request owns its
current instructions, tools, and generation settings. Setting changes continue
the same provider lineage.

A healthy active continuation does not compact its complete replay copy.
Finalization can narrow its instructions and tool contract while it continues
the active provider lineage.

Complete Responses replay keeps ordered reasoning, hosted web actions, assistant phases, citation annotations, and tool calls.

Codex requests use `store:false`. OpenAI requests keep the provider account storage policy.

An unsupported WebSocket handshake selects HTTP for that session. A transient setup failure selects HTTP for the current request.

Codex HTTP requests always use complete replay without a response identifier. OpenAI HTTP requests can use stored response continuation.

If an earlier response is missing, the session retries complete replay once when all prior output is replayable. Provider-hosted web state fails closed because complete replay cannot restore its source identities.

The session returns an interrupted error after text, tool, or hosted-action output arrives. It does not replay the request automatically.

## Transcript Phases

Assistant commentary and final text are separate durable items. Their provider order and exact text remain unchanged.

Same-provider replay uses provider text and phase metadata. A provider change uses readable stored text.

Old assistant items without phase metadata are final text.

Prompt caching uses the existing conversation identifier. Caching improves request cost but never replaces local replay state.

## Saved context changes

Chat preserves earlier context messages and appends changes to named sections.
The sections are agent identity, runtime environment, projects, and tool visibility.
Each request compares current values with the latest saved values after the active checkpoint.
Unchanged sections add no messages. Changed sections add complete replacements.
Missing sections add removal messages. These messages clear the earlier value.

Initial requests, tool continuations, and finalization save these updates before generation.
Provider continuation receives only new updates beside the new tool results.
Complete replay receives the same saved updates in conversation order.
Compaction compares current sections with retained history and restores only missing or changed sections.
Admission and saved replay use the same comparison. Sections retained in a checkpoint remain available for later comparisons.
A context reset starts with full sections. Memory root text remains in the base instructions.

Native tool definitions carry complete descriptions and schemas for every Chat provider.
The tool visibility section lists callable names and service ownership without repeating tool descriptions.
Context admission counts ordinary message text without adding JSON string escapes or empty record fields.

Sources: [Chat session ownership](../../internal/runtime/chat_session.go), [Responses transport](../../internal/provider/responses_websocket.go), [Codex requests](../../internal/provider/codex_generation.go), and [OpenAI requests](../../internal/provider/openai_generation.go).
