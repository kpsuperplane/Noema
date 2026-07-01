# MCP Tool List Cache Invalidation Design

## Goal

Keep already-open chats up to date with newly added or removed tools on the very next user turn, while still avoiding unnecessary recomputation of the model-visible tool list on every turn.

## Problem Statement

Noema now supports MCP tool discovery and built-in model tools, but a conversation that is already open can still keep using a stale tool set if tool availability was captured too early. The desired behavior is:

- a tool list change becomes visible on the next user turn
- the cache should stay useful
- privacy or calibration details that do not change the final model-visible tool list should not force a refresh

The cache boundary should therefore be the exact final list of tools provided to the model, regardless of whether those tools are built-in or MCP-backed.

## Scope

In scope:

- caching the final model-visible tool list
- hashing that final list for cheap change detection
- invalidating the cache when any write can change that final list
- refreshing the cache at turn start when needed

Out of scope:

- changing the underlying MCP permission or privacy model
- changing how tool visibility is determined
- conversation-level semantic caching beyond the tool list
- provider-specific caching of unrelated prompt sections

## Design

### Cache shape

Store one cached tool snapshot per conversation, or per conversation plus any other identity dimension that affects which tools are available to that conversation. The snapshot contains:

- the final ordered tool list sent to the model
- a stable hash of that list

The cache must hold the final model-visible list, not raw MCP server rows or calibration rows. That keeps the cache aligned with the actual provider request boundary.
The cache lives in runtime memory, scoped to the active conversation state, and is rebuilt after process restart.

### Hashing

Compute a stable hash from the final tool list after it has been assembled and normalized. The hash should cover the model-visible tool identity and shape, including any fields that affect the final tool payload sent to the provider.

This hash is used as a cheap dirty check:

- if the hash matches the cached snapshot, reuse the snapshot
- if it differs, rebuild the snapshot and replace the cache

This is intentionally lightweight. The expensive part is discovering and assembling the tools, not comparing a hash.
The hash is runtime-only and does not need to be persisted.

### Invalidation rules

Invalidate the cached tool snapshot on any change that can alter the final list of tools the model sees. That includes:

- MCP server create
- MCP server delete
- MCP server enable or disable changes, if they affect model-visible availability
- tool discovery refreshes that add or remove discovered tools
- calibration saves that add or remove a tool from the final model-visible list
- built-in tool registry changes

Do not invalidate for privacy-only changes if those changes are evaluated deterministically by the tool apparatus and do not change the final model-visible list.

### Turn flow

At the beginning of each user turn:

1. Look up the cached final tool snapshot for the conversation.
2. If there is no cached snapshot, compute the current final tool list.
3. If there is a cached snapshot, compare its hash to the current final tool list hash.
4. Reuse the cached snapshot when the hash matches.
5. Rebuild and replace the cache when the hash differs.
6. Pass the current final tool list into provider request construction for that turn.

This guarantees that an already-open conversation picks up a tool change on the next user turn, while still allowing stable reuse when nothing changed.

## Data Flow

1. A user sends a new message in an existing conversation.
2. The daemon begins turn setup.
3. The daemon resolves the current final tool list from built-in and MCP sources.
4. The daemon computes or compares the stable hash.
5. If needed, the daemon refreshes the cache.
6. The daemon builds the provider request with the refreshed final list.
7. The provider sees the updated tool set on that same turn.

## Failure Handling

- If tool resolution fails, the turn should fail the same way it would today when the provider cannot be prepared.
- A stale cache entry must never outlive a tool-list-changing write without being invalidated.
- If hash computation or normalization fails, the system should treat that as a cache miss and rebuild from source rather than silently reusing a potentially stale snapshot.

## Testing

Add coverage for the following cases:

- an existing conversation sees a newly added MCP tool on the next user turn
- an existing conversation sees a removed MCP tool on the next user turn
- a tool-list-preserving change does not force cache invalidation
- built-in tool changes participate in the same cache boundary
- the cached hash matches when the final list is unchanged
- the cached hash changes when a tool identity or model-visible shape changes

## Open Questions
None.

## Decision

Use a cached final tool list with a stable hash, store it in runtime memory scoped to the active conversation state, and invalidate it on any write that can change the model-visible tool list. That gives immediate next-turn freshness with inexpensive reuse when nothing changed.
