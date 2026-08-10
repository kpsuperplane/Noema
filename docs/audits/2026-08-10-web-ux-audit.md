# Web UX audit

Date: 2026-08-10

## Outcome

The static audit found eight high-severity findings and seven medium-severity findings.

Approval safety is the main risk. Navigation, focus, contrast, state accuracy, and failure recovery also need work.

This report recommends changes only. It does not change the web application.

## Scope and coverage

This was a full static audit of every current web route and its owning user-facing components.

The scope included authentication, onboarding, Chat, Tasks, Memory, Settings, the shell, dialogs, and offline states.

The audit used the current product contracts and Astryx component guidance.

Generated GraphQL output, backend-only code, and tests were supporting evidence only.

No browser, screen-reader, network, or device inspection occurred. Runtime findings remain explicit verification work.

The wireframes are schematic. They show information order and interaction states, not final spacing or styling.

| Domain | Reviewed evidence | Result |
| --- | --- | --- |
| Better Accessibility | Landmarks, focus, live updates, labels, keyboard behavior, hidden content, and control semantics | Six findings |
| Better Layout | Task completion paths, information order, mobile navigation, dialogs, and responsive grouping | One primary finding and shared support |
| Better Writing | Decision copy, status labels, empty states, errors, and recovery actions | Three primary findings and shared support |
| Better Typography | Type roles, minimum sizes, metadata density, wrapping, and web-font delivery | One finding |
| Better Colors | Semantic roles and measured foreground-background contrast | One finding |
| Better UI | Submission state, destructive actions, approval behavior, feedback, motion, and surface reuse | Four findings |

## Findings

### UX-01 — HIGH — Approvals omit required decision evidence

Owner: Better Writing. Better Layout supports the information order.

Evidence:

- `apps/web/src/components/actions/HumanInterventionDecisionCards.tsx:101`
- `apps/web/src/graphql/governedActionOperations.ts:48`
- `docs/harness/action-governance.md:408`

Before:

- Generic cards show `safeSummary`, a capability name, and collapsed raw arguments.
- Queried `destructive`, `openWorld`, and `idempotent` flags are not rendered.
- The read model has no structured destination, audience, payload, or egress summary.

After:

- Add typed target, destination, payload, audience, egress, and reversibility fields to the approval read model.
- Do not derive these fields from prose or raw JSON.
- Lead with a concrete verb and target.
- Keep review routing, capability identifiers, and raw arguments under technical details.
- Keep secrets absent and preserve authorized private information.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Approve action                       |
| capability: email.send               |
| { raw arguments }      [Show details]|
| [Decline]                 [Approve]  |
+--------------------------------------+

AFTER
+--------------------------------------+
| Send project update to Sam           |
| To: Sam                              |
| Shares: message and attachment       |
| Audience: external                   |
| Reversible: no                       |
| [Technical details]                  |
| [Decline]                 [Approve]  |
+--------------------------------------+
```

Why:

People can approve an external effect without reliable evidence about its target, disclosure, or reversibility.

### UX-02 — HIGH — Failed intervention queries can hide the only valid task action

Owner: Better Layout. Noema product UI supports the state hierarchy.

Evidence:

- `apps/web/src/components/actions/PendingGovernedActions.tsx:59`
- `apps/web/src/components/actions/PendingGovernedActions.tsx:83`
- `apps/web/src/components/chatDetail/task/TaskDetailQueryPanel.tsx:64`
- `apps/web/src/components/chatDetail/task/TaskDetailPanel.tsx:67`

Before:

- The intervention region has no loading, error, retry, or stale state.
- Task detail always removes `ANSWER` and `RETRY` from its fallback controls.
- Cached task detail stays interactive after a refresh failure.

After:

- Give the intervention region explicit loading, empty, error, and stale states.
- Show a nearby Retry control when required actions cannot load.
- Remove fallback actions only after a successful intervention query.
- Keep stale evidence visible, but disable irreversible commands until reconciliation succeeds.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Task: Confirm delivery date          |
| Waiting for you                      |
|                                      |
| No response control                  |
+--------------------------------------+

AFTER
+--------------------------------------+
| Task: Confirm delivery date          |
| Action request could not load        |
| [Retry]                              |
|                                      |
| Last known question                  |
| [Response unavailable while stale]   |
+--------------------------------------+
```

