# Memory Graph Tab Design

## Goal

Restore a top-level Memory tab that visualizes Supermemory's graph of memories about the human, while keeping all Supermemory network access behind Noema Core.

## User Experience

- `/memory` is a top-level shell destination again.
- `/settings/memory` remains the configuration surface for the local Supermemory service and the memory model selection.
- The Memory page renders the official Supermemory memory graph component.
- If Supermemory is unavailable, the page shows an unavailable state sourced from Noema Core.
- If the human graph has no documents, the page shows an empty state rather than falling back to unrelated conversation-scoped memory.
- The page supports loading additional graph documents without loading the full graph at once.

## Architecture

- The web UI calls Noema Core GraphQL only.
- Noema Core proxies graph document requests to the configured Supermemory service.
- The browser never connects directly to Supermemory and never receives a Supermemory key or service URL.
- Noema Core maps Supermemory's document response into the `DocumentWithMemories` shape expected by `@supermemory/memory-graph`.

## Graph Scope

- The first implementation visualizes memories for the local human scope.
- The backend uses the human container tag, `human:local`, when requesting graph documents.
- The Memory page does not expose arbitrary Supermemory space or tag selection in this slice.
- If existing ingested memory is primarily conversation-scoped, this page may initially be empty until human-scoped documents exist.

## Backend API

Add a GraphQL query that returns:

- Supermemory availability for this request.
- A list of graph documents and their memory entries.
- Pagination information needed by the graph component.

The resolver should:

- Use the existing runtime Supermemory connection.
- Return a structured unavailable response when Supermemory is not configured or not running.
- Fetch graph documents from Supermemory through the core process.
- Normalize timestamps and optional fields for the frontend.
- Avoid storing health or graph status in the local database.

## Frontend

- Add `@supermemory/memory-graph`.
- Add a top-level route for `/memory`.
- Restore a top-level shell navigation item named `Memory`.
- Query Noema Core GraphQL for graph data.
- Pass normalized documents into `MemoryGraph`.
- Disable the built-in spaces selector because Noema owns the graph scope.

## Validation

- Add backend tests for the Supermemory graph request body and response normalization.
- Add GraphQL tests for available and unavailable Supermemory states.
- Update existing route/navigation tests that encode `/memory` routing.
- Regenerate GraphQL and TanStack route artifacts.
- Run project formatting, Rust checks, and relevant frontend checks before committing implementation.

## Non-Goals

- No migration path.
- No local graph database.
- No direct web UI connection to Supermemory.
- No reintroduction of memory markers or `/remember`.
- No arbitrary Supermemory space browser in this slice.
