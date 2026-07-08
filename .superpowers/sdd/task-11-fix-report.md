# Task 11 Global Constraint Fix Report

## Implementation

- Removed the `memory_marker` render entry kind from the transcript render model.
- Removed all memory extraction/proposal grouping in `renderableTranscriptEntries`; historical `memory_extraction` activities and `memory_proposals` cards now remain ordinary transcript `entry` rows.
- Removed the `memory_marker` rendering branch from `Transcript.tsx`.
- Removed the `memory_save` special card/detail affordance from `ActivityRow.tsx`; activities now render through the generic activity notice path.
- Removed the `memory_proposals` and `memory_cards` special cases from `StructuredCard.tsx`; structured cards now render as a generic attachment card.
- Removed the memory mode from `ToolMarker.tsx`; it now renders only actual tool call/result markers.
- Removed stale memory marker/detail UI helpers:
  - `MemoryDetailAttachment.tsx`
  - `MemoryDetailList.tsx`
  - `MemoryDetailRow.tsx`
  - `MemoryStructuredCard.tsx`
  - `src/memory/cards.ts`
- Removed stale memory detail helpers/tests from `markerModel.ts` and `markerModel.test.ts`.
- Removed live transcript cleanup that dropped started memory extraction entries on turn completion; transcript event handling no longer special-cases automatic memory capture.

## Tests And Searches

- Added a render-model regression test proving old memory-shaped transcript items remain generic entries:
  - `renderableTranscriptEntries > leaves memory extraction and proposal items on the generic transcript path`
- Verified focused transcript tests:
  - `bun test src/components/transcript/renderModel.test.ts src/components/transcript/markerModel.test.ts src/components/transcript/transcriptScrollerModel.test.ts src/transcript/window.test.ts`
  - Result: 32 pass, 0 fail.
- Verified web lint:
  - `bun run lint`
  - Result: passed.
- Verified web build:
  - `bun run build`
  - Result: passed. Vite emitted the existing large chunk warning for `app.js`.
- Search:
  - `rg -n "memory_marker|memory_extraction|memory_proposals|memory_save|MemoryStructuredCard|MemoryDetailAttachment|MemoryDetailList|memoryCardsFromStructuredItem" crates/noema-core/web/src`
  - Remaining hits are intentional test fixtures only:
    - `components/transcript/renderModel.test.ts` uses `memory_extraction` and `memory_proposals` as historical input shapes and asserts they render as generic entries.

## Files Changed Or Deleted

- Changed:
  - `crates/noema-core/web/src/app/App.tsx`
  - `crates/noema-core/web/src/components/transcript/ActivityRow.tsx`
  - `crates/noema-core/web/src/components/transcript/StructuredCard.tsx`
  - `crates/noema-core/web/src/components/transcript/ToolMarker.tsx`
  - `crates/noema-core/web/src/components/transcript/Transcript.tsx`
  - `crates/noema-core/web/src/components/transcript/markerModel.ts`
  - `crates/noema-core/web/src/components/transcript/markerModel.test.ts`
  - `crates/noema-core/web/src/components/transcript/renderModel.ts`
  - `crates/noema-core/web/src/components/transcript/renderModel.test.ts`
  - `crates/noema-core/web/src/components/transcript/scrollModel.ts`
  - `crates/noema-core/web/src/transcript/events.ts`
  - `crates/noema-core/web/src/transcript/window.test.ts`
- Deleted:
  - `crates/noema-core/web/src/components/transcript/MemoryDetailAttachment.tsx`
  - `crates/noema-core/web/src/components/transcript/MemoryDetailList.tsx`
  - `crates/noema-core/web/src/components/transcript/MemoryDetailRow.tsx`
  - `crates/noema-core/web/src/components/transcript/MemoryStructuredCard.tsx`
  - `crates/noema-core/web/src/memory/cards.ts`

## Self-Review

- Confirmed no `memory_marker` render entry kind remains.
- Confirmed transcript rendering no longer branches on `memory_extraction`, `memory_proposals`, or `memory_save`.
- Confirmed tool marker rendering still groups only `tool_call` and `tool_result` activities.
- Confirmed deleted memory UI helpers have no remaining imports.
- Confirmed old memory-shaped fixtures are retained only in the regression test asserting generic rendering.

## Concerns

- The worktree contains unrelated dirty files outside this fix:
  - `crates/noema-core/src/daemon/prompts.rs`
  - `crates/noema-core/src/daemon/tests.rs`
- Those files were not touched for this fix and should not be included in the commit.