Why:

A waiting task can show no response control and no explanation.

### UX-03 — HIGH — Approval controls default to approval and report the wrong pending choice

Owner: Better UI.

Evidence:

- `apps/web/src/components/tasks/TaskActions.tsx:47`
- `apps/web/src/components/tasks/TaskActions.tsx:149`
- `apps/web/src/components/tasks/TaskActions.tsx:210`
- `apps/web/src/components/tasks/TaskActionDialog.tsx:23`
- `apps/web/src/components/actions/HumanInterventionDecisionCards.tsx:108`

Before:

- Task approvals initialize as `APPROVED`.
- Plain Enter submits the selected approval after a user types a response.
- A declined governed action shows loading on the Approve button.

After:

- Start approval gates without a selected decision.
- Require an explicit Approve or Decline choice.
- Let plain Enter add text during approval gates.
- Submit approval only through the explicit decision control.
- Track the pending decision and mark only its control as loading.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Decision                             |
| (x) Approve  ( ) Decline             |
| [Add a note___________________]      |
| Enter submits                        |
| After Decline: [Decline] [Approving] |
+--------------------------------------+

AFTER
+--------------------------------------+
| Decision                             |
| Initial: ( ) Decline  ( ) Approve    |
| [Add a note___________________]      |
| Enter adds a new line                |
| After Decline: [Declining] [Approve] |
+--------------------------------------+
```

Why:

The current interaction can approve by default and can misstate which consequential command is running.

### UX-04 — HIGH — Chat submission permits duplicate messages

Owner: Better UI.

Evidence:

- `apps/web/src/app/App.tsx:77`
- `apps/web/src/app/App.tsx:793`
- `apps/web/src/components/composerModel.ts:5`
- `apps/web/src/components/Composer.tsx:407`

Before:

- `pending` changes the button label only.
- The command guard and submit control ignore the pending state.
- Enter, pointer, and touch paths can start another mutation.

After:

- Add a synchronous in-flight guard at the command boundary.
- Include `pending` in the composer disabled state.
- Clear the guard only after success or recoverable failure.
- Keep one optimistic row and one client message identifier per submission.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| You: Send the report                 |
| You: Send the report                 |
|                                      |
| [Send the report____________] [Send] |
+--------------------------------------+

AFTER
+--------------------------------------+
| You: Send the report        Sending  |
|                                      |
| [Composer unavailable while sending] |
|                           [Sending]  |
+--------------------------------------+
```

Why:

Duplicate messages can start duplicate model work and duplicate downstream actions.

### UX-05 — HIGH — Destructive settings actions bypass confirmation

Owner: Better UI. Better Writing supports consequence copy.

Evidence:

- `apps/web/src/components/settings/LocalModelsSettingsPane.tsx:296`
- `apps/web/src/components/settings/ProvidersSettingsPane.tsx:239`
- `apps/web/src/components/settings/NotificationsSettingsPane.tsx:156`

Before:

- Remove deletes a local model installation immediately.
- Clear key deletes provider credentials immediately.
- Other destructive settings already use a shared confirmation dialog.

After:

- Reuse the existing confirmation dialog for both actions.
- Name the affected model or provider account.
- State the lost data, access effect, and recovery path.
- Prevent repeated submission and restore focus after dismissal.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Local model: Granite 8B              |
| 4.2 GB installed            [Remove] |
+--------------------------------------+
                |
                +-- removed immediately

