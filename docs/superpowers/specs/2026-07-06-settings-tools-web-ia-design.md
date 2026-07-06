# Settings Tools Web IA Design

## Goal

Improve Settings information architecture so first-party web tooling lives
under Tools instead of Agents.

The first implementation slice should:

- Make Agents a standalone top-level Settings item.
- Group live non-agent settings under Tools, Safety, and System labels.
- Add a canonical `Settings > Tools > Web` page.
- Move the Web fetch summarizer model setting from Agents to Web.
- Keep the implementation limited to live pages; do not show future Images,
  Voice, or other placeholder categories.

## Problem

The current Web fetch summarizer setting is shown under Agents because the
implementation reused the agent model-preference editor. That placement is
conceptually wrong: the summarizer is shared first-party web fetch
infrastructure, not per-agent identity or runtime behavior.

Settings is also currently flat:

- Providers
- Agents
- MCPs
- Trusted identities
- Approvals

That flat list obscures the product model. MCPs and Web are tool capability
settings. Approvals and identities are safety/governance settings. Providers
are system/provider-account settings.

## Chosen Approach

Use grouped Settings navigation with direct section pages.

Navigation shape:

- Agents
- Tools
  - Web
  - MCPs
- Safety
  - Approvals
  - Identities
- System
  - Providers

Agents has no group label above it. Tools, Safety, and System are labels, not
clickable landing pages.

This keeps the slice focused: no Tools overview page, no group landing pages,
and no future placeholder nav items.

## Canonical Routes

Settings should use only canonical nested paths:

| Page | Canonical path |
| --- | --- |
| Agents | `/settings/agents` |
| Web | `/settings/tools/web` |
| MCPs | `/settings/tools/mcps` |
| Approvals | `/settings/safety/approvals` |
| Identities | `/settings/safety/identities` |
| Providers | `/settings/system/providers` |

`/settings` should resolve to Agents.

Old flat paths such as `/settings/providers`, `/settings/mcps`,
`/settings/trusted-identities`, and `/settings/approvals` should not be
supported. This project is pre-V1, and route compatibility should not add
parallel IA. These paths should fall through to the app's normal unknown-path
behavior instead of redirecting.

`pathForRoute` should emit only canonical paths.

## Web Settings Page

`Settings > Tools > Web` owns first-party web capability configuration.

The page should have two primary sections.

### Search

Search represents `web.search`.

The first slice should show:

- Status: enabled.
- Provider: DuckDuckGo public adapter.
- Contract: model proposes a query, Noema returns normalized search results,
  and this tool does not fetch or read result pages.

Search is read-only for now. No provider picker, result tuning, or additional
configuration is included in this slice.

### Fetch

Fetch represents `web.fetch`.

The first slice should show:

- Status: enabled.
- Provider: direct HTTP.
- Extraction: `readabilityrs` markdown output.
- Safety posture: public HTTP(S), redirect validation, private/local targets
  blocked, response size caps.

Fetch owns the Fetch summarizer model preference as a subsection or nested
card. The summarizer is not a peer web capability; it is part of fetch behavior.

The summarizer editor should keep the existing backend behavior:

- Query through `webFetchSettings`.
- Save through `saveWebFetchSummarizerPreference`.
- Default to the configured web-fetch summarizer default when no explicit
  preference is saved.

## Agents Page

`Settings > Agents` should return to agent-specific concerns only:

- Agent cards.
- Per-agent model/runtime preferences.
- Read-only agent metadata currently available in the backend.

It should not query or render Web fetch settings.

## Frontend Structure

The route model can remain a single Settings section union. Use section values
that represent the grouped IA explicitly:

- `agents`
- `tools-web`
- `tools-mcps`
- `safety-approvals`
- `safety-identities`
- `system-providers`

The shell Settings submenu should be generated from a grouped nav model rather
than hand-written flat items. The grouped nav model should carry:

- Optional group label.
- Display label.
- Settings section id.
- Route.

Agents should be represented as a top-level item with no group label.

Component ownership:

- `AgentsSettingsPane` owns `AgentsDocument` and agent preference mutations.
- New `WebSettingsPane` owns `WebFetchSettingsDocument` and
  `SaveWebFetchSummarizerPreferenceDocument`.
- Existing MCPs, Approvals, Identities, and Providers panes keep their data
  ownership and move only by route/nav placement.
- The model preference editor should be extracted from
  `AgentsSettingsPaneContent` into a neutral settings component so Web can
  reuse it without importing Agents-specific UI.

## Error Handling

- Agent query errors should affect only the Agents page.
- Web fetch settings query errors should affect only the Fetch summarizer
  editor. Static Search and Fetch status sections should still render.
- Fetch summarizer save errors should stay local to the Web page.
- If Web fetch settings are unavailable, the Fetch section should still show
  provider/extraction/safety information and mark summarizer controls as
  unavailable.

## Non-Goals

- No future Images, Voice, browser, or hosted-web placeholder nav items.
- No Tools overview page.
- No Safety overview page.
- No System overview page.
- No old flat route aliases.
- No backend changes to the summarizer preference contract.
- No new web search provider configuration.
- No web fetch provider picker.

## Validation

Implementation should include:

- Route tests for every canonical Settings path.
- A route test confirming old flat Settings paths no longer resolve as
  Settings routes.
- Settings pane selection tests if they can be added through the existing
  frontend test harness without introducing a new UI test framework; otherwise
  route tests are sufficient for this slice.
- Frontend type/lint validation through the existing web command.

No browser inspection is required unless explicitly requested during
implementation.
