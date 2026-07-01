# Astryx Web UI Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Noema web UI foundation with Astryx, StyleX, and a Noema-owned Neutral-derived theme while preserving Noema IA, shell behavior, chat behavior, GraphQL data flow, settings flows, and memory graph semantics.

**Architecture:** Install Astryx and StyleX first, then migrate from primitive dependencies upward into shell, setup, chat/transcript, settings, and memory surfaces. Prefer direct Astryx imports and Noema-owned StyleX domain components over shadcn-shaped wrappers. Finish by deleting shadcn/Base UI/Tailwind dependencies, config, utility classes, and old primitive files.

**Tech Stack:** React 19, TypeScript, Vite, Apollo Client, GraphQL, Astryx (`@astryxdesign/core`, `@astryxdesign/theme-neutral`, `@astryxdesign/cli`), StyleX, lucide-react, Bun.

---

## Approved Spec

- Design spec: `docs/superpowers/specs/2026-07-01-astryx-web-ui-migration-design.md`
- Product docs to keep aligned:
  - `docs/frontend/current-contract.md`
  - `docs/context/current.md`

## Scope Check

This plan covers one frontend foundation replacement, not multiple independent
subsystems. The work touches many surfaces, but the implementation order keeps
the task testable after each milestone:

1. Astryx and StyleX foundation.
2. Domain-owned replacement primitives.
3. Shell and setup migration.
4. Chat and transcript migration.
5. Settings and memory migration.
6. Dependency/config cleanup and documentation.

## File Structure

### Create

- `crates/noema-core/web/src/theme/noema-neutral.css`
  - Noema-owned Astryx Neutral-derived theme file. Start from generated Astryx Neutral and replace values with Noema colors/fonts.
- `crates/noema-core/web/src/components/transcript/TranscriptAttachmentCard.tsx`
  - Replaces shadcn/Base UI attachment wrappers with a Noema transcript-domain card.
- `crates/noema-core/web/src/components/transcript/TranscriptMarkerFrame.tsx`
  - Replaces marker wrappers for memory/tool/error rows.
- `crates/noema-core/web/src/components/transcript/TranscriptScroller.tsx`
  - Owns transcript scrolling layout after removing `@shadcn/react/message-scroller`.

### Modify

- `crates/noema-core/web/package.json`
  - Add Astryx and StyleX packages. Remove shadcn/Base UI/Tailwind packages at cleanup.
- `crates/noema-core/web/bun.lock`
  - Updated by `bun install`, `bun add`, and `bun remove`.
- `crates/noema-core/web/vite.config.ts`
  - Add StyleX Babel transform through the React plugin. Keep the Tailwind Vite plugin until Task 9 because current JSX still relies on Tailwind utilities.
- `crates/noema-core/web/vite.desktop.config.ts`
  - Same StyleX addition as browser Vite config, preserving the Tailwind plugin and desktop asset rewrite plugin until Task 9.
- `crates/noema-core/web/src/styles.css`
  - Add Astryx imports and the Noema theme while keeping Tailwind/shadcn imports and token aliases until JSX utility classes are migrated; later cleanup reduces this file to Astryx imports, root sizing, Tauri transparency, and global animations only.
- `crates/noema-core/web/src/main.tsx`
  - Keep importing `./styles.css`; no app state changes.
- `crates/noema-core/web/src/lib/utils.ts`
  - Delete after all `cn`/Tailwind merge usage is removed.
- `crates/noema-core/web/components.json`
  - Delete during cleanup.
- `crates/noema-core/web/src/components/ui/*`
  - Delete during cleanup after call sites are migrated.