AFTER
+--------------------------------------+
| Remove Granite 8B?                   |
| This deletes 4.2 GB of model data.   |
| Download the model again to restore. |
| [Cancel]              [Remove model] |
+--------------------------------------+
```

Why:

One accidental activation can remove downloaded data or stop provider access.

### UX-06 — HIGH — The shell lacks one accessible navigation contract

Owner: Better Accessibility. Better Layout supports the shell structure.

Evidence:

- `apps/web/src/components/shell/AppShell.tsx:236`
- `apps/web/src/components/shell/AppShell.tsx:586`
- `apps/web/src/components/shell/AppShell.tsx:674`
- `apps/web/src/components/shell/AppShell.tsx:711`
- `apps/web/src/components/shell/ShellSidebar.tsx:156`
- `apps/web/src/pages/MemoryPageTree.tsx:36`
- `apps/web/src/components/tasks/TasksSurface.tsx:77`
- `apps/web/src/components/tasks/TasksViews.tsx:304`
- `apps/web/index.html:10`

Before:

- Primary and sidebar routes use buttons with imperative navigation.
- Hidden mobile route content gets `pointer-events: none`, but remains focusable and exposed.
- The shell `<main>` contains repeated chrome, while Tasks nests another `<main>`.
- Task links receive `role="listitem"`, which replaces their native link role.
- The document title remains `Noema`, and the shell has no skip link.

After:

- Render routes with TanStack links styled through Astryx patterns.
- Move focus into mobile navigation before hiding route content.
- Apply `inert` and `aria-hidden` only after focus moves.
- Restore focus to the trigger when navigation closes.
- Keep repeated chrome outside one route-level `<main>`.
- Use semantic lists with links inside list items.
- Add a first-focusable skip link.
- Update the document title and route heading focus after navigation.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| [button Chat] [button Tasks]         |
| <main> shell header                  |
|   <main> Tasks                       |
|     [Task link with listitem role]   |
| Mobile menu open: route still tabs   |
+--------------------------------------+

AFTER
+--------------------------------------+
| [Skip to content]                    |
| <nav> [Chat link] [Tasks link]       |
| <main id="content">                  |
|   Tasks <- focus after navigation    |
|   <ul><li>[Task link]</li></ul>      |
| Mobile menu open: route is inert     |
+--------------------------------------+
```

Why:

Keyboard and screen-reader users can reach hidden content and lose native navigation behavior.

### UX-07 — HIGH — Focal text controls remove visible keyboard focus

Owner: Better Accessibility.

Evidence:

- `apps/web/src/components/Composer.tsx:496`
- `apps/web/src/components/tasks/TaskActions.tsx:402`
- `apps/web/src/components/transcript/ExpandableTextBubble.tsx:40`

Before:

- Chat and task response fields remove their Astryx focus treatment.
- Their surrounding composer surfaces have no `:focus-within` replacement.
- The full-bubble expand button sets `outline: none` without a replacement.

After:

- Add a two-pixel token-backed `:focus-within` ring to both composer surfaces.
- Add a visible focus state to the expansion trigger.
- Preserve each indicator in forced-colors mode.
- Cap composer height and enable internal scrolling for long drafts.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Composer                             |
|   caret in field, no visible focus   |
| [Message bubble: select to expand]   |
+--------------------------------------+

AFTER
+======================================+
| Composer has a visible focus ring    |
| [Draft text____________________]     |
+======================================+
| Message preview   [Show full message]|
|                    ^ visible focus   |
+--------------------------------------+
```

Why:

Keyboard users cannot reliably locate the main input or message expansion control.

### UX-08 — HIGH — Muted text tokens fail normal-text contrast

Owner: Better Colors.

Evidence:

- `apps/web/src/theme/noema-neutral.css:32`
- `apps/web/src/theme/noema-neutral.css:104`
- `apps/web/src/components/tasks/TasksViews.tsx:385`
- `apps/web/src/pages/memoryPageStyles.ts:12`

Before:

- `--noema-text-muted` resolves to `#827d6b`.
- It measures 4.12:1 on white and 3.69:1 on the paper surface.
- `--noema-text-faint` measures 2.53:1 on white.
- Both tokens render ordinary metadata and supporting text.

After:

- Correct the semantic tokens at the theme authority.
- Keep ordinary text at 4.5:1 or higher on every supported surface.
- Reserve faint colors for disabled or decorative content.
- Recheck accent text on paper surfaces before release.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Task title                           |
| Muted metadata on paper    3.69:1 X  |
| Faint supporting text      2.53:1 X  |
+--------------------------------------+

AFTER
+--------------------------------------+
| Task title                           |
| Supporting metadata         4.5:1+   |
| Secondary help text         4.5:1+   |
| Faint color: disabled marks only     |
+--------------------------------------+
```

Why:

Normal text currently fails WCAG AA across common light surfaces.

### UX-09 — MEDIUM — Operational metadata uses a 9–11 pixel type scale

Owner: Better Typography.

Evidence:

- `apps/web/src/components/tasks/TasksViews.tsx:385`
- `apps/web/src/components/tasks/TaskScheduleSummary.tsx:154`
- `apps/web/src/components/actions/PendingGovernedActions.tsx:720`
- `apps/web/src/components/actions/HumanInterventionDecisionCards.tsx:467`
- `apps/web/src/components/transcript/RuntimeDebugDialog.tsx:527`

Before:

- The source contains 69 non-test style rules at 9, 10, or 11 pixels.
- These rules render counts, times, schedules, action evidence, and diagnostics.

After:

- Define one supporting-text role at 12 pixels or larger.
- Replace microtype in task, action, transcript, and settings surfaces.
- Use tabular numerals for changing counts and times.
- Keep smaller type only for nonessential marks.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Review quarterly plan                |
| 2 RUNS | DUE 09:30 | WAITING   9px   |
| Next run: 08/12 14:00         10px   |
+--------------------------------------+

AFTER
+--------------------------------------+
| Review quarterly plan                |
| 2 runs | Due 09:30 | Waiting   12px+ |
| Next run: 08/12 14:00          12px+ |
| Times use tabular numerals           |
+--------------------------------------+
```

Why:

Important operational data becomes hard to scan, especially at increased text size.

### UX-10 — MEDIUM — New assistant messages lack reliable announcements

Owner: Better Accessibility.

Evidence:

- `apps/web/src/components/transcript/TranscriptScroller.tsx:594`
- `apps/web/src/components/transcript/Message.tsx:165`

Before:

- The transcript is a focusable region without chat-log or live semantics.
- Streaming completion does not create a dedicated announcement.

After:

- Add tested log semantics around the virtualized transcript.
- Announce one completed assistant response through a separate live region.
- Do not announce every streamed token.
- Expose busy state while a response is incomplete.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Transcript region                    |
| Noema: Streaming response...         |
| Response completes silently          |
+--------------------------------------+

AFTER
+--------------------------------------+
| Transcript log              [busy]   |
| Noema: Streaming response...         |
|                                      |
| Live status: "Noema replied"         |
| Announced once after completion      |
+--------------------------------------+
```

Why:

Screen-reader users can miss the product's primary response.

### UX-11 — MEDIUM — Custom selection widgets expose incomplete state and keyboard behavior

Owner: Better Accessibility.

Evidence:

- `apps/web/src/components/transcript/MultipleChoicePrompt.tsx:209`
- `apps/web/src/components/tasks/ScheduleFields.tsx:109`

Before:

- Pick-one prompts use radio roles, but every option remains a Tab stop.
- The group has no arrow-key or roving-focus behavior.
- Weekday buttons show selection through visual variants only.
- The Days label does not name a group.

After:

- Reuse Astryx radio components for pick-one prompts.
- Preserve the existing semantic submission command.
- Use a named weekday group with checkbox controls.
- If buttons remain, add accurate `aria-pressed` state.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Choose one                           |
| [ ] Basic       Tab stop             |
| [x] Advanced    Tab stop             |
|                                      |
| Days  [M] [T] [W] [T] [F]            |
| Selection is visual only             |
+--------------------------------------+

AFTER
+--------------------------------------+
| Choose one                           |
| ( ) Basic       Tab, then arrows     |
| (x) Advanced                         |
|                                      |
| Days                                 |
| [x] Mon [ ] Tue [x] Wed [ ] Thu      |
+--------------------------------------+
```

