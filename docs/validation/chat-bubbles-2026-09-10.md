# Assistant paragraph bubbles

Base revision: `8f991272`.

The model followed the existing prompt by separating the greeting and question with a blank line.
The web display put both paragraphs inside one bubble.
Web Chat now separates ordinary assistant paragraphs for display.
It reuses the existing bubble groups, spacing, avatar, and attachment behavior.
Saved text and prompts remain unchanged.

The Markdown parser keeps structured responses intact.
Cited responses retain their original text positions and source links.
Progress text and human messages retain their existing display.
This bounded web change does not change the native client.

## Checks

- `bun run check:generated` passed from `apps/web`.
- `bun run lint` stopped at the existing missing `enabled` field in `TaskModelPoolsSettings.tsx:100`.
- `bunx eslint src/components/transcript/Message.tsx --max-warnings=0` passed.
- `bun run build` passed.
- `git diff --check` passed.

Read-only browser inspection used the live conversation at 1440×1000 and 390×1000.
The screenshot replies now show separate greeting and question bubbles at both widths.
The avatar and composer remain correctly placed. The phone layout has no horizontal overflow.
No live conversation data changed during inspection.
The helper rejected the application's attempted Chat initialization mutation.
Its insecure inspection origin needed a temporary browser-only UUID function.

Code review checked the structured Markdown, citations, human text, and progress exclusions.
Live streaming and structured-response rendering were not separately exercised.
The same Message component receives streamed and saved text.

The change adds 33 net production lines. It adds no tests, dependencies, schema, or APIs.
