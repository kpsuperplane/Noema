# Settings Visual Consistency Implementation Plan

**Mode:** Plan only. This document does not authorize implementation by itself.

**Goal:** Give every Noema settings page one predictable visual and interaction
grammar while preserving the existing centered page width and the API/MCP
master-detail layout.

**Observable outcome:** A person moving between Agents, Memory, Web, APIs, MCPs,
Privacy, Execution, Local Models, and Providers sees the same page header,
section hierarchy, setting-row anatomy, state treatment, spacing rhythm, and
responsive order. Collections remain optimized for managing objects, and APIs
and MCPs retain their two-column desktop workflow.

**Architecture:** Keep `SettingsSurface`, `ShellPageLayout`, and
`MasterDetailLayout` as the layout authorities. Compose page regions from Astryx
`Section`, `List`, and `ListItem`, use existing Astryx inputs and selectors, and
reuse `SettingsEditDialog` for bounded multi-field changes. Start without a new
Noema settings framework; extract a shared helper only after identical production
usage remains in at least two pages.

---

## Constraints And Non-Goals

- Work directly on `main` and preserve unrelated worktree changes.
- Keep the centered content width at `860px` and keep APIs/MCPs fluid.
- Keep the API/MCP two-column desktop layout and current narrow-screen detail replacement.
- Do not add backend behavior, GraphQL schema fields, database migrations, or new settings.
- Do not redesign the Settings sidebar categories or add Settings search in this slice.
- Keep existing routes. The visible `Usage` label may become `Execution`, but its route remains `/settings/safety/usage`.
- Do not expose source manifests, raw metadata, provider internals, or credentials by default.
- Do not add frontend tests. Validate generated types, lint, build, and static states.
- Use browser inspection only when the user explicitly authorizes it; otherwise report static/build validation only.
- The refactor must be net-negative in production code. Stop if it requires a parallel abstraction or becomes net-positive.

## Product Model

**Human:** The owner configuring a self-hosted personal agent system.

**Job:** Find a setting, understand its consequence, and change or inspect it safely.

**Focal point:** The current section's setting rows or managed objects.

**Now:** Setting name, consequence, current state, and available control.

**Next:** Health, scope, provider, model, or policy context required to act safely.

**Later:** Technical metadata, provenance, source definitions, and destructive actions.

**Omit:** Duplicated metadata, decorative status, repeated headings, and implementation detail that does not change the next action.

**Grouping:** Sections represent one user question; rows represent one setting or managed object; detail surfaces represent one selected connection.

**Density:** Compact management density, with 8px within rows, 12–16px within sections, and 24px only between major page regions.

## Research Basis

- Linear separates account, workspace, and administration scope, then groups controls under semantic page sections: <https://linear.app/docs/account-preferences> and <https://linear.app/docs/workspaces>.
- Stripe groups settings into Personal, Account, and Product before individual product controls: <https://docs.stripe.com/dashboard/basics>.
- Vercel distinguishes team-wide from project-specific settings: <https://examples.vercel.com/academy/optimize-your-vercel-account/tour-the-dashboard>.
- Raycast uses direct-to-object settings, categorized extension lists, and search only after the catalog becomes large: <https://manual.raycast.com/settings>.
- Notion separates ordinary configuration, security controls, and a final danger zone: <https://www.notion.com/help/workspace-settings>.

Noema's existing sidebar already follows the successful scope-first pattern.
The inconsistency is inside pages, so this slice standardizes content composition
without adding another navigation layer.

---

## Settings Visual Contract

### Page

- Use one sticky page header containing the title and one-line description on every settings route.
- Use the existing centered track for ordinary pages and the fluid track for APIs/MCPs.
- Give ordinary page content one vertical rhythm; do not let each pane define its own top inset.
- Put a page-level focal action in the header when it creates the primary managed object.

### Section

- Use Astryx `Section` for a page region; do not use a Card merely to box a settings group.
- Give each section one concise heading and optional consequence-oriented description.
- Separate same-surface sections with spacing or a divider, not independent decorative containers.
- Use a muted section only for a meaningful current state such as the active local runtime.