- `crates/noema-core/web/src/App.tsx`
- `crates/noema-core/web/src/components/ChatSurface.tsx`
- `crates/noema-core/web/src/components/Composer.tsx`
- `crates/noema-core/web/src/components/EmptyState.tsx`
- `crates/noema-core/web/src/components/ErrorMarker.tsx`
- `crates/noema-core/web/src/components/IdentityAvatar.tsx`
- `crates/noema-core/web/src/components/onboarding/AuthAttempt.tsx`
- `crates/noema-core/web/src/components/onboarding/Onboarding.tsx`
- `crates/noema-core/web/src/components/onboarding/ProviderStatus.tsx`
- `crates/noema-core/web/src/components/shell/AppShell.tsx`
- `crates/noema-core/web/src/components/shell/SetupFrame.tsx`
- `crates/noema-core/web/src/components/shell/ShellAttentionItem.tsx`
- `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
- `crates/noema-core/web/src/components/transcript/*.tsx`
- `crates/noema-core/web/src/components/settings/*.tsx`
- `crates/noema-core/web/src/components/memory/*.tsx`
- `crates/noema-core/web/src/pages/*.tsx`
  - Replace `@/components/ui` imports, Tailwind class strings, and `cn` usage with Astryx components and StyleX.
- `docs/frontend/current-contract.md`
  - Replace the shadcn/Base UI frontend foundation note with the Astryx/StyleX direction.
- `docs/context/current.md`
  - Record the settled Astryx web UI direction once implementation is complete.

### Do Not Modify

- `crates/noema-core/web/src/graphql/*`
- `crates/noema-core/web/src/generated/*`
- Rust backend files, unless `bun run gen:types` proves a generated type export issue exists.

## Global Implementation Rules

- Work on `main`.
- Preserve unrelated dirty changes.
- Do not run browser tooling unless the user explicitly asks.
- Do not write new UI tests unless the user explicitly asks.
- Commit after each task that leaves the app compiling or intentionally after an inventory/docs task.
- After every task, run:

```bash
cd crates/noema-core/web
bun run lint
```

- After any task that changes bundling, styles, imports, or package dependencies, also run:

```bash
cd crates/noema-core/web
bun run build
```

---

### Task 1: Capture Baseline And Install Astryx Foundation

**Files:**
- Modify: `crates/noema-core/web/package.json`
- Modify: `crates/noema-core/web/bun.lock`
- Modify: `crates/noema-core/web/vite.config.ts`
- Modify: `crates/noema-core/web/vite.desktop.config.ts`
- Create: `crates/noema-core/web/src/theme/noema-neutral.css`
- Modify: `crates/noema-core/web/src/styles.css`

- [ ] **Step 1: Confirm starting status**

Run:

```bash
git status --short --branch
```

Expected: report unrelated dirty files and do not stage them.

- [ ] **Step 2: Install Astryx and StyleX packages**

Run:

```bash
cd crates/noema-core/web
bun add @astryxdesign/core @astryxdesign/theme-neutral @stylexjs/stylex
bun add -d @astryxdesign/cli @stylexjs/babel-plugin
```

Expected: `package.json` and `bun.lock` update. If package resolution fails because of network sandboxing, rerun the same command with escalation.

- [ ] **Step 3: Add the Astryx CLI package script**

Modify `crates/noema-core/web/package.json` scripts to include:

```json
"astryx": "node node_modules/@astryxdesign/cli/bin/astryx.mjs"
```

Expected: existing scripts remain unchanged.

- [ ] **Step 4: Generate or copy the Neutral theme**

Run:

```bash
cd crates/noema-core/web
bun run astryx theme add neutral
```

Expected: Astryx writes an editable Neutral theme file. Move that generated theme to:

```text
crates/noema-core/web/src/theme/noema-neutral.css
```

If the CLI writes a different path, keep its generated CSS variable names and place the final file at `src/theme/noema-neutral.css`.

- [ ] **Step 5: Apply Noema values to the theme**

Edit `crates/noema-core/web/src/theme/noema-neutral.css`. Preserve Astryx variable names and replace values with this Noema baseline:

```css
:root {
  --noema-paper-50: #fcfaf5;
  --noema-paper-100: #f7f2e8;
  --noema-paper-200: #efe7d7;
  --noema-ink-900: #17160f;
  --noema-ink-600: #5e5a4b;
  --noema-ink-500: #827d6b;
  --noema-ink-400: #a8a38e;
  --noema-white: #ffffff;
  --noema-pine-50: #e9f2ec;
  --noema-pine-100: #c9e4d6;
  --noema-pine-500: #1f7a57;
  --noema-pine-600: #176046;
  --noema-pine-700: #114a37;
  --noema-clay-50: #fbede3;
  --noema-clay-600: #bc4e2b;
  --noema-red-100: #f8dbd3;
  --noema-red-700: #8f2a1c;
  --noema-blue-100: #d7e8ef;
  --noema-blue-700: #1d4a60;
  --noema-font-display: "Bricolage Grotesque", "Hanken Grotesk", system-ui, sans-serif;
  --noema-font-body: "Hanken Grotesk", system-ui, -apple-system, sans-serif;
  --noema-font-mono: "JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace;
}
```

Map Astryx semantic background, foreground, surface, border, primary, accent, danger, info, radius, and typography tokens to those Noema variables in the same file.

- [ ] **Step 6: Add StyleX Babel transform while keeping Tailwind active**

Edit `crates/noema-core/web/vite.config.ts` to keep `@tailwindcss/vite` and add StyleX through the React plugin:

```ts
import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const stylexPlugin = [
  "@stylexjs/babel-plugin",
  {
    dev: process.env.NODE_ENV !== "production",
    runtimeInjection: true,
    treeshakeCompensation: true,
    unstable_moduleResolution: {
      type: "commonJS",
      rootDir: __dirname
    }
  }
];

export default defineConfig({
  plugins: [
    react({
      babel: {
        plugins: [stylexPlugin]
      }
    }),
    tailwindcss()
  ],
  base: "/assets/",
  publicDir: "public",
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src")
    }
  },
  build: {
    outDir: "../src/daemon/web/assets",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: "app.js",
        chunkFileNames: "[name].js",
        assetFileNames: (assetInfo) => {
          if (assetInfo.names.some((name) => name.endsWith(".css"))) {
            return "styles.css";
          }
          return "[name][extname]";
        }
      }
    }
  }
});
```

- [ ] **Step 7: Apply the same StyleX transform to desktop Vite config**

Edit `crates/noema-core/web/vite.desktop.config.ts` to keep Tailwind and the desktop asset plugin while adding StyleX:

```ts
const stylexPlugin = [
  "@stylexjs/babel-plugin",
  {
    dev: process.env.NODE_ENV !== "production",
    runtimeInjection: true,
    treeshakeCompensation: true,
    unstable_moduleResolution: {
      type: "commonJS",
      rootDir: __dirname
    }
  }
];
```

Use `react({ babel: { plugins: [stylexPlugin] } })` as the first plugin, followed by `tailwindcss()`, followed by the existing `noema-desktop-public-assets` plugin unchanged.

- [ ] **Step 8: Add Astryx global CSS imports without removing Tailwind yet**

Edit the top of `crates/noema-core/web/src/styles.css` so it keeps the existing Tailwind/shadcn imports and then imports Astryx:

```css
@import "tailwindcss";
@import "tw-animate-css";
@import "shadcn/tailwind.css";
@import "@astryxdesign/core/reset.css";
@import "@astryxdesign/core/astryx.css";
@import "@astryxdesign/theme-neutral/theme.css";
@import "./theme/noema-neutral.css";
```

Keep the existing Tailwind bridge until Task 9:

```css
@custom-variant dark (&:is(.dark *));
@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --font-heading: var(--font-display);
  --font-sans: var(--font-body);
  --radius-sm: calc(var(--radius) * 0.6);
  --radius-md: calc(var(--radius) * 0.8);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) * 1.4);
  --radius-2xl: calc(var(--radius) * 1.8);
  --radius-3xl: calc(var(--radius) * 2.2);
  --radius-4xl: calc(var(--radius) * 2.6);
}
```

Keep root height, box sizing, Tauri transparency, and current keyframes for now. Remove the Tailwind imports, Tailwind Vite plugin, `@custom-variant`, and `@theme inline` only in Task 9 after JSX utility classes have been migrated.

- [ ] **Step 9: Validate foundation compile**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
```

Expected: both commands pass before committing. Tailwind class strings may still exist in JSX and must continue to render because Tailwind remains active until Task 9.

- [ ] **Step 10: Commit foundation**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/package.json crates/noema-core/web/bun.lock crates/noema-core/web/vite.config.ts crates/noema-core/web/vite.desktop.config.ts crates/noema-core/web/src/styles.css crates/noema-core/web/src/theme/noema-neutral.css
git diff --cached --stat
git diff --cached --name-status
git commit -m "feat: add astryx web foundation"
```

Expected: commit contains only foundation package/config/theme/style changes.

---

### Task 2: Replace Shared Noema Domain Primitives

**Files:**
- Create: `crates/noema-core/web/src/components/transcript/TranscriptAttachmentCard.tsx`
- Create: `crates/noema-core/web/src/components/transcript/TranscriptMarkerFrame.tsx`
- Create: `crates/noema-core/web/src/components/transcript/TranscriptScroller.tsx`
- Modify: `crates/noema-core/web/src/components/transcript/index.ts`
- Modify: transcript files that import `@/components/ui/attachment`, `@/components/ui/marker`, `@/components/ui/message`, `@/components/ui/bubble`, or `@/components/ui/message-scroller`

- [ ] **Step 1: Create transcript marker frame**

Create `crates/noema-core/web/src/components/transcript/TranscriptMarkerFrame.tsx`:

```tsx
import type { ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";

type TranscriptMarkerTone = "muted" | "success" | "warning" | "error";

type TranscriptMarkerFrameProps = {
  tone?: TranscriptMarkerTone;
  pending?: boolean;
  icon?: ReactNode;
  children: ReactNode;
};

const styles = stylex.create({
  root: {
    display: "inline-flex",
    maxWidth: "100%",
    alignItems: "center",
    gap: 8,
    borderRadius: 999,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    backgroundColor: "var(--noema-surface-card)",
    paddingBlock: 5,
    paddingInline: 10,
    fontSize: 12,
    lineHeight: 1.35,
    color: "var(--noema-text-secondary)"
  },
  success: {
    color: "var(--noema-pine-700)",
    backgroundColor: "color-mix(in srgb, var(--noema-pine-50) 80%, transparent)"
  },
  warning: {
    color: "var(--noema-clay-600)",
    backgroundColor: "color-mix(in srgb, var(--noema-clay-50) 82%, transparent)"
  },
  error: {
    color: "var(--noema-red-700)",
    backgroundColor: "color-mix(in srgb, var(--noema-red-100) 66%, transparent)"
  },
  icon: {
    display: "inline-flex",
    flexShrink: 0
  },
  content: {
    minWidth: 0,
    overflowWrap: "anywhere"
  },
  pending: {
    opacity: 0.76
  }
});

export function TranscriptMarkerFrame({
  tone = "muted",
  pending = false,
  icon,
  children
}: TranscriptMarkerFrameProps) {
  return (
    <span
      {...stylex.props(
        styles.root,
        tone === "success" && styles.success,
        tone === "warning" && styles.warning,
        tone === "error" && styles.error,
        pending && styles.pending
      )}
      data-pending={pending ? "true" : undefined}
    >
      {icon ? <span {...stylex.props(styles.icon)}>{icon}</span> : null}
      <span {...stylex.props(styles.content)}>{children}</span>
    </span>
  );
}
```

- [ ] **Step 2: Create transcript attachment card**

Create `crates/noema-core/web/src/components/transcript/TranscriptAttachmentCard.tsx`:

```tsx
import type { ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";

type TranscriptAttachmentCardProps = {
  title: ReactNode;
  description?: ReactNode;
  meta?: ReactNode;
  icon?: ReactNode;
  children?: ReactNode;
};

const styles = stylex.create({
  root: {
    display: "grid",
    maxWidth: "100%",
    minWidth: 0,
    gap: 8,
    borderRadius: 8,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    backgroundColor: "var(--noema-surface-card)",
    padding: 12
  },
  header: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr)",
    alignItems: "start",
    gap: 10
  },
  icon: {
    color: "var(--noema-text-accent)"
  },
  title: {
    minWidth: 0,
    margin: 0,
    fontSize: 14,
    fontWeight: 600,
    color: "var(--noema-text-primary)",
    overflowWrap: "anywhere"
  },
  description: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 13,
    overflowWrap: "anywhere"
  },
  meta: {
    color: "var(--noema-text-muted)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 11,
    overflowWrap: "anywhere"
  }
});

export function TranscriptAttachmentCard({
  title,
  description,
  meta,
  icon,
  children
}: TranscriptAttachmentCardProps) {
  return (
    <article {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}>
        <span {...stylex.props(styles.icon)}>{icon}</span>
        <div>
          <h3 {...stylex.props(styles.title)}>{title}</h3>
          {description ? <p {...stylex.props(styles.description)}>{description}</p> : null}
          {meta ? <div {...stylex.props(styles.meta)}>{meta}</div> : null}
        </div>
      </div>
      {children}
    </article>
  );
}
```

- [ ] **Step 3: Create transcript scroller**

Create `crates/noema-core/web/src/components/transcript/TranscriptScroller.tsx`:

```tsx
import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { Button } from "@astryxdesign/core/Button";
import { ArrowDownIcon } from "lucide-react";

type TranscriptScrollerProps = {
  children: React.ReactNode;
  onStickinessChange?: (stuckToBottom: boolean) => void;
};

const styles = stylex.create({
  viewport: {
    position: "relative",
    minHeight: 0,
    height: "100%",
    overflowY: "auto",
    overscrollBehavior: "contain"
  },
  content: {
    display: "flex",
    minHeight: "100%",
    flexDirection: "column",
    justifyContent: "flex-end",
    gap: 12,
    width: "var(--chat-column-width)",
    marginInline: "auto",
    paddingBlock: 24,
    paddingInline: 2
  },
  scrollButton: {
    position: "absolute",
    left: "50%",
    bottom: 16,
    transform: "translateX(-50%)"
  },
  hidden: {
    display: "none"
  }
});

export function TranscriptScroller({ children, onStickinessChange }: TranscriptScrollerProps) {
  const viewportRef = React.useRef<HTMLDivElement | null>(null);
  const [stuckToBottom, setStuckToBottom] = React.useState(true);

  const updateStickiness = React.useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    const distance = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
    const next = distance < 80;
    setStuckToBottom(next);
    onStickinessChange?.(next);
  }, [onStickinessChange]);

  React.useLayoutEffect(() => {
    if (stuckToBottom) {
      viewportRef.current?.scrollTo({ top: viewportRef.current.scrollHeight });
    }
  }, [children, stuckToBottom]);

  return (
    <div ref={viewportRef} {...stylex.props(styles.viewport)} onScroll={updateStickiness}>
      <div {...stylex.props(styles.content)}>{children}</div>
      <div {...stylex.props(styles.scrollButton, stuckToBottom && styles.hidden)}>
        <Button
          label="Scroll to latest"
          icon={<ArrowDownIcon aria-hidden="true" />}
          onClick={() => viewportRef.current?.scrollTo({ top: viewportRef.current.scrollHeight, behavior: "smooth" })}
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Replace marker call sites**

Modify:

- `crates/noema-core/web/src/components/ErrorMarker.tsx`
- `crates/noema-core/web/src/components/transcript/MemoryMarker.tsx`
- `crates/noema-core/web/src/components/transcript/ToolMarker.tsx`

Replace imports like:

```tsx
import { Marker, MarkerContent, MarkerIcon } from "@/components/ui/marker";
```

with:

```tsx
import { TranscriptMarkerFrame } from "@/components/transcript/TranscriptMarkerFrame";
```

Render `TranscriptMarkerFrame` with `tone`, `pending`, and `icon` props. Preserve all text and expansion behavior.

- [ ] **Step 5: Replace attachment call sites**

Modify:

- `crates/noema-core/web/src/components/transcript/ActivityRow.tsx`
- `crates/noema-core/web/src/components/transcript/MemoryDetailAttachment.tsx`
- `crates/noema-core/web/src/components/transcript/MemoryStructuredCard.tsx`
- `crates/noema-core/web/src/components/transcript/StructuredCard.tsx`
- `crates/noema-core/web/src/components/transcript/ToolDetailAttachment.tsx`

Replace `@/components/ui/attachment` imports with `TranscriptAttachmentCard`. Preserve title, detail rows, metadata, and structured-card payload display.

- [ ] **Step 6: Replace message/scroller call sites**

Modify:

- `crates/noema-core/web/src/components/transcript/Transcript.tsx`
- `crates/noema-core/web/src/components/transcript/TranscriptBottomFollower.tsx`
- `crates/noema-core/web/src/components/transcript/TranscriptRow.tsx`
- `crates/noema-core/web/src/components/transcript/Message.tsx`
- `crates/noema-core/web/src/components/transcript/TypingMessage.tsx`

Use `TranscriptScroller` for viewport/content and ordinary StyleX layouts for rows and bubbles. Preserve:

```text
human lane aligned end
assistant lane aligned start
avatar visibility
arrival animation data attributes
bottom follower visibility
typing indicator timing
```

- [ ] **Step 7: Export new transcript components**

Update `crates/noema-core/web/src/components/transcript/index.ts`:

```ts
export { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
export { TranscriptMarkerFrame } from "./TranscriptMarkerFrame";
export { TranscriptScroller } from "./TranscriptScroller";
```

Keep existing exports.

- [ ] **Step 8: Validate shared primitive replacement**

Run:

```bash
cd crates/noema-core/web
bun run lint
bun run build
rg "@/components/ui/(attachment|marker|message|bubble|message-scroller)" src
```

Expected:

```text
no matches for the rg command
```

- [ ] **Step 9: Commit transcript primitive replacement**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/components/ErrorMarker.tsx crates/noema-core/web/src/components/transcript
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: replace transcript ui primitives with stylex"
```

Expected: commit includes transcript-domain replacement components and their call sites.

---

### Task 3: Migrate Onboarding, Setup, And Empty States

**Files:**
- Modify: `crates/noema-core/web/src/App.tsx`
- Modify: `crates/noema-core/web/src/components/EmptyState.tsx`
- Modify: `crates/noema-core/web/src/components/IdentityAvatar.tsx`
- Modify: `crates/noema-core/web/src/components/onboarding/AuthAttempt.tsx`
- Modify: `crates/noema-core/web/src/components/onboarding/Onboarding.tsx`
- Modify: `crates/noema-core/web/src/components/onboarding/ProviderStatus.tsx`
- Modify: `crates/noema-core/web/src/components/shell/SetupFrame.tsx`

- [ ] **Step 1: Replace imports**

Use Astryx imports:

```tsx
import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
```

Only import components that each file uses.

- [ ] **Step 2: Add StyleX styles per file**

At the top of each modified component that has layout classes, add:

```tsx
import * as stylex from "@stylexjs/stylex";
```

Convert Tailwind `className` strings into `stylex.create` objects in the same file. Keep component-specific styles local to the component file.

- [ ] **Step 3: Preserve onboarding behavior**

Do not change these props or callbacks:

```tsx
onConnectProvider
authAttempt
onboardingError
status
complete
```

Keep the first-run copy, provider status display, auth code display, and automatic continuation message.

- [ ] **Step 4: Validate no old UI imports in setup/onboarding**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui|className=" src/App.tsx src/components/EmptyState.tsx src/components/IdentityAvatar.tsx src/components/onboarding src/components/shell/SetupFrame.tsx
bun run lint
bun run build
```

Expected: `rg` reports no `@/components/ui` imports. Remaining `className` output is allowed only on third-party SVG/icon components or generated graph-library elements.

- [ ] **Step 5: Commit setup/onboarding migration**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/App.tsx crates/noema-core/web/src/components/EmptyState.tsx crates/noema-core/web/src/components/IdentityAvatar.tsx crates/noema-core/web/src/components/onboarding crates/noema-core/web/src/components/shell/SetupFrame.tsx
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: migrate onboarding surfaces to astryx"
```

---

### Task 4: Migrate Shell And Navigation

**Files:**
- Modify: `crates/noema-core/web/src/components/shell/AppShell.tsx`
- Modify: `crates/noema-core/web/src/components/shell/ShellSidebar.tsx`
- Modify: `crates/noema-core/web/src/components/shell/ShellAttentionItem.tsx`
- Modify: `crates/noema-core/web/src/components/shell/deckNavigation.ts`
- Modify: `crates/noema-core/web/src/components/shell/shellNavigation.ts`
- Modify: `crates/noema-core/web/src/styles.css`

- [ ] **Step 1: Replace shell commodity controls**

Replace:

```tsx
import { Button } from "@/components/ui/button";
```

with:

```tsx
import { Button } from "@astryxdesign/core/Button";
```

Use Astryx list/item/navigation components where available. Keep route state and deck navigation data unchanged.

- [ ] **Step 2: Convert shell layout classes to StyleX**

Move shell-specific classes into `stylex.create` in `AppShell.tsx` and `ShellSidebar.tsx`. Preserve data attributes used by deck transitions:

```text
data-shell-menu-frame-state
data-shell-menu-transition-direction
data-shell-surface-visibility
```

- [ ] **Step 3: Keep shell transition keyframes global**

In `crates/noema-core/web/src/styles.css`, keep only the global keyframes/selectors that depend on data attributes spanning multiple shell components:

```css
[data-slot="shell-sidebar-menu-level-frame"] {
  will-change: transform, opacity;
}

@keyframes shell-sidebar-menu-enter-forward {
  from {
    opacity: 0.4;
    transform: translateX(100%);
  }

  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes shell-sidebar-menu-exit-forward {
  from {
    opacity: 1;
    transform: translateX(0);
  }

  to {
    opacity: 0;
    transform: translateX(-22%);
  }
}

@keyframes shell-sidebar-menu-enter-backward {
  from {
    opacity: 0.4;
    transform: translateX(-22%);
  }

  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes shell-sidebar-menu-exit-backward {
  from {
    opacity: 1;
    transform: translateX(0);
  }

  to {
    opacity: 0;
    transform: translateX(100%);
  }
}
```

All static layout, spacing, color, and typography rules move to StyleX.

- [ ] **Step 4: Validate route-derived shell behavior at source level**

Run:

```bash
cd crates/noema-core/web
rg "settings|providers|agents|mcps|trusted|approvals|audit" src/components/shell src/routes.ts
rg "@/components/ui|className=" src/components/shell
bun run lint
bun run build
```

Expected: navigation route strings remain present. `@/components/ui` has no matches.

- [ ] **Step 5: Commit shell migration**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/components/shell crates/noema-core/web/src/styles.css
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: migrate shell navigation to astryx"
```

---

### Task 5: Migrate Composer And Chat Surface

**Files:**
- Modify: `crates/noema-core/web/src/components/ChatSurface.tsx`
- Modify: `crates/noema-core/web/src/components/Composer.tsx`
- Modify: `crates/noema-core/web/src/components/MessageTextAnimation.tsx`
- Modify: `crates/noema-core/web/src/styles.css`

- [ ] **Step 1: Replace composer button and textarea imports**

In `Composer.tsx`, replace:

```tsx
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
```

with Astryx field/button imports:

```tsx
import { Button } from "@astryxdesign/core/Button";
import { Textarea } from "@astryxdesign/core/Textarea";
```

If Astryx exposes the multiline field under a different component name, use the Astryx component documented by `bun run astryx component Textarea`.

- [ ] **Step 2: Preserve composer sizing algorithm**

Do not change:

```tsx
measureTextHeight
syncHeight
canSend
onSubmit
onDraftChange
```

Keep the canvas-based height measurement and mobile submit-button accommodation.

- [ ] **Step 3: Convert chat layout to StyleX**

Convert Tailwind classes in `ChatSurface.tsx`, `Composer.tsx`, and `MessageTextAnimation.tsx` to local StyleX styles. Preserve CSS custom property:

```css
--chat-column-width
```

Set it on the chat surface root through StyleX:

```tsx
const styles = stylex.create({
  root: {
    "--chat-column-width": "min(860px, calc(100% - 48px))",
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr) auto",
    minHeight: 0,
    height: "100%",
    width: "100%",
    overflow: "hidden",
    paddingBottom: 22
  },
  mobileRoot: {
    "--chat-column-width": "calc(100% - 40px)"
  }
});
```

- [ ] **Step 4: Keep global mobile quirk rules**

In `styles.css`, keep the mobile input font-size rule and data-slot composer submit positioning until equivalent StyleX responsive rules are in place:

```css
@media (hover: none) and (pointer: coarse) {
  input,
  select,
  textarea {
    font-size: max(16px, 1em);
  }
}
```

Move component-specific composer placement into `Composer.tsx`.

- [ ] **Step 5: Validate chat source**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui|className=" src/components/ChatSurface.tsx src/components/Composer.tsx src/components/MessageTextAnimation.tsx
bun run lint
bun run build
```

Expected: no `@/components/ui` matches. Remaining `className` is limited to icon elements if present.

- [ ] **Step 6: Commit chat composer migration**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/components/ChatSurface.tsx crates/noema-core/web/src/components/Composer.tsx crates/noema-core/web/src/components/MessageTextAnimation.tsx crates/noema-core/web/src/styles.css
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: migrate chat composer to astryx"
```

---

### Task 6: Migrate Settings Provider And Agent Panes

**Files:**
- Modify: `crates/noema-core/web/src/pages/SettingsPage.tsx`
- Modify: `crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx`

- [ ] **Step 1: Replace UI imports**

Replace `@/components/ui/button` and `@/components/ui/badge` imports with:

```tsx
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
```

Use Astryx list/card/form components where they reduce local markup without changing GraphQL state.

- [ ] **Step 2: Preserve provider and agent GraphQL behavior**

Do not change prop names, generated GraphQL types, mutation inputs, query variables, retry callbacks, or provider/model preference state.

- [ ] **Step 3: Convert Settings layout to StyleX**

Convert Tailwind class strings in the listed files into local `stylex.create` blocks. Preserve:

```text
loading state
error retry state
empty state
provider auth metadata display
agent provider/model preference editor
save pending/disabled states
```

- [ ] **Step 4: Validate provider/agent panes**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui|className=" src/pages/SettingsPage.tsx src/components/settings/ProvidersSettingsPane.tsx src/components/settings/ProvidersSettingsPaneContent.tsx src/components/settings/AgentsSettingsPane.tsx src/components/settings/AgentsSettingsPaneContent.tsx
bun run lint
bun run build
```

Expected: no `@/components/ui` matches.

- [ ] **Step 5: Commit provider/agent settings migration**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/pages/SettingsPage.tsx crates/noema-core/web/src/components/settings/ProvidersSettingsPane.tsx crates/noema-core/web/src/components/settings/ProvidersSettingsPaneContent.tsx crates/noema-core/web/src/components/settings/AgentsSettingsPane.tsx crates/noema-core/web/src/components/settings/AgentsSettingsPaneContent.tsx
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: migrate provider agent settings to astryx"
```

---

### Task 7: Migrate MCP, Trusted Identity, Approval, And Audit Settings

**Files:**
- Modify: `crates/noema-core/web/src/components/settings/McpServerSetupFlow.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpToolPermissionsFooter.tsx`
- Modify: `crates/noema-core/web/src/components/settings/McpToolPermissionsModal.tsx`
- Modify: `crates/noema-core/web/src/components/settings/TrustedIdentitiesSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/TrustedIdentitiesSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/ApprovalsSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/ApprovalsSettingsPaneContent.tsx`
- Modify: `crates/noema-core/web/src/components/settings/AuditSettingsPane.tsx`
- Modify: `crates/noema-core/web/src/components/settings/AuditSettingsPaneContent.tsx`

- [ ] **Step 1: Replace modal, tabs, item, badge, and button imports**

Use Astryx imports:

```tsx
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Dialog } from "@astryxdesign/core/Dialog";
import { Tabs } from "@astryxdesign/core/Tabs";
```

For old `Item`, `ItemContent`, `ItemTitle`, and `ItemActions`, use Astryx list/item components if present. If Astryx has no matching item component, replace them with semantic `article`, `header`, and `div` elements styled with StyleX inside the settings component.

- [ ] **Step 2: Preserve MCP setup form state**

Do not change:

```text
mcpSetupForm.ts helper contracts
transport/auth tab state
OAuth/auth-required retry flow
server creation mutation input
tool permissions draft state
autofill suggestion state
batch save behavior
delete confirmation behavior
```

- [ ] **Step 3: Split large files only where the migration touches tangled UI**

`McpToolPermissionsModal.tsx` is over 750 lines. If the file remains over 750 lines after migration, split these UI-only sections into nearby files:

```text
McpToolPermissionRow.tsx
McpToolOwnerResolutionFields.tsx
McpToolSchemaPreview.tsx
```

Keep state ownership in `McpToolPermissionsModal.tsx`.

- [ ] **Step 4: Convert settings styles to StyleX**

Move Tailwind layout, border, typography, and color classes into local StyleX styles. Keep ordinary `className` only where a third-party component requires it.

- [ ] **Step 5: Validate settings surfaces**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui" src/components/settings
rg "className=" src/components/settings
bun run lint
bun run build
```

Expected: no `@/components/ui` matches. Review each remaining `className=` line and keep only third-party or native browser-required uses.

- [ ] **Step 6: Commit MCP and remaining settings migration**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/components/settings
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: migrate mcp settings to astryx"
```

---

### Task 8: Migrate Memory Pages And Graph Controls

**Files:**
- Modify: `crates/noema-core/web/src/pages/MemoryHomePage.tsx`
- Modify: `crates/noema-core/web/src/pages/MemoryGraphPage.tsx`
- Modify: `crates/noema-core/web/src/components/memory/MemoryGraphFlow.tsx`
- Modify: `crates/noema-core/web/src/components/memory/MemoryGraphControls.tsx`
- Modify: `crates/noema-core/web/src/components/memory/MemoryGraphDetailPanel.tsx`
- Modify: `crates/noema-core/web/src/components/memory/MemoryEntityNode.tsx`
- Modify: `crates/noema-core/web/src/components/memory/MemoryClaimEdge.tsx`
- Modify: `crates/noema-core/web/src/components/memory/MemoryGraphCanvas.tsx`

- [ ] **Step 1: Replace Memory Home button/card usage**

Replace:

```tsx
import { Button } from "@/components/ui/button";
```

with:

```tsx
import { Button } from "@astryxdesign/core/Button";
```

Use Astryx card/list components for the Memory Graph entry if available.

- [ ] **Step 2: Preserve graph library class names**

Keep React Flow required class names:

```tsx
className="react-flow__edge-interaction"
className="react-flow__edge-path"
```

Move Noema-owned layout classes around the graph into StyleX.

- [ ] **Step 3: Convert graph control fields**

Replace manual search field styling with Astryx field components where compatible. Preserve:

```text
filter text state
reset/fit actions
selected edge detail behavior
detail loading/error states
redaction and evidence count display
```

- [ ] **Step 4: Validate memory surfaces**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui" src/pages/MemoryHomePage.tsx src/pages/MemoryGraphPage.tsx src/components/memory
rg "className=" src/pages/MemoryHomePage.tsx src/pages/MemoryGraphPage.tsx src/components/memory
bun run lint
bun run build
```

Expected: no `@/components/ui` matches. Remaining `className` lines are limited to React Flow class names or icon class names.

- [ ] **Step 5: Commit memory migration**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/src/pages/MemoryHomePage.tsx crates/noema-core/web/src/pages/MemoryGraphPage.tsx crates/noema-core/web/src/components/memory
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: migrate memory surfaces to astryx"
```

---

### Task 9: Delete Old UI Foundation

**Files:**
- Delete: `crates/noema-core/web/components.json`
- Delete: `crates/noema-core/web/src/components/ui/*`
- Delete: `crates/noema-core/web/src/lib/utils.ts`
- Modify: `crates/noema-core/web/package.json`
- Modify: `crates/noema-core/web/bun.lock`
- Modify: `crates/noema-core/web/src/styles.css`

- [ ] **Step 1: Verify no imports remain**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui|@base-ui/react|@shadcn/react|shadcn|tailwind-merge|class-variance-authority|tw-animate-css|@tailwindcss/vite|tailwindcss" .
```

Expected: matches are limited to `package.json`, `bun.lock`, `components.json`, deleted-target files, and historical docs outside `crates/noema-core/web`.

- [ ] **Step 2: Delete old config and primitive files**

Run:

```bash
git rm crates/noema-core/web/components.json
git rm -r crates/noema-core/web/src/components/ui
git rm crates/noema-core/web/src/lib/utils.ts
```

Expected: all old shadcn/Base UI primitive files are removed.

- [ ] **Step 3: Remove old packages**

Run:

```bash
cd crates/noema-core/web
bun remove @base-ui/react @shadcn/react @tailwindcss/vite class-variance-authority shadcn tailwind-merge tailwindcss tw-animate-css
```

If `next-themes` is used only by deleted `components/ui/sonner.tsx`, remove it too:

```bash
cd crates/noema-core/web
bun remove next-themes
```

Keep `clsx` only if source still imports it after `src/lib/utils.ts` is gone.

- [ ] **Step 4: Verify final source scan**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui|@base-ui/react|@shadcn/react|shadcn|tailwind-merge|class-variance-authority|tw-animate-css|@tailwindcss/vite|tailwindcss|from \"@/lib/utils\"|from '@/lib/utils'" src package.json vite.config.ts vite.desktop.config.ts
rg "className=" src
bun run lint
bun run build
bun run build:tauri
```

Expected:

- First `rg` command has no matches.
- Second `rg` command has only icon, React Flow, or third-party-required `className` uses.
- All validation commands pass.

- [ ] **Step 5: Commit cleanup**

Run:

```bash
git status --short --branch
git diff --check
git add crates/noema-core/web/package.json crates/noema-core/web/bun.lock crates/noema-core/web/vite.config.ts crates/noema-core/web/vite.desktop.config.ts crates/noema-core/web/src/styles.css
git add -u crates/noema-core/web/components.json crates/noema-core/web/src/components/ui crates/noema-core/web/src/lib/utils.ts
git diff --cached --stat
git diff --cached --name-status
git commit -m "refactor: remove shadcn tailwind web foundation"
```

---

### Task 10: Update Frontend Documentation And Durable Context

**Files:**
- Modify: `docs/frontend/current-contract.md`
- Modify: `docs/context/current.md`

- [ ] **Step 1: Update frontend contract**

In `docs/frontend/current-contract.md`, replace the frontend code organization paragraph that mentions shadcn-style primitive wrappers with:

```markdown
Noema-owned React product components should live one component per file.
Component folders may contain pure `.ts` helpers, shared type files, and nearby
tests. The web UI foundation is Astryx with a Noema-owned Neutral-derived theme
and StyleX for Noema-specific layout and state styling. Generic shadcn/Base UI
primitive wrappers are no longer part of the frontend contract; local UI
components should express Noema domain semantics rather than compatibility with
an old component library.
```

- [ ] **Step 2: Update durable context**

In `docs/context/current.md`, replace the settled decision that says:

```markdown
- The web UI uses shadcn/ui `base-rhea` components backed by Base UI, with
  Noema colors applied through local CSS tokens. Noema-owned shell and domain
  components remain responsible for chat, memory, provenance, approvals, tools,
  runs, and object detail semantics.
```

with:

```markdown
- The web UI uses Astryx as its component foundation, with a Noema-owned
  Neutral-derived theme and StyleX for Noema-specific layout and state styling.
  Noema-owned shell and domain components remain responsible for chat, memory,
  provenance, approvals, tools, runs, settings, and object detail semantics.
  shadcn/Base UI/Tailwind are no longer part of the frontend foundation.
```

- [ ] **Step 3: Validate docs and status**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: docs have no whitespace errors. Unrelated dirty files remain unstaged.

- [ ] **Step 4: Commit docs**

Run:

```bash
git add docs/frontend/current-contract.md docs/context/current.md
git diff --cached --stat
git diff --cached --name-status
git commit -m "docs: record astryx web ui foundation"
```

---

### Task 11: Final Validation And Handoff Notes

**Files:**
- No required source edits.

- [ ] **Step 1: Run final web validation**

Run:

```bash
cd crates/noema-core/web
bun run gen:types
bun run lint
bun run build
bun run build:tauri
```

Expected: all commands pass.

- [ ] **Step 2: Run final source scans**

Run:

```bash
cd crates/noema-core/web
rg "@/components/ui|@base-ui/react|@shadcn/react|shadcn|tailwind-merge|class-variance-authority|tw-animate-css|@tailwindcss/vite|tailwindcss|from \"@/lib/utils\"|from '@/lib/utils'" src package.json vite.config.ts vite.desktop.config.ts
rg "className=" src
```

Expected:

- First scan has no matches.
- Second scan has only React Flow class names, icon class names, or third-party-required class names. Each remaining line must be reviewed in the final summary.

- [ ] **Step 3: Run repository-level whitespace and status checks**

Run:

```bash
git status --short --branch
git diff --check
```

Expected: no whitespace errors. Report unrelated dirty files separately.

- [ ] **Step 4: Prepare final summary**

Report:

```text
Validation run:
- bun run gen:types
- bun run lint
- bun run build
- bun run build:tauri

Remaining className uses:
- list exact files and why each remains

Remaining unstaged/untracked files:
- list exact paths
```

Do not claim browser-verified visual correctness unless the user explicitly requested browser inspection and it was performed.
