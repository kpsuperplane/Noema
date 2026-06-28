# Web Transcript Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Standardize Noema web transcript placement so human-originated items render in the human lane and every current non-human item renders in the assistant lane using shared bubble, marker, and card primitives.

**Architecture:** Keep GraphQL-to-transcript normalization unchanged, then add a small transcript row-frame layer inside the React transcript renderer. The row frame owns lane placement and avatar gutters; `Bubble`, `Marker`, and `Attachment` own visual appearance. Marker tone variants standardize compact status/error rendering without creating special transcript layouts.

**Tech Stack:** React, TypeScript, Tailwind CSS, Base UI render helpers, class-variance-authority, Bun, Node test runner.

---

## File Structure

- Modify `crates/noema-core/web/src/components/Transcript.tsx`
  - Add exported lane classification helpers.
  - Add a shared transcript row frame for human and assistant lanes.
  - Route user text through the human row and assistant text, typing, errors, activity rows, cards, and memory markers through the assistant row.
  - Remove memory marker manual offset classes.
- Modify `crates/noema-core/web/src/components/ErrorMarker.tsx`
  - Keep this as a small wrapper around the shared `Marker` primitive.
  - Use the shared error tone instead of local red placement styling.
- Modify `crates/noema-core/web/src/components/ui/marker.tsx`
  - Add marker tone variants: `default`, `success`, `warning`, `error`, and `muted`.
  - Preserve existing structural variants: `default`, `separator`, and `border`.
- Modify `crates/noema-core/web/src/components/Transcript.test.ts`
  - Add unit coverage for lane classification and current assistant-originated event types.
  - Keep existing typing indicator and anchoring tests.

No backend files, generated GraphQL files, onboarding files, composer files, or daemon assets are part of this implementation scope.

## Task 1: Add Transcript Lane Tests

**Files:**
- Modify: `crates/noema-core/web/src/components/Transcript.test.ts`
- Later Modify: `crates/noema-core/web/src/components/Transcript.tsx`

- [ ] **Step 1: Add failing lane-classification tests**

Add these imports and tests to `crates/noema-core/web/src/components/Transcript.test.ts`:

```ts
import {
  renderedTranscriptLane,
  shouldAnchorTranscriptEntry,
  shouldShowTypingIndicator,
  transcriptEntryLane
} from "./Transcript";
```

```ts
describe("transcriptEntryLane", () => {
  test("places user entries in the human lane", () => {
    assert.equal(transcriptEntryLane("user"), "human");
  });

  test("places assistant entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("assistant"), "assistant");
  });

  test("places activity entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("activity"), "assistant");
  });

  test("places structured card entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("card"), "assistant");
  });

  test("places error entries in the assistant lane", () => {
    assert.equal(transcriptEntryLane("error"), "assistant");
  });
});

describe("renderedTranscriptLane", () => {
  test("places typing indicators in the assistant lane", () => {
    assert.equal(renderedTranscriptLane({ kind: "typing" }), "assistant");
  });

  test("places grouped memory markers in the assistant lane", () => {
    assert.equal(renderedTranscriptLane({ kind: "memory_marker" }), "assistant");
  });

  test("delegates normal entries to transcriptEntryLane", () => {
    assert.equal(renderedTranscriptLane({ kind: "entry", entryType: "user" }), "human");
    assert.equal(renderedTranscriptLane({ kind: "entry", entryType: "error" }), "assistant");
  });
});
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run:

```bash
cd crates/noema-core/web
bun test src/components/Transcript.test.ts
```

Expected result:

```text
FAIL src/components/Transcript.test.ts
renderedTranscriptLane is not exported
```

The exact failure can name `transcriptEntryLane` first. The required signal is that the new tests fail because the lane helpers do not exist yet.

- [ ] **Step 3: Commit after the failing test is in place**

Run:

```bash
git add crates/noema-core/web/src/components/Transcript.test.ts
git commit -m "test: cover transcript lane classification"
```

Expected result:

```text
[main <hash>] test: cover transcript lane classification
```

## Task 2: Implement Transcript Lane Classification And Row Frame

**Files:**
- Modify: `crates/noema-core/web/src/components/Transcript.tsx`
- Test: `crates/noema-core/web/src/components/Transcript.test.ts`

- [ ] **Step 1: Add lane types and exported helper signatures**

In `crates/noema-core/web/src/components/Transcript.tsx`, add these types near `RenderTranscriptEntry`:

```ts
type TranscriptLane = "human" | "assistant";

type RenderTranscriptLaneCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" };
```

Add these exported helpers below `shouldAnchorRenderedEntry`:

```ts
export function transcriptEntryLane(entryType: TranscriptEntry["type"]): TranscriptLane {
  return entryType === "user" ? "human" : "assistant";
}

export function renderedTranscriptLane(entry: RenderTranscriptLaneCandidate): TranscriptLane {
  if (entry.kind === "entry") {
    return transcriptEntryLane(entry.entryType);
  }
  return "assistant";
}
```

- [ ] **Step 2: Run the focused test to verify the helper tests pass**

Run:

```bash
cd crates/noema-core/web
bun test src/components/Transcript.test.ts
```

Expected result:

```text
pass
```

- [ ] **Step 3: Add the shared transcript row frame**

In `crates/noema-core/web/src/components/Transcript.tsx`, add this component near `Message`:

```tsx
function TranscriptRow({
  lane,
  children
}: {
  lane: TranscriptLane;
  children: React.ReactNode;
}) {
  const role = lane === "human" ? "user" : "assistant";

  return (
    <MessagePrimitive align={lane === "human" ? "end" : "start"} className="max-w-[760px]">
      <MessageAvatar>
        <Avatar size="sm">
          <AvatarFallback>{role === "user" ? "ME" : "N"}</AvatarFallback>
        </Avatar>
      </MessageAvatar>
      <MessageContent>{children}</MessageContent>
    </MessagePrimitive>
  );
}
```

Update `Message` to render only the bubble content inside `TranscriptRow`:

```tsx
function Message({ role, text }: { role: "user" | "assistant"; text: string }) {
  return (
    <TranscriptRow lane={role === "user" ? "human" : "assistant"}>
      <Bubble variant={role === "user" ? "default" : "muted"}>
        <BubbleContent className="leading-[1.7] whitespace-pre-wrap">{text}</BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}
```

Update `TypingMessage` to use the assistant lane:

```tsx
function TypingMessage() {
  return (
    <TranscriptRow lane="assistant">
      <Bubble variant="muted">
        <BubbleContent
          className="flex min-h-9 w-[58px] items-center justify-center gap-1.5 px-3 py-2"
          aria-label="Noema is typing"
          role="status"
        >
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.24s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.12s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70" />
        </BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}
```

- [ ] **Step 4: Route non-human renderers through the assistant row**

In `renderTranscriptRenderEntry`, wrap memory markers:

```tsx
if (entry.kind === "memory_marker") {
  return (
    <TranscriptRow lane="assistant">
      <MemoryMarker
        id={entry.id}
        extraction={entry.extraction}
        proposal={entry.proposal}
        open={expandedActivities.has(entry.id)}
        onToggle={() => onToggleActivity(entry.id)}
      />
    </TranscriptRow>
  );
}
```

In `renderTranscriptEntry`, wrap activity rows, structured cards, and errors:

```tsx
if (entry.type === "activity") {
  return (
    <TranscriptRow lane="assistant">
      <ActivityRow item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
    </TranscriptRow>
  );
}
if (entry.type === "card") {
  return (
    <TranscriptRow lane="assistant">
      <StructuredCard item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
    </TranscriptRow>
  );
}
return (
  <TranscriptRow lane="assistant">
    <ErrorNotice message={entry.message} recoverable={entry.recoverable} />
  </TranscriptRow>
);
```

- [ ] **Step 5: Remove duplicated item-level max widths where the row owns width**

In `ActivityRow`, change:

```tsx
<Attachment className="max-w-[760px]">
```

to:

```tsx
<Attachment className="w-full max-w-full">
```

In the generic structured-card fallback, change:

```tsx
<Attachment className="max-w-[760px]">
```

to:

```tsx
<Attachment className="w-full max-w-full">
```

In `MemoryStructuredCard`, change:

```tsx
<Attachment className="max-w-[760px]">
```

to:

```tsx
<Attachment className="w-full max-w-full">
```

In `MemoryDetailAttachment`, change:

```tsx
<Attachment id={id} state={failed ? "error" : "done"} className="max-w-[760px]">
```

to:

```tsx
<Attachment id={id} state={failed ? "error" : "done"} className="w-full max-w-full">
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/Transcript.test.ts
```

Expected result:

```text
pass
```

- [ ] **Step 7: Commit the lane frame implementation**

Run:

```bash
git add crates/noema-core/web/src/components/Transcript.tsx crates/noema-core/web/src/components/Transcript.test.ts
git commit -m "feat: add transcript lane frame"
```

Expected result:

```text
[main <hash>] feat: add transcript lane frame
```

## Task 3: Standardize Marker Tone Variants

**Files:**
- Modify: `crates/noema-core/web/src/components/ui/marker.tsx`
- Modify: `crates/noema-core/web/src/components/ErrorMarker.tsx`
- Modify: `crates/noema-core/web/src/components/Transcript.tsx`

- [ ] **Step 1: Add tone variants to `Marker`**

In `crates/noema-core/web/src/components/ui/marker.tsx`, update `markerVariants` to include a `tone` axis:

```ts
const markerVariants = cva(
  "group/marker relative flex min-h-4 w-full items-center gap-2 text-left text-sm [&_svg:not([class*='size-'])]:size-4 [a]:underline [a]:underline-offset-3 [a]:hover:text-foreground",
  {
    variants: {
      variant: {
        default: "",
        separator:
          "before:mr-1 before:h-px before:min-w-0 before:flex-1 before:bg-border after:ml-1 after:h-px after:min-w-0 after:flex-1 after:bg-border",
        border: "border-b border-border pb-2",
      },
      tone: {
        default: "text-muted-foreground",
        success: "text-[var(--pine-700)]",
        warning: "text-[var(--clay-600)]",
        error: "text-[var(--red-700)]",
        muted: "text-[var(--text-faint)]",
      },
    },
    defaultVariants: {
      variant: "default",
      tone: "default",
    },
  }
)
```

The `Marker` function already accepts `VariantProps<typeof markerVariants>`, so `tone` becomes an allowed prop after this change.

- [ ] **Step 2: Update `ErrorMarker` to use marker tone**

In `crates/noema-core/web/src/components/ErrorMarker.tsx`, change the marker call to:

```tsx
<Marker
  role={recoverable ? "status" : "alert"}
  tone="error"
  className={cn("w-fit max-w-full", className)}
>
```

Keep `MarkerContent` unchanged.

- [ ] **Step 3: Update memory marker tone usage**

In `crates/noema-core/web/src/components/Transcript.tsx`, update `MemoryMarker`:

```tsx
<Marker
  render={<button type="button" />}
  aria-expanded={open}
  aria-controls={`${id}-details`}
  onClick={onToggle}
  tone={failed ? "error" : "success"}
  className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
>
```

Remove `failed && "text-[var(--red-700)]"` from this marker because tone now owns the color.

- [ ] **Step 4: Run frontend lint for type and class issues**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected result:

```text
No lint errors.
```

- [ ] **Step 5: Commit marker standardization**

Run:

```bash
git add crates/noema-core/web/src/components/ui/marker.tsx crates/noema-core/web/src/components/ErrorMarker.tsx crates/noema-core/web/src/components/Transcript.tsx
git commit -m "feat: standardize transcript marker tones"
```

Expected result:

```text
[main <hash>] feat: standardize transcript marker tones
```

## Task 4: Remove Manual Memory Offset And Verify Responsive Layout

**Files:**
- Modify: `crates/noema-core/web/src/components/Transcript.tsx`

- [ ] **Step 1: Remove the memory marker manual lane offset**

In `MemoryMarker`, change the wrapper from:

```tsx
<div className="ml-10 grid w-[calc(100%-2.5rem)] max-w-[720px] gap-2">
```

to:

```tsx
<div className="grid w-full max-w-full gap-2">
```

- [ ] **Step 2: Keep expanded memory details in the same assistant content column**

Confirm `MemoryDetailAttachment` still renders under the marker:

```tsx
{open ? (
  <MemoryDetailAttachment id={`${id}-details`} extraction={extraction} memoryCount={memories.length} failed={failed} />
) : null}
```

No code change is needed in this step if the call already matches the snippet.

- [ ] **Step 3: Run focused tests**

Run:

```bash
cd crates/noema-core/web
bun test src/components/Transcript.test.ts
```

Expected result:

```text
pass
```

- [ ] **Step 4: Run lint and build**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected result:

```text
No lint errors.
Build completes successfully.
```

- [ ] **Step 5: Commit memory marker placement cleanup**

Run:

```bash
git add crates/noema-core/web/src/components/Transcript.tsx
git commit -m "fix: align memory markers with assistant lane"
```

Expected result:

```text
[main <hash>] fix: align memory markers with assistant lane
```

## Task 5: Browser Smoke Check

**Files:**
- No source edits expected.
- Use the local web app under `crates/noema-core/web`.

- [ ] **Step 1: Start the frontend dev server**

Run:

```bash
cd crates/noema-core/web
bun run dev --host 127.0.0.1
```

Expected result:

```text
Local: http://127.0.0.1:<port>/
```

Keep the server running for the next steps.

- [ ] **Step 2: Capture desktop layout**

Open the local URL at a desktop viewport such as `1280x900`.

Expected visual checks:

- Assistant bubbles, typing indicator, errors, memory markers, activity cards, and structured cards all start in the Noema lane.
- The Noema avatar gutter is consistent for every non-human transcript item.
- Human text remains right-aligned in the human lane.
- Compact markers do not span wider than their content unless the text wraps.
- Expanded cards stay inside the assistant content column.

- [ ] **Step 3: Capture mobile layout**

Open the same local URL at a mobile viewport such as `390x844`.

Expected visual checks:

- No horizontal overflow.
- Error marker text wraps inside the assistant content column.
- Memory marker and expanded card stay aligned with assistant bubbles.
- Human bubbles remain right-aligned and do not collide with assistant rows.

- [ ] **Step 4: Stop the dev server**

Stop the dev server with `Ctrl-C`.

Expected result:

```text
Dev server exits cleanly.
```

## Task 6: Final Validation And Handoff

**Files:**
- No source edits expected unless validation reveals a scoped bug.

- [ ] **Step 1: Run final frontend validation**

Run:

```bash
cd crates/noema-core/web
bun test src/components/Transcript.test.ts
bun run lint
bun run build
```

Expected result:

```text
Transcript tests pass.
No lint errors.
Build completes successfully.
```

Do not run `bun run gen:types`; the planned implementation does not change GraphQL operations or generated schema/types.

- [ ] **Step 2: Check repository state**

Run:

```bash
git status --short --branch
git diff --check
```

Expected result:

```text
git diff --check exits with no whitespace errors.
git status shows only intentional transcript cleanup changes plus pre-existing unrelated worktree changes.
```

- [ ] **Step 3: Summarize implementation results**

Include in the handoff:

```text
Implemented the transcript lane frame so all current non-human transcript items render in the assistant lane.
Standardized Marker tone variants and routed errors through the shared error tone.
Verified with Transcript tests, lint, build, and desktop/mobile smoke checks.
Unrelated pre-existing worktree changes were preserved.
```

## Self-Review Notes

- Spec coverage: The plan covers origin-based placement, assistant lane for current non-human items, future human-lane cards, marker tone standardization, memory offset removal, error semantics, tests, lint, build, and browser smoke checks.
- Placeholder scan: No `TBD`, `TODO`, or undefined later work remains in the plan.
- Type consistency: The plan introduces `TranscriptLane`, `transcriptEntryLane`, and `renderedTranscriptLane` before tests and render usage depend on them.