### Row

- Use Astryx `List` and `ListItem` for related settings or managed objects.
- Place the label and a one-sentence description on the start side and one control, status, or action on the end side.
- Show secondary metadata beneath the label in plain text; use badges only for counts or enumerated states and status tokens for live status.
- Save selectors and switches immediately. Open `SettingsEditDialog` for multi-field edits with explicit Save and initial focus.
- Place loading, warning, validation, and save errors with the row or section they affect.
- On narrow screens, stack end controls beneath their row copy while preserving source and focus order.

### Disclosure And Danger

- Keep technical contracts, source definitions, raw metadata, and provenance behind a clear disclosure.
- Put removal and credential-clearing actions in a final danger region or destructive confirmation dialog.
- Keep API/MCP connection detail ordered as identity, policy, tools, source details, then connection removal.

---

## Page Destination Map

| Page | Final groups and behavior |
| --- | --- |
| Agents | `Agent models` rows for ordinary agents; `Task models` rows for Simple, Medium, and High complexity. Remove the large task-executor card and nested global limit section. |
| Memory | One `Background updates` section containing the model row, local-human scope, and owned warning/error feedback. |
| Web | `Search` and `Fetch` sections. Provider and summarizer controls become rows; tool IDs, extraction, safety, and reliability move to technical disclosure. |
| Privacy | One `Risky action reviews` section containing the reviewer-model row and fail-closed consequence. |
| Execution | Move global task execution limits here from Agents, then show `Progress auditing` as a model row. Keep the existing route. |
| Local Models | `Runtime`, `Installed models`, and `Available models` sections using compact rows. Open advanced GGUF import in a dialog. |
| Providers | `Provider accounts` list with Add provider as the focal action. Accounts show provider, authentication, and status; credential and deletion work uses disclosures/dialogs. |
| APIs/MCPs | Preserve master-detail. Apply the same section, row, status, disclosure, spacing, and danger treatment inside both panes. |

Dialog contracts:

- To add a provider account, the human needs a provider, optional account label,
  and the supported authentication input; dismissing leaves provider accounts unchanged.
- To import a GGUF, the human needs a source, model name, and source-specific
  provenance fields; dismissing leaves the model store unchanged.

---

## Milestone 1: Shared Page And Row Grammar

**Expected files:** `SettingsPage.tsx` and the smallest necessary shared settings
presentation file, only if Astryx primitives alone cannot express the repeated anatomy.

- [ ] Unify title and description placement for centered and fluid settings pages.
- [ ] Establish one ordinary-page content inset and major-region gap.
- [ ] Prove the contract on Memory and Web, covering one simple page and one multi-section page.
- [ ] Replace custom cards, headings, definition rows, and duplicated provider metadata with Astryx sections and lists.
- [ ] Preserve loading, error, save, warning, and narrow-screen behavior.
- [ ] Run the patch-size report and commit the independently coherent grammar slice.

**Gate:** Stop if the proving slice adds more presentation code than it deletes or
requires changes outside existing frontend settings behavior.

## Milestone 2: Preference Pages And Semantic Ownership

**Expected files:** `PrivacySettingsPaneContent.tsx`, `UsageSettingsPane.tsx`,
`UsageSettingsPaneContent.tsx`, `AgentsSettingsPane.tsx`,
`AgentsSettingsPaneContent.tsx`, `TaskExecutionPolicySettings.tsx`,
`shellNavigation.ts`, and `SettingsPage.tsx`.

- [ ] Convert Privacy to the shared section-and-row grammar.
- [ ] Move the existing task-execution policy query, mutation, and presentation from Agents to Usage.
- [ ] Change the visible Usage label/title to Execution while retaining the route and section identifier.
- [ ] Compose Execution from `Run limits` and `Progress auditing` sections.
- [ ] Remove task-execution policy props and rendering from Agents without changing backend authority.
- [ ] Keep each query error scoped to its owning Execution section.
- [ ] Run the patch-size report and commit the semantic ownership slice.

**Gate:** The move must reuse the existing GraphQL operations and component rather
than introduce a combined settings DTO, query, or compatibility layer.

