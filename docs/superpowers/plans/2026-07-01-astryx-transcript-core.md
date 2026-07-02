# Astryx Transcript Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adopt Astryx core chat primitives for Noema transcript message, typing, and tool-call rows while preserving Noema's transcript scroller, render model, replay behavior, memory/activity rows, cards, and composer.

**Architecture:** Keep `Transcript`, `TranscriptScroller`, `TranscriptBottomFollower`, and `renderModel.ts` as Noema-owned infrastructure. Introduce a small transcript avatar adapter, render text/typing rows with Astryx `ChatMessage` and `ChatMessageBubble`, and render grouped tool activity with Astryx `ChatToolCalls` while reusing Noema's existing tool detail attachment.

**Tech Stack:** React 19, TypeScript, StyleX, Astryx `@astryxdesign/core/Chat`, existing Noema transcript components, Bun validation.

---

## Current Dirty Worktree

At plan-writing time, the following files were already dirty and must not be
staged or reverted unless the user explicitly asks:

- `.gitignore`
- `crates/noema-core/src/daemon/web/assets/app.js`
- `crates/noema-core/src/daemon/web/assets/styles.css`

Implementation should stage only files changed for this transcript slice.

## File Structure

Create:

- `crates/noema-core/web/src/components/transcript/TranscriptActorAvatar.tsx`
  - Shared transcript-local adapter that maps Noema transcript lanes to
    deterministic Noema identity avatars while supporting hidden avatar gutters
    for grouped rows.

Modify:

- `crates/noema-core/web/src/components/transcript/TranscriptRow.tsx`
  - Reuse `TranscriptActorAvatar` for memory/activity/card/error rows that
    still use the Noema row wrapper.
- `crates/noema-core/web/src/components/transcript/Message.tsx`
  - Replace the local bubble implementation with Astryx `ChatMessage` and
    `ChatMessageBubble`, preserving `AnimatedMessageText`.
- `crates/noema-core/web/src/components/transcript/TypingMessage.tsx`
  - Use Astryx `ChatMessage` and `ChatMessageBubble`, preserving the Noema dots
    and `role="status"` semantics.
- `crates/noema-core/web/src/components/transcript/markerModel.ts`
  - Export a structured helper for the display tool name so `ToolMarker` can
    feed Astryx `ChatToolCalls` without duplicating metadata parsing or changing
    the existing `toolMarkerLabel` wording used by detail cards.
- `crates/noema-core/web/src/components/transcript/ToolMarker.tsx`
  - Render the grouped tool marker through Astryx `ChatToolCalls` with Noema's
    existing `ToolDetailAttachment` as the expanded detail.

Do not modify in this slice:

- `crates/noema-core/web/src/components/transcript/Transcript.tsx`
- `crates/noema-core/web/src/components/transcript/TranscriptScroller.tsx`
- `crates/noema-core/web/src/components/transcript/TranscriptBottomFollower.tsx`
- `crates/noema-core/web/src/components/transcript/renderModel.ts`
- Noema composer files
- Memory marker/detail files unless TypeScript requires import-only cleanup

## Task 1: Add Shared Transcript Avatar Adapter

**Files:**
- Create: `crates/noema-core/web/src/components/transcript/TranscriptActorAvatar.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/TranscriptRow.tsx`

- [ ] **Step 1: Create `TranscriptActorAvatar.tsx`**

Create `crates/noema-core/web/src/components/transcript/TranscriptActorAvatar.tsx`:

```tsx
import * as stylex from "@stylexjs/stylex";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID, LOCAL_HUMAN_AVATAR_ID } from "../IdentityAvatar";
import type { TranscriptLane } from "./renderModel";

const styles = stylex.create({
  root: {
    display: "flex",
    width: "fit-content",
    minWidth: 32,
    alignItems: "center",
    justifyContent: "center",
    alignSelf: "flex-end",
    flexShrink: 0,
    overflow: "hidden"
  },
  hidden: {
    visibility: "hidden"
  }
});

export function TranscriptActorAvatar({
  lane,
  visible = true
}: {
  lane: TranscriptLane;
  visible?: boolean;
}) {
  const actorId = lane === "human" ? LOCAL_HUMAN_AVATAR_ID : LOCAL_AGENT_AVATAR_ID;
  const actorType = lane === "human" ? "human" : "agent";

  return (
    <span {...stylex.props(styles.root, !visible && styles.hidden)} aria-hidden={!visible}>
      <IdentityAvatar actorId={actorId} actorType={actorType} size="sm" />
    </span>
  );
}
```

- [ ] **Step 2: Update `TranscriptRow.tsx` to use the adapter**

Replace the direct `IdentityAvatar` import and avatar rendering with:

