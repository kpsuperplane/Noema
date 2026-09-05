# Core app UI audit

Status: recommendations and mocks. Product UI changes are not approved or implemented.

The existing navigation, list/detail layout, typography, and surface treatment work well.
The main opportunity is clearer task states and more consistent controls within those surfaces.

## Open the mocks

[Open the running gallery](http://localhost:8766/#inbox).
Use the numbered links to inspect ten proposals. Select **Phone width** to narrow the surface.
At phone screen widths, the gallery uses the available width automatically.

The gallery includes Inbox details, recurring details, scheduling, provider settings, notifications,
inline advanced settings, start/cancel dialog copy, task capture, and agent model settings.
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
Show missed-run and overlap controls together below the main schedule. These two controls do not need a disclosure.

**Mock:** [Schedule dialog](http://localhost:8766/#schedule).
**Source:** [ScheduleFields](../../../apps/web/src/components/tasks/ScheduleFields.tsx), [recurrence editor](../../../apps/web/src/components/tasks/TaskRecurrenceDetailPanel.tsx).
The mock illustrates layout only. Production must retain server-calculated dates, timezone rules, and validation.

### 2. Make task confirmations describe one decision

**Observed during the initial audit:** Start confirmation copy used “Queue this task?” above a “Start task” button.
The current task bar starts tasks directly. Keep that behavior.
Cancelling a task can produce two buttons labeled “Cancel.”

**Suggestion:** Where confirmation is required, name the action and show the task in its existing list item card.
Place the card first in the dialog body. Remove the small task-title subtitle.
Use the same title, preview, and state arrangement as task lists. Keep the action explanation below the card.
Use this pattern for task confirmations, scheduling, and instruction edits. Selecting the card opens the task mock.
The standalone start dialog illustrates copy only. Do not insert it into the current start flow.
Explain: “Noema will plan this task and add it to the queue.”
For cancellation, use **Keep task** and **Cancel task**.
Describe retained history without promising that every external operation stops immediately.
Keep the current confirmation points. Do not add confirmations to Start task or Run now.

**Mocks:** [Start task](http://localhost:8766/#start), [Cancel task](http://localhost:8766/#cancel).
**Source:** [TaskActionDialog](../../../apps/web/src/components/tasks/TaskActionDialog.tsx), [taskActionModel](../../../apps/web/src/components/tasks/taskActionModel.ts).

### 3. Give Inbox tasks an intentional waiting state

**Observed in source:** Tasks without runs use “No agent run yet” and “No output yet.”
These describe missing system activity instead of the person's next action.

**Suggestion:** Keep the instructions prominent. Put Project and Task agent choices in the info row below the title.
Hide Working folder in task details. Remove repeated Inbox, added-time, source, and revision metadata.
Use the same timing summary below the title for Inbox and recurring tasks.
Show Schedule for an unscheduled task and Reschedule with a calendar icon for a scheduled task.
Keep Start task, Run now, and lifecycle actions in the existing floating task bar.
Show “Ready when you are” in that bar. Do not duplicate execution controls in the document.
Remove the separate detail header, task Settings button, and task-settings dialog.
For recurring tasks, expose Advanced settings directly below Instructions, before Run history.
Retain workspace files and transcript access when they contain useful content.
The cropped mock shows the unstarted state, not a replacement for completed task details.

**Mock:** [Inbox task](http://localhost:8766/#inbox).
**Source:** [TaskDetailPanel](../../../apps/web/src/components/chatDetail/task/TaskDetailPanel.tsx), [TaskBody](../../../apps/web/src/components/chatDetail/task/TaskBody.tsx).

### 4. Make the chosen model readable

**Observed in the app:** Model names truncate in Agents while reasoning and Fast controls remain visible.
The empty ACP group also separates agent models from task models.

**Suggestion:** Give the chosen model enough width to show its name.
Keep model, reasoning, Fast, and task-model enablement visible in each row.
Hide the Model and Reasoning labels. Keep accessible names that identify each model group.
Use a compact group with aligned controls. On phones, give the model selector its own full-width line.
Keep a visible Off state for disabled task models.
Keep agent models and task models adjacent. Put external agent setup after them on the same page.
Retain direct saving for inline choices. Use one local saving, saved, or error state at the affected row.

**Mock:** [Agent and task models](http://localhost:8766/#agents).
**Source:** [AgentsSettingsPane](../../../apps/web/src/components/settings/AgentsSettingsPane.tsx), [ControlledModelPreferenceSelect](../../../apps/web/src/components/settings/ControlledModelPreferenceSelect.tsx), [TaskModelPoolsSettings](../../../apps/web/src/components/settings/TaskModelPoolsSettings.tsx).

## Priority 2: improve hierarchy and consistency

### 5. Make recurring history answer “Did it work?”

**Observed in source:** Linked occurrence rows repeat “Open task.” They do not show task outcomes.
Schedule metadata and description labels use several small type sizes and different left offsets.

**Suggestion:** Emphasize the next date in the same compact timing summary used by Inbox tasks.
Label the calendar action Reschedule. Use Edit beside the Instructions heading in both views.
Keep execution and pause controls in the shared floating task bar.
Align instructions and history to one left edge. Use the existing body and heading tokens.
Use **Run history** instead of **Occurrences**. Reuse the existing task list item cards and TaskStatusBadge.
Keep the same title, timestamp, preview, and status arrangement. Show the task outcome where available.
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
Use familiar capability names with brief explanations. Show one or two short technical values directly.
Reserve Technical details disclosure for larger records.
Keep account deletion visible in a quiet lower section with explicit consequences.
Do not collapse useful connection errors or remove supported providers.

**Mock:** [Provider settings](http://localhost:8766/#providers).
**Source:** [ProvidersSettingsPane](../../../apps/web/src/components/settings/ProvidersSettingsPane.tsx).

### 7. Start notification settings with the current device

**Observed in the app:** Apple push server configuration appears before Device notifications.
The browser's installation requirement appears in the lower group.

**Suggestion:** Put this device's state and next step first.
Keep Apple delivery setup in a secondary section on the same page. Show its short explanation and Configure action directly.
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
Move Inbox project and agent choices into the info row below the task title.
Hide Working folder in task details. Keep recurring schedule settings above Run history.
Use direct saving for inline controls, with local saving, saved, and error feedback.
Inbox choices are Project and Task agent. Recurring fields are missed-run and overlap behavior.
Save selectors on selection. Keep accessible names when visible labels are hidden.
Focus the first input on open. Keep draft, saving, stale-version, and field-error behavior intact.

**Mocks:** [Task options](http://localhost:8766/#task-settings), [Schedule dialog](http://localhost:8766/#schedule).
The gallery also opens modal and mobile drawer previews for bounded edits.
**Source:** [TaskSettingsDialog](../../../apps/web/src/components/tasks/TaskSettingsDialog.tsx), [SettingsEditDialog](../../../apps/web/src/components/settings/SettingsEditDialog.tsx), [ResponsiveDialog](../../../apps/web/src/components/ResponsiveDialog.tsx).

### 9. Make “save for later” visible during task capture

**Observed in the app and source:** Add to Inbox sits inside the Run Now split-button menu.
The project, schedule, agent, folder, and Markdown controls are all icon-only.

**Suggestion:** Preserve the current document editor. Remove the visible Task title and Instructions labels.
Keep accessible names on both fields. Refine the footer.
Show Add to Inbox beside Run now. Give project and scheduling controls visible labels.
Place the Advanced section above the action row, at full width.
Keep agent, folder, and source controls within that section. Show selected non-default values when they matter.
Expanding Advanced must not change the arrangement of the action buttons.
Do not turn capture into a multi-step form.

**Mock:** [New task](http://localhost:8766/#capture).
**Source:** [CaptureTaskDetail](../../../apps/web/src/components/tasks/CaptureTaskDetail.tsx).

## Shared task detail implementation direction

Reuse TaskBody, TaskContextCard, TaskActions, and TaskScheduleSummary as the existing foundations.
Inbox and recurrence views should share title, timing, instruction editing, advanced settings, and floating-bar composition.
Keep the bar's existing docking, status, accessibility, and response behavior when applying these proposals.
Preserve its compact status on the left and icon controls on the right.
Place Advanced settings before Run history so a long history cannot push the controls out of reach.
The sample bar represents that existing surface; it is not a second toolbar.
Use one Edit label beside Instructions. Reuse the existing task cards for run history.
Share field rendering and save feedback where semantics match. Keep task and recurrence commands at their existing authorities.
Preserve available-action checks, revision checks, stale-draft handling, and lifecycle restrictions.
Do not introduce a generic task settings service or a second command path.

Current recurrence reads do not expose project assignment. Recurrence updates do not support agent or folder overrides.
The mocks therefore show supported schedule policies for recurrences, rather than imply full field parity.
Adding these fields would require a separate product and data-contract decision.
The shared mock components demonstrate composition, not a production infrastructure change.

## Smaller copy and finish changes

Keep short information and compact controls visible. A disclosure must save meaningful space or hide a substantial secondary task.
Do not use disclosure for one or two small items.


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