Why:

Keyboard and screen-reader users do not receive the expected selection model.

### UX-12 — MEDIUM — Settings status presentation can be false or opaque

Owner: Better Writing. Better UI and Better Colors support state presentation.

Evidence:

- `apps/web/src/components/settings/WebSettingsPane.tsx:111`
- `apps/web/src/components/settings/WebSettingsPane.tsx:341`
- `apps/web/src/components/settings/CapabilityIntegrationList.tsx:99`
- `apps/web/src/components/settings/CapabilityConnectionDetail.tsx:220`
- `apps/web/src/components/settings/AgentsSettingsPane.tsx:205`

Before:

- Search, Fetch, and Browse always show a success dot and `Enabled`.
- This label remains during loading, errors, and missing provider configuration.
- Integration and agent surfaces print internal status values unchanged.

After:

- Derive each visible status from its current read model.
- Use one shared status-label authority.
- Provide `Loading`, `Needs setup`, `Sign-in required`, `Unavailable`, and `Ready` states.
- Keep raw values inside technical details.
- Pair each blocked state with its recovery action.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Search       green dot  Enabled      |
| Fetch        green dot  Enabled      |
| Calendar     AUTH_REQUIRED           |
+--------------------------------------+

AFTER
+--------------------------------------+
| Search                   Loading...  |
| Fetch                    Needs setup |
|                    [Choose provider] |
| Calendar            Sign-in required |
|                             [Sign in]|
+--------------------------------------+
```

Why:

People can receive false success signals or internal values without a useful next step.

### UX-13 — MEDIUM — A loaded empty conversation looks permanently busy

Owner: Better Writing.

Evidence:

- `apps/web/src/app/App.tsx:884`
- `apps/web/src/components/ChatSurface.tsx:185`

Before:

- Chat distinguishes initial loading from a loaded empty transcript.
- The surface still renders a loading skeleton whenever the transcript is empty.

After:

- Render the skeleton only while initial data is loading.
- Show a compact loaded-empty orientation state afterward.
- Keep the composer as the focal action.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Conversation                         |
| [////////////////////////////]       |
| [////////////////////]               |
| Loading skeleton never ends          |
| [Message Noema_______________]       |
+--------------------------------------+

AFTER
+--------------------------------------+
| New conversation                     |
| Ask Noema a question to begin.       |
|                                      |
| [Message Noema_______________] [Send]|
+--------------------------------------+
```

Why:

Fresh conversations appear stuck, even when Chat is ready.

### UX-14 — MEDIUM — Async failure states lack a consistent recovery path

Owner: Better UI. Better Writing supports recovery copy.

Evidence:

- `apps/web/src/components/shell/AppBootBoundary.tsx:88`
- `apps/web/src/components/chatDetail/ArtifactDetailPanel.tsx:25`
- `apps/web/src/components/settings/CapabilityConnectionDetail.tsx:194`
- `apps/web/src/components/settings/DeleteConnectionDialog.tsx:118`

Before:

- Boot failure shows an error without Retry.
- Artifact refresh failure replaces cached content without Retry.
- Tool reset and toggle failures have no local catch or feedback.
- Shared deletion errors are not announced as alerts.

After:

- Add Retry or Reload beside every recoverable failure.
- Preserve cached content with a clear stale warning.
- Catch reset and toggle failures while preserving prior state.
- Announce mutation failures once and retain focus.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Artifact unavailable                 |
| Refresh failed                       |
|                                      |
| No content and no recovery action    |
+--------------------------------------+