```tsx
import * as stylex from "@stylexjs/stylex";
import type { TranscriptLane } from "./renderModel";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";
```

Remove the `avatar` and `hiddenAvatar` styles from `styles`.

In the returned JSX, replace the avatar `<div>` with:

```tsx
<TranscriptActorAvatar lane={lane} visible={showAvatar} />
```

The final return block should be:

```tsx
return (
  <div {...stylex.props(styles.root, lane === "human" && styles.human)} data-lane={lane}>
    <TranscriptActorAvatar lane={lane} visible={showAvatar} />
    <div {...stylex.props(styles.content, lane === "human" && styles.humanContent)}>{children}</div>
  </div>
);
```

- [ ] **Step 3: Run TypeScript/lint feedback for touched files**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: lint may still fail later if other tasks are incomplete, but there
should be no import or unused-style errors in `TranscriptActorAvatar.tsx` or
`TranscriptRow.tsx`.

- [ ] **Step 4: Commit Task 1**

```bash
git add crates/noema-core/web/src/components/transcript/TranscriptActorAvatar.tsx \
  crates/noema-core/web/src/components/transcript/TranscriptRow.tsx
git commit -m "refactor: share transcript actor avatar"
```

## Task 2: Render Text Messages With Astryx Chat Primitives

**Files:**
- Modify: `crates/noema-core/web/src/components/transcript/Message.tsx`

- [ ] **Step 1: Replace `Message.tsx` implementation**

Replace the file contents with:

```tsx
import { ChatMessage, ChatMessageBubble, type ChatMessageBubbleProps, type ChatMessageProps } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { AnimatedMessageText } from "../MessageTextAnimation";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";

type ChatMessageXStyle = ChatMessageProps["xstyle"];
type ChatMessageBubbleXStyle = ChatMessageBubbleProps["xstyle"];

const styles = stylex.create({
  message: {
    width: "100%",
    maxWidth: 760,
    minWidth: 0
  },
  bubble: {
    width: "fit-content",
    maxWidth: "80%",
    minWidth: 0,
    overflow: "hidden",
    fontSize: 14,
    lineHeight: 1.7,
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap"
  },
  assistantBubble: {
    maxWidth: "100%"
  }
});

export function Message({
  animate,
  role,
  text,
  showAvatar
}: {
  animate: boolean;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
}) {
  const lane = role === "user" ? "human" : "assistant";
  const sender = role === "user" ? "user" : "assistant";

  return (
    <ChatMessage
      sender={sender}
      avatar={<TranscriptActorAvatar lane={lane} visible={showAvatar} />}
      xstyle={chatMessageXStyle(styles.message)}
    >
      <ChatMessageBubble
        xstyle={chatMessageBubbleXStyle(styles.bubble, role === "assistant" && styles.assistantBubble)}
      >
        <AnimatedMessageText animate={animate} text={text} />
      </ChatMessageBubble>
    </ChatMessage>
  );
}

function chatMessageXStyle(...xstyle: unknown[]): ChatMessageXStyle {
  return xstyle as unknown as ChatMessageXStyle;
}

function chatMessageBubbleXStyle(...xstyle: unknown[]): ChatMessageBubbleXStyle {
  return xstyle as unknown as ChatMessageBubbleXStyle;
}
```

- [ ] **Step 2: Verify no `TranscriptRow` import remains in `Message.tsx`**

Run:

```bash
rg -n "TranscriptRow" crates/noema-core/web/src/components/transcript/Message.tsx
```

Expected: no output and exit code `1`.

- [ ] **Step 3: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: pass, or fail only on files unrelated to this task. If lint reports
line-length or formatting issues in `Message.tsx`, format the imports and JSX
without changing behavior.

- [ ] **Step 4: Commit Task 2**

```bash
git add crates/noema-core/web/src/components/transcript/Message.tsx
git commit -m "refactor: render transcript messages with astryx chat"
```

## Task 3: Render Typing Indicator With Astryx Chat Primitives

**Files:**
- Modify: `crates/noema-core/web/src/components/transcript/TypingMessage.tsx`

- [ ] **Step 1: Replace `TypingMessage.tsx` imports**

Use these imports:

```tsx
import { ChatMessage, ChatMessageBubble, type ChatMessageBubbleProps, type ChatMessageProps } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { TranscriptActorAvatar } from "./TranscriptActorAvatar";
```

- [ ] **Step 2: Replace `TypingMessage.tsx` styles and component**

Keep `dotAnimation` unchanged. Replace the StyleX styles and component body
with:

```tsx
type ChatMessageXStyle = ChatMessageProps["xstyle"];
type ChatMessageBubbleXStyle = ChatMessageBubbleProps["xstyle"];

const styles = stylex.create({
  message: {
    width: "100%",
    maxWidth: 760,
    minWidth: 0
  },
  bubble: {
    width: 58,
    minHeight: 36
  },
  content: {
    display: "flex",
    width: "100%",
    minHeight: 36,
    alignItems: "center",
    justifyContent: "center",
    gap: 6
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 999,
    backgroundColor: "color-mix(in srgb, var(--muted-foreground) 70%, transparent)",
    ...dotAnimation
  },
  firstDot: {
    animationDelay: "-0.24s"
  },
  secondDot: {
    animationDelay: "-0.12s"
  }
});

export function TypingMessage({ showAvatar }: { showAvatar: boolean }) {
  return (
    <ChatMessage
      sender="assistant"
      avatar={<TranscriptActorAvatar lane="assistant" visible={showAvatar} />}
      xstyle={chatMessageXStyle(styles.message)}
    >
      <ChatMessageBubble xstyle={chatMessageBubbleXStyle(styles.bubble)}>
        <div {...stylex.props(styles.content)} aria-label="Noema is typing" role="status">
          <span {...stylex.props(styles.dot, styles.firstDot)} />
          <span {...stylex.props(styles.dot, styles.secondDot)} />
          <span {...stylex.props(styles.dot)} />
        </div>
      </ChatMessageBubble>
    </ChatMessage>
  );
}

function chatMessageXStyle(...xstyle: unknown[]): ChatMessageXStyle {
  return xstyle as unknown as ChatMessageXStyle;
}

function chatMessageBubbleXStyle(...xstyle: unknown[]): ChatMessageBubbleXStyle {
  return xstyle as unknown as ChatMessageBubbleXStyle;
}
```

- [ ] **Step 3: Verify accessibility attributes remain**

Run:

```bash
rg -n "Noema is typing|role=\"status\"" crates/noema-core/web/src/components/transcript/TypingMessage.tsx
```

Expected output includes both `Noema is typing` and `role="status"`.

- [ ] **Step 4: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: pass, or fail only on files unrelated to this task.

- [ ] **Step 5: Commit Task 3**

```bash
git add crates/noema-core/web/src/components/transcript/TypingMessage.tsx
git commit -m "refactor: render typing row with astryx chat"
```

## Task 4: Render Tool Activity With Astryx ChatToolCalls

**Files:**
- Modify: `crates/noema-core/web/src/components/transcript/markerModel.ts`
- Modify: `crates/noema-core/web/src/components/transcript/ToolMarker.tsx`

- [ ] **Step 1: Export a tool display-name helper from `markerModel.ts`**

In `markerModel.ts`, add this function near `toolMarkerLabel`:

```ts
export function toolMarkerName(marker: ToolMarkerGroup): string {
  return (
    toolNameFromMetadata(marker.call?.item.metadata) ??
    toolNameFromMetadata(marker.result?.item.metadata) ??
    marker.call?.item.title ??
    marker.result?.item.title ??
    "Tool activity"
  );
}
```

Do not change `toolMarkerLabel`. It is still used by `ToolDetailAttachment`,
and its current fallback behavior intentionally returns activity titles without
adding `Used`.

- [ ] **Step 2: Replace `ToolMarker.tsx` implementation**

Replace the file contents with:

```tsx
import { ChatToolCalls, type ChatToolCallItem, type ChatToolCallStatus } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { toolMarkerName, toolMarkerPending } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";

const styles = stylex.create({
  root: {
    display: "grid",
    width: "100%",
    maxWidth: "100%",
    gap: 8
  }
});

export function ToolMarker({
  marker,
  open,
  onToggle
}: {
  marker: ToolMarkerGroup;
  open: boolean;
  onToggle: () => void;
}) {
  const calls = toolMarkerCalls(marker);

  return (
    <div {...stylex.props(styles.root)}>
      <ChatToolCalls
        calls={calls}
        isExpanded={open}
        onExpandedChange={(nextOpen) => {
          if (nextOpen !== open) {
            onToggle();
          }
        }}
      />
    </div>
  );
}

function toolMarkerCalls(marker: ToolMarkerGroup): ChatToolCallItem[] {
  const target = marker.result?.item.summary ?? marker.call?.item.summary;
  const errorMessage =
    marker.result?.item.status === "FAILED" ? marker.result.item.summary ?? marker.result.item.title : undefined;
  const call: ChatToolCallItem = {
    key: marker.id,
    name: toolMarkerName(marker),
    status: toolMarkerStatus(marker),
    resultDetail: <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} />
  };

  if (target) {
    call.target = target;
  }
  if (errorMessage) {
    call.errorMessage = errorMessage;
  }

  return [call];
}

function toolMarkerStatus(marker: ToolMarkerGroup): ChatToolCallStatus {
  if (marker.result?.item.status === "FAILED") {
    return "error";
  }
  if (marker.result) {
    return "complete";
  }
  if (toolMarkerPending(marker)) {
    return "running";
  }
  return "pending";
}
```

