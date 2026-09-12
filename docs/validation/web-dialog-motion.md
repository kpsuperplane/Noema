# Web dialog motion

Date: 2026-09-12

## Change

Native dialogs use the shared surface spring for entry and exit.
Opacity increases from zero to one. Scale increases from 0.97 to one.
The dialog moves upward by the spacing-2 token during entry.
Exit reverses these changes. The backdrop fades with the dialog.

Motion owns removal. The native dialog keeps its existing reference and focus behavior.
Closed dialogs are inert and hidden from accessibility tools during exit.
Native display rules prevent the header from taking focus before the dialog opens.
The overrides take precedence over Astryx's entry and visibility styles.

Conditional task, project, credential, and MCP dialogs retain their exit animation.
The web provider dialog remains a native dialog on phones.
Bottom sheets keep their existing implementation.

## Validation

The checked patch changes eight production files: 94 added lines and six removed lines.
No tests were added. Browser inspection made no product data changes.

- `bun run build`: passed for the final patch. Existing bundle size warnings remain.
- `bunx eslint src --max-warnings=0`: passed for the final TypeScript changes.
- `bun run lint`: blocked by the existing missing `enabled` field in
  `TaskModelPoolsSettings.tsx:100`. No dialog TypeScript errors remain.
- `bun run check:generated`: passed. Reused because this patch changes no generated inputs.
- `git diff --check`: passed.

Live Chromium inspection used the private read-only socket.
The web provider dialog was inspected at 1440 and 390 pixels wide.
Entry opacity increased. Exit opacity decreased without restarting entry.
Escape and Cancel closed the dialog. Focus returned to the opening button.
Repeated opening and closing worked. Reopening during exit also worked.
Reduced motion showed and removed the dialog immediately.
The Clients bottom sheet opened, closed, and reopened at 390 pixels wide.
Screenshots remained outside the repository.
