# Provider Generation Sessions

Local replay input is the correctness authority for each generation.

The runtime opens one provider session for each foreground turn, background run, and primary notification.

Each session receives complete replay input. It can also receive input added after the prior response.

The session owns connection state and provider response identifiers. The runtime does not retry provider continuation requests.
Every provider wrapper forwards the session's active-continuation state. The
runtime uses that state to avoid replay compaction during a live continuation.

## Responses Transport

Codex and OpenAI Responses share one WebSocket transport and one response accumulator.

The session opens its WebSocket connection when the first request starts. The session closes the connection when its handle is dropped.

WebSocket requests use the provider's configured request timeout. A timeout after output does not replay the request.

Incremental input requires unchanged request settings and an earlier response identifier. Changed settings use complete replay. A healthy active continuation does not compact its complete replay copy.

Progress finalization keeps the active session's instructions and tool contract. Finalization guidance enters as incremental developer input.

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