- [ ] **Step 3: Verify old marker frame is no longer used by `ToolMarker.tsx`**

Run:

```bash
rg -n "TranscriptMarkerFrame|WrenchIcon|toolMarkerTone|toolMarkerLabel" crates/noema-core/web/src/components/transcript/ToolMarker.tsx
```

Expected: no output and exit code `1`.

- [ ] **Step 4: Run lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: pass. If TypeScript reports that `resultDetail` is unsupported, stop and inspect
`crates/noema-core/web/node_modules/@astryxdesign/core/dist/Chat/ChatToolCalls.d.ts`.
The local type definition used for this plan includes `resultDetail?: ReactNode`
on `ChatToolCallItem`; any mismatch means the installed package changed and the
worker should adapt to the local type, preserving `ToolDetailAttachment`.

- [ ] **Step 5: Commit Task 4**

```bash
git add crates/noema-core/web/src/components/transcript/markerModel.ts \
  crates/noema-core/web/src/components/transcript/ToolMarker.tsx
git commit -m "refactor: render tool markers with astryx chat calls"
```

## Task 5: Final Validation And Cleanup

**Files:**
- Inspect only unless validation requires fixes.

- [ ] **Step 1: Confirm no forbidden stack usage was introduced**

Run:

```bash
rg -n "tailwind|className=|@base-ui|@shadcn|components/ui" crates/noema-core/web/src/components/transcript
```

Expected: no output, except existing `className` text inside unrelated comments
or type-safe adapter code if present. Do not introduce Tailwind classes.

- [ ] **Step 2: Confirm Noema transcript shell ownership remains**

Run:

```bash
git diff --name-only HEAD~4..HEAD
```

Expected files are limited to:

```text
crates/noema-core/web/src/components/transcript/TranscriptActorAvatar.tsx
crates/noema-core/web/src/components/transcript/TranscriptRow.tsx
crates/noema-core/web/src/components/transcript/Message.tsx
crates/noema-core/web/src/components/transcript/TypingMessage.tsx
crates/noema-core/web/src/components/transcript/markerModel.ts
crates/noema-core/web/src/components/transcript/ToolMarker.tsx
```

If additional files changed, inspect each one and keep it only if it is directly
required by this plan.

- [ ] **Step 3: Run frontend lint**

Run:

```bash
cd crates/noema-core/web
bun run lint
```

Expected: pass.

- [ ] **Step 4: Run frontend build**

Run:

```bash
cd crates/noema-core/web
bun run build
```

Expected: pass.

- [ ] **Step 5: Run repository diff checks**

Run:

```bash
git status --short --branch
git diff --check
```

Expected:

- `git diff --check` prints no output and exits `0`.
- `git status --short --branch` may still show the pre-existing dirty files
  listed at the top of this plan. Do not stage those files.

- [ ] **Step 6: Inspect final staged changes before any final commit**

If any validation fixes were needed after Task 4, stage only transcript files:

```bash
git add crates/noema-core/web/src/components/transcript/TranscriptActorAvatar.tsx \
  crates/noema-core/web/src/components/transcript/TranscriptRow.tsx \
  crates/noema-core/web/src/components/transcript/Message.tsx \
  crates/noema-core/web/src/components/transcript/TypingMessage.tsx \
  crates/noema-core/web/src/components/transcript/markerModel.ts \
  crates/noema-core/web/src/components/transcript/ToolMarker.tsx
git diff --cached --stat
git diff --cached --name-status
```

Expected: only transcript files are staged. If there are staged fixes, commit:

```bash
git commit -m "chore: polish astryx transcript adoption"
```

If no validation fixes were needed after Task 4, do not create an empty commit.

## Self-Review Notes

- Spec coverage: message rows are covered by Task 2, typing row by Task 3,
  shared avatar preservation by Task 1, tool rows by Task 4, and validation by
  Task 5.
- Preserved scope: `Transcript`, `TranscriptScroller`, `TranscriptBottomFollower`,
  `renderModel.ts`, composer, memory markers, activity rows, structured cards,
  and error notices remain Noema-owned and unchanged unless TypeScript import
  cleanup is required.
- Deferred work: markdown/code rendering, date dividers/system messages, artifact
  side panel, and full `ChatLayout` scroll ownership remain future slices.