## Milestone 3: Managed Object Collections

**Expected files:** Agents, task-model-pool, Providers, and Local Models settings
components plus existing dialogs directly reused or narrowly adapted.

- [ ] Convert agents and task complexity tiers to divided object/setting rows.
- [ ] Convert provider accounts to rows and move Add provider into a focused dialog.
- [ ] Keep provider replacement, clearing, OAuth, and deletion semantics unchanged.
- [ ] Convert local runtime, installed models, and curated models to sections and rows.
- [ ] Move advanced GGUF import into a focused dialog using Astryx inputs.
- [ ] Keep progress, cancellation, activation, removal, empty, failed, and unavailable states visible and owned.
- [ ] Delete superseded card, raw input, heading, metadata, and spacing styles.
- [ ] Run the patch-size report and commit the collection slice.

**Gate:** Do not add provider or model detail routes in this slice. Use rows,
disclosure, and existing dialogs unless the information cannot remain task-bounded.

## Milestone 4: API And MCP Inner Consistency

**Expected files:** `CapabilityManagementLayout.tsx`,
`CapabilityIntegrationList.tsx`, `CapabilityConnectionDetail.tsx`,
`CapabilityToolTable.tsx`, `AdapterSettingsPane.tsx`, and MCP settings content.

- [ ] Preserve `MasterDetailLayout`, desktop column sizing, independent scrolling, and mobile detail replacement.
- [ ] Convert integration groups and connection entries to the shared list language.
- [ ] Normalize list-pane actions, connection status, policy summary, and detail section hierarchy.
- [ ] Keep the tool table as a table because its columns support comparison.
- [ ] Keep source definition and metadata behind disclosure and connection removal last.
- [ ] Preserve API definition review and MCP setup/reauthentication behavior.
- [ ] Run the patch-size report and commit the integration consistency slice.

**Gate:** No route, connection-policy, authentication, tool-management, or
definition-review behavior may change to achieve visual consistency.

## Milestone 5: Static Review And Validation

- [ ] Confirm every page has one focal action/content area identifiable within three seconds.
- [ ] Confirm every section has tighter internal spacing than the gap to adjacent sections.
- [ ] Confirm controls remain attached to the setting or object they change.
- [ ] Confirm empty, loading, error, saving, stale, long-label, and narrow-screen states preserve hierarchy.
- [ ] Confirm badges, cards, borders, headings, icons, and exposed metadata are no more numerous than the workflow requires.
- [ ] Run `bun run scripts/report-rust-size.ts --base <unit-base> --require-net-negative` at each milestone and before the final commit.
- [ ] Run `bun run gen:types`, `bun run lint`, and `bun run build` from `apps/web`.
- [ ] Run `git diff --check`, inspect status, and inspect staged stat/name-status before each commit.
- [ ] If browser inspection is authorized, inspect representative desktop and mobile widths for every page type; otherwise explicitly report that visual verification was not performed.

---

## Budget And Stop Conditions

- Production code: net negative across the completed refactor; target 250–600 fewer handwritten lines.
- Test code: zero new lines and zero new tests.
- New production abstractions: zero by default; at most one small shared presentation helper after demonstrated repeated use.
- Expected implementation: four milestone commits plus a final validation/correction commit only if needed.
- Stop when the patch becomes net-positive, requires backend/schema/route expansion,
  changes behavior outside settings presentation, or exceeds the expected production
  reduction by 500 lines without a simpler consolidation path.

## Acceptance Scenarios

1. Moving between ordinary settings pages preserves header placement, content width, section rhythm, row anatomy, and control alignment.
2. A model or provider selection saves immediately and reports failure next to that setting.
3. A multi-field provider or GGUF operation opens a focused dialog and does not leave an inline Save form on the page.
4. Execution owns global run limits and progress auditing; Agents owns agent and task-model selection.
5. API and MCP list/detail management remains two-column on desktop and single-detail on narrow screens with no behavior loss.
6. Technical and destructive controls remain available without dominating the default view.
7. The final implementation is net-negative and passes generated-type, lint, build, and whitespace validation.
