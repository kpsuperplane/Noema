# Compact Chat Composer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the web chat composer one line by default, expandable for multi-line drafts, and replace the text send button with an accessible icon-only button.

**Architecture:** Keep the change local to `Composer.tsx`. Add small exported helpers for composer-specific textarea props and submit labels so focused tests can cover the behavior without pulling in a browser renderer.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, shadcn/base-rhea `Button` and `Textarea`, `lucide-react`, Bun test/lint/build.

---

## Scope Check

This is one cohesive frontend UI tweak. It does not change the GraphQL send
flow, transcript rendering, shared textarea primitive, or app routing.

## File Structure

- `crates/noema-core/web/src/components/Composer.tsx`: local composer sizing,
  accessible send label, and icon-only submit button.
- `crates/noema-core/web/src/components/Composer.test.ts`: focused helper tests
  for submit state, accessible labels, and one-line textarea sizing.

## Task 1: Compact Composer Controls

**Files:**
- Modify: `crates/noema-core/web/src/components/Composer.tsx`
- Test: `crates/noema-core/web/src/components/Composer.test.ts`

- [x] **Step 1: Write the failing tests**

Add tests for the desired helper behavior in
`crates/noema-core/web/src/components/Composer.test.ts`:

```ts
import {
  composerSubmitState,
  composerTextareaProps,
  isComposerTextareaDisabled,
  refocusComposerTextarea
} from "./Composer";

describe("composer submit presentation", () => {
  test("uses accessible labels for the icon-only send button", () => {
    assert.equal(composerSubmitState({ ready: true, value: "Hello", pending: false }).label, "Send message");
    assert.equal(composerSubmitState({ ready: true, value: "Hello", pending: true }).label, "Sending message");
  });
});

describe("composer textarea presentation", () => {
  test("starts as one line with local compact sizing", () => {
    const props = composerTextareaProps();

    assert.equal(props.rows, 1);
    assert.match(props.className, /min-h-9/);
    assert.match(props.className, /max-h-40/);
  });
});
```

- [x] **Step 2: Run the focused test and verify it fails**

Run:

```bash
bun test src/components/Composer.test.ts
```

Expected: failure because `composerTextareaProps` is not exported and submit
labels still return visible button text.

- [x] **Step 3: Implement the compact composer**

Update `crates/noema-core/web/src/components/Composer.tsx` to:

```tsx
import { SendHorizontal } from "lucide-react";

export function composerTextareaProps() {
  return {
    rows: 1,
    className: "min-h-9 max-h-40 py-2"
  };
}

export function composerSubmitState({
  ready,
  value,
  pending
}: {
  ready: boolean;
  value: string;
  pending: boolean;
}) {
  return {
    disabled: !ready || !value.trim(),
    label: pending ? "Sending message" : "Send message"
  };
}
```

Then pass the compact props into `<Textarea />` and render the icon button:

```tsx
const textareaProps = composerTextareaProps();

<Textarea
  ref={textareaRef}
  value={value}
  disabled={isComposerTextareaDisabled({ ready })}
  placeholder={placeholder}
  rows={textareaProps.rows}
  className={textareaProps.className}
  onChange={(event) => onChange(event.currentTarget.value)}
  onKeyDown={(event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  }}
/>
<Button
  type="submit"
  size="icon"
  aria-label={submitState.label}
  disabled={submitState.disabled}
>
  <SendHorizontal aria-hidden="true" />
</Button>
```

- [x] **Step 4: Run the focused test and verify it passes**

Run:

```bash
bun test src/components/Composer.test.ts
```

Expected: all Composer tests pass.

- [x] **Step 5: Run web validation**

Run from `crates/noema-core/web`:

```bash
bun run lint
bun run build
```

Expected: both commands complete without TypeScript, ESLint, or build errors.

- [x] **Step 6: Commit**

Run the ship checklist, then stage only the focused files and commit:

```bash
git status --short --branch
git diff --check
git add docs/superpowers/plans/2026-06-29-compact-chat-composer.md \
  crates/noema-core/web/src/components/Composer.tsx \
  crates/noema-core/web/src/components/Composer.test.ts
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: compact chat composer"
```