AFTER
+--------------------------------------+
| Artifact                    Stale    |
| Refresh failed. Showing saved copy.  |
| [Retry]                              |
|--------------------------------------|
| Last available artifact content      |
+--------------------------------------+
```

Why:

Several failed operations leave people without a clear, local recovery action.

### UX-15 — MEDIUM — Repeated row controls have ambiguous accessible names

Owner: Better Accessibility. Better Writing supports the names.

Evidence:

- `apps/web/src/components/settings/CapabilityToolTable.tsx:97`
- `apps/web/src/components/settings/TaskModelPoolsSettings.tsx:116`
- `apps/web/src/components/settings/WebSettingsPane.tsx:273`
- `apps/web/src/components/settings/AgentsSettingsPane.tsx:137`

Before:

- Tool rows repeat `Edit`, `Reset`, and `Turn on` names.
- Model switches all use `Enabled`.
- Every web selector uses `Provider for web tooling`.
- Agent rows repeat `Test` and `Edit`.

After:

- Include the affected object in every accessible name.
- Keep visible labels concise when row context already provides meaning.
- Use names such as `Reset Create event` and `Enable Simple task model`.

Low-fidelity wireframe:

```text
BEFORE
+--------------------------------------+
| Create event     [Edit] [Reset]      |
| Find event       [Edit] [Reset]      |
| Screen reader: Edit, Reset, Edit...  |
+--------------------------------------+

AFTER
+--------------------------------------+
| Create event     [Edit] [Reset]      |
| names: Edit Create event             |
|        Reset Create event            |
| Find event       [Edit] [Reset]      |
| names: Edit Find event, Reset...     |
+--------------------------------------+
```

Why:

Screen-reader control lists and voice control cannot identify each target reliably.

## Recommended delivery order

1. Fix UX-01 through UX-03 as one approval-safety milestone.
2. Fix UX-04 and UX-05 as one command-safety milestone.
3. Fix UX-06 through UX-11 as one accessibility-foundation milestone.
4. Fix UX-12 through UX-15 as one state-clarity milestone.

Keep each milestone independently shippable. Stop if a fix requires a new unsupported product contract.

## Verification plan

| Findings | Required verification |
| --- | --- |
| UX-01 | Exercise read, send, update, delete, share, and browser approvals. Check data disclosure and secret absence. |
| UX-02 | Test slow loading, empty cache failure, stale cache failure, reconnect, and subscription loss. |
| UX-03 | Test approval and decline with keyboard, pointer, failure, stale revisions, and repeated activation. |
| UX-04 | Throttle submission. Activate Enter, pointer, and touch repeatedly. Confirm one request and one optimistic row. |
| UX-05 | Test Cancel, Escape, focus return, repeated confirmation, and mutation failure. |
| UX-06 | Inspect landmarks and the accessibility tree. Test Tab, Escape, browser link actions, route titles, and 320-pixel navigation. |
| UX-07 | Test keyboard focus, forced colors, 200% zoom, long drafts, mobile keyboards, and safe areas. |
| UX-08 | Measure every computed text and surface pair. Include default, hover, selected, sunken, warning, and disabled states. |
| UX-09 | Test 200% text zoom, long labels, changing counts, and dense task details. |
| UX-10 | Test streaming, completion, tool updates, failures, and user-scrolled-away states with NVDA and VoiceOver. |
| UX-11 | Test Tab, arrows, Space, Enter, selected-state announcements, and submitted states. |
| UX-12 | Render every known status, one unknown status, stale data, no provider, and query failure. |
| UX-13 | Test fresh accounts, empty saved conversations, offline empty state, and narrow widths. |
| UX-14 | Force boot, artifact, tool mutation, and deletion failures. Confirm retained context and one recovery action. |
| UX-15 | Inspect screen-reader control lists. Test voice control against repeated rows. |

Static validation should include TypeScript, ESLint, and automated contrast checks.

Runtime validation should include Chromium, Firefox, Safari, iOS Safari, NVDA, and VoiceOver.

## Considered but rejected

### Convert the full palette to OKLCH

The current semantic token system is consistent. Fix the failing roles without introducing a second color-authoring system.

### Remove all product motion

Shared motion paths already honor reduced motion. Runtime review should judge quality after the blockers are fixed.

### Treat every settings boundary as excess card usage

`SettingsSection` groups one semantic settings unit. It does not wrap every row as an independent card.

### Raise mobile input text from source declarations alone

`apps/web/src/styles.css:181` already enforces 16-pixel inputs on coarse pointers. Device verification remains necessary.

## Verdict

Block
