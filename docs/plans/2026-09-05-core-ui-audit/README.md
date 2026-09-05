# Core app UI audit

Status: recommendations and mocks. Product UI changes are not approved or implemented.

The existing navigation, list/detail layout, typography, and surface treatment work well.
The main opportunity is clearer task states and more consistent controls within those surfaces.

## Open the mocks

[Open the running gallery](http://localhost:8766/#inbox).
Use the numbered links to inspect ten proposals. Select **Phone width** to narrow the surface.
At phone screen widths, the gallery uses the available width automatically.

The gallery includes Inbox details, recurring details, scheduling, provider settings, notifications,
task settings, start/cancel dialogs, task capture, and agent model settings.
Buttons and fields use Astryx. List choices and settings groups reuse the current Noema components.

If the preview server has stopped, run this command from the repository root:

```sh
python3 -m http.server 8766 --bind 127.0.0.1 --directory docs/plans/2026-09-05-core-ui-audit/gallery
```

Open `http://localhost:8766/` in a browser. Opening the HTML file in an editor shows its source.
The built gallery has no API connection. Changes stay in the current mock and reset on navigation.

Rebuild the saved gallery after editing its source:

```sh
node docs/plans/2026-09-05-core-ui-audit/build.mjs
```

## Inspection evidence

I inspected the rendered development app through its private Unix socket on September 5, 2026.
I inspected Tasks, task capture, Agents, Providers, Notifications, Execution, Memory settings, and API accounts.
The desktop inspection used a 1440 × 1050 viewport. Mobile inspection used 390 × 844.
I also read the relevant components and surface contracts.

The development workspace had no tasks. Populated Inbox and recurrence findings therefore come from source inspection.
Their mocks use sample data. They are not screenshots of existing tasks.
Apple push settings and API accounts returned access errors during socket inspection.
Those errors are not classified as product UI defects here.
Mobile inspection covered Agents, execution limits, and task scheduling controls.
The proposed mocks were also reviewed at desktop and phone widths.
Populated live task details remain unverified because the workspace had no tasks.

Live screenshots remain outside the repository in the local audit working directory.
Saved screenshots contain sample mocks only. No task or settings data was changed during inspection.

The user separately approved web access through the development socket.
That small server change enables this audit. It does not change product UI or public authentication.

## Priority 1: remove unnecessary uncertainty

### 1. Keep simple recurring schedules simple

**Observed in source:** Every recurrence edit starts as `repeat: "custom"`.
A weekday schedule therefore becomes a cron form when edited.
The timezone hint says “IANA timezone used for wall-clock recurrence.”

**Suggestion:** Recognize supported presets when opening the editor.
Keep unmatched cron schedules intact under Custom schedule.
Group Repeat, Time, Time zone, and Starts on in that order.
Use a searchable timezone control with friendly names and the exact zone available for inspection.
Show a compact first-run preview. Keep the full upcoming-run preview available through disclosure.
Keep missed-run and overlap controls together below the main schedule.

**Mock:** [Schedule dialog](http://localhost:8766/#schedule).
**Source:** [ScheduleFields](../../../apps/web/src/components/tasks/ScheduleFields.tsx), [recurrence editor](../../../apps/web/src/components/tasks/TaskRecurrenceDetailPanel.tsx).
The mock illustrates layout only. Production must retain server-calculated dates, timezone rules, and validation.

### 2. Make task confirmations describe one decision

**Observed in source:** Starting a task opens “Queue this task?” above a “Start task” button.
The subtitle describes version authorization. Cancelling a task can produce two buttons labeled “Cancel.”

**Suggestion:** Use “Start this task?” and name the task in the subtitle.
Explain: “Noema will plan this task and add it to the queue.”
For cancellation, use **Keep task** and **Cancel task**.
Describe retained history without promising that every external operation stops immediately.
Keep the current confirmation points. Do not add confirmations to routine actions such as Run now.

**Mocks:** [Start task](http://localhost:8766/#start), [Cancel task](http://localhost:8766/#cancel).
**Source:** [TaskActionDialog](../../../apps/web/src/components/tasks/TaskActionDialog.tsx), [taskActionModel](../../../apps/web/src/components/tasks/taskActionModel.ts).

### 3. Give Inbox tasks an intentional waiting state

**Observed in source:** Tasks without runs use “No agent run yet” and “No output yet.”
These describe missing system activity instead of the person's next action.

**Suggestion:** Keep the instructions prominent. Show “Ready when you are” beside Start task and Schedule.
Keep provenance visible. Place revision and other inspection details below the instructions.
Retain workspace files and transcript access when they contain useful content.
The cropped mock shows the unstarted state, not a replacement for completed task details.

**Mock:** [Inbox task](http://localhost:8766/#inbox).
**Source:** [TaskDetailPanel](../../../apps/web/src/components/chatDetail/task/TaskDetailPanel.tsx), [TaskBody](../../../apps/web/src/components/chatDetail/task/TaskBody.tsx).

### 4. Make the chosen model readable

**Observed in the app:** Model names truncate in Agents while reasoning and Fast controls remain visible.
The empty ACP group also separates agent models from task models.

**Suggestion:** Give the chosen model enough width to show its name.
Move reasoning, Fast, and task-model enablement into row-level Options.
Keep a visible Off state for disabled task models. Show changed options in a concise row summary when useful.
Keep agent models and task models adjacent. Put external agent setup after them on the same page.
Retain direct saving for inline choices. Use one local saving, saved, or error state at the affected row.

**Mock:** [Agent and task models](http://localhost:8766/#agents).
**Source:** [AgentsSettingsPane](../../../apps/web/src/components/settings/AgentsSettingsPane.tsx), [ControlledModelPreferenceSelect](../../../apps/web/src/components/settings/ControlledModelPreferenceSelect.tsx), [TaskModelPoolsSettings](../../../apps/web/src/components/settings/TaskModelPoolsSettings.tsx).

## Priority 2: improve hierarchy and consistency

### 5. Make recurring history answer “Did it work?”

**Observed in source:** Linked occurrence rows repeat “Open task.” They do not show task outcomes.
Schedule metadata and description labels use several small type sizes and different left offsets.

**Suggestion:** Emphasize the next date in one compact green summary.
Align instructions and history to one left edge. Use the existing body and heading tokens.
Use **Run history** instead of **Occurrences**. Show the task outcome where available.
Keep schedule state separate from the state of an individual run.
Show skipped runs without suggesting a task exists. Keep destructive schedule actions in the existing menu.

**Mock:** [Recurring task](http://localhost:8766/#recurring).
**Source:** [TaskRecurrenceDetailPanel](../../../apps/web/src/components/tasks/TaskRecurrenceDetailPanel.tsx).
Outcome labels require linked task status. Creation of an occurrence does not prove successful completion.
The mock's result summaries are examples. Do not add invented summaries or a second task-status authority.

### 6. Reduce provider details to useful facts

**Observed in the app:** Authentication, Status, and Default account each occupy a full two-line row.
Capability identifiers and contract fields appear in the main content.

**Suggestion:** Put connection health beside the account name.
Use compact label/value rows for sign-in method and default status.
Use familiar capability names with brief explanations. Keep exact IDs and contracts under Technical details.
Keep account deletion available in a quiet lower section with explicit consequences.
Do not collapse useful connection errors or remove supported providers.

**Mock:** [Provider settings](http://localhost:8766/#providers).
**Source:** [ProvidersSettingsPane](../../../apps/web/src/components/settings/ProvidersSettingsPane.tsx).

### 7. Start notification settings with the current device

**Observed in the app:** Apple push server configuration appears before Device notifications.
The browser's installation requirement appears in the lower group.

**Suggestion:** Put this device's state and next step first.
Keep Apple delivery setup in a secondary section on the same page.
Show why notifications cannot be enabled when installation or permission is missing.
The mock's enabled state is illustrative; it must not replace actual permission and installation checks.

**Mock:** [Notification settings](http://localhost:8766/#notifications).
**Source:** [NotificationsSettingsPane](../../../apps/web/src/components/settings/NotificationsSettingsPane.tsx).

### 8. Give small dialogs one consistent finish

**Observed in source:** Task settings and task actions use hand-styled fields.
Task settings specifies 38px minimum field height. Other forms use Astryx fields.
Several dialog action rows live inside the scrolling form content.
In the rendered execution-limits dialog, Save limits and Cancel have the same neutral treatment.
The shared SettingsEditDialog does not set a primary variant on Save.

**Suggestion:** Reuse Astryx fields and one footer pattern within ResponsiveDialog.
Use 16px body spacing, 12px field groups, 6px label gaps, and 8px action gaps.
Use standard 32px buttons. Set an explicit primary variant on Save.
Pair equal-width actions with the primary action on the right.
Retain mobile drawers. Keep actions reachable when forms or validation messages become long.
Focus the first input on open. Keep draft, saving, stale-version, and field-error behavior intact.

**Mock:** [Task settings](http://localhost:8766/#task-settings). The gallery also opens actual modal and mobile drawer previews.
**Source:** [TaskSettingsDialog](../../../apps/web/src/components/tasks/TaskSettingsDialog.tsx), [SettingsEditDialog](../../../apps/web/src/components/settings/SettingsEditDialog.tsx), [ResponsiveDialog](../../../apps/web/src/components/ResponsiveDialog.tsx).

### 9. Make “save for later” visible during task capture

**Observed in the app and source:** Add to Inbox sits inside the Run Now split-button menu.
The project, schedule, agent, folder, and Markdown controls are all icon-only.

**Suggestion:** Preserve the current document editor. Refine its footer.
Show Add to Inbox beside Run now. Give project and scheduling controls visible labels.
Keep agent, folder, and source controls in Advanced. Show selected non-default values when they matter.
Do not turn capture into a multi-step form.

**Mock:** [New task](http://localhost:8766/#capture).
**Source:** [CaptureTaskDetail](../../../apps/web/src/components/tasks/CaptureTaskDetail.tsx).

## Smaller copy and finish changes

- On a completely empty Tasks page, replace “No matching history” with a first-task message.
  Use “No completed tasks yet” for empty history without filters. Reserve “matching” for active filters.
- In Memory settings, move “Local human only” into an explanation of access restrictions when needed.
  “Consolidation model” can become “Memory update model” without changing its function.
- Keep the existing green wash, clay primary action, fonts, corner shapes, and shadows.
- Use the existing list hover and focus treatments. Keep small status changes local to the affected control.
- Use warmth in waiting and success states. Keep errors and destructive decisions plain.
- Use `text-wrap: pretty` for explanatory text. Allow wrapping at phone widths rather than forcing one line.

## Suggested application order

First fix schedule editing, confirmation copy, and unreadable model selections.
Then refine Inbox status, recurring history, and capture controls.
Finally align settings rows and dialog spacing through the existing shared components.
Keep each change bounded. No navigation redesign, new typography, or new settings framework is needed.

See [validation](validation.md) for review coverage and [source](source/gallery.jsx) for the editable mocks.
