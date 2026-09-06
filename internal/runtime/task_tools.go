package runtime

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"os"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/schedule"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	taskCaptureName          = "task.capture"
	taskListName             = "task.list"
	taskUpdateName           = "task.update"
	taskQueueName            = "task.queue"
	taskScheduleName         = "task.schedule"
	taskRescheduleName       = "task.reschedule"
	taskUnscheduleName       = "task.unschedule"
	taskScheduleRunNowName   = "task.schedule.run_now"
	taskRecurrenceUpdateName = "task.recurrence.update"
	taskRecurrencePauseName  = "task.recurrence.pause"
	taskRecurrenceResumeName = "task.recurrence.resume"
	taskRecurrenceSkipName   = "task.recurrence.skip_next"
	taskRecurrenceEndName    = "task.recurrence.end"
	taskRecurrenceRunNowName = "task.recurrence.run_now"
	taskDelegateName         = "task.delegate"
	taskAnswerName           = "task.answer"
	taskRetryName            = "task.retry"
	taskCancelName           = "task.cancel"
	taskReopenName           = "task.reopen"
)

var taskToolSpecs = []provider.GenerationTool{
	{Name: taskCaptureName, Description: "Capture work in Inbox, optionally with future execution and Repeat.", InputSchema: json.RawMessage(`{"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"task_document":{"type":"string","maxLength":65536},"project_id":{"type":"string","minLength":1,"maxLength":255},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"schedule":{"type":"object","properties":{"scheduled_for":{"type":"string","format":"date-time"},"time_zone":{"type":"string"},"missed_run_policy":{"type":"string","enum":["skip","run_once"]},"recurrence":{"type":"object","properties":{"starts_at":{"type":"string","format":"date-time"},"cron_expression":{"type":"string"},"overlap_policy":{"type":"string","enum":["skip","queue_one","allow"]}},"required":["starts_at","cron_expression"],"additionalProperties":false}},"required":["scheduled_for"],"additionalProperties":false}},"required":["title"],"additionalProperties":false}`)},
	{Name: taskListName, Description: "List bounded owner-authorized Task summaries and active gates.", InputSchema: json.RawMessage(`{"type":"object","properties":{"project_id":{"type":"string","minLength":1},"stage_behavior":{"type":"string","enum":["intake","dispatch","active","human_gate","terminal_success","terminal_cancelled"]},"attention_only":{"type":"boolean"},"limit":{"type":"integer","minimum":1,"maximum":100},"cursor":{"type":"string","minLength":1}},"additionalProperties":false}`)},
	taskInspectTool(),
	{Name: taskUpdateName, Description: "Update an Inbox Task and its document with revision fences.", InputSchema: json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"title":{"type":"string","minLength":1,"maxLength":200},"task_document":{"type":"string","maxLength":65536},"project_id":{"type":"string"},"clear_project":{"type":"boolean"},"executor_agent_id":{"type":"string"},"cwd_override":{"type":"string"},"clear_cwd_override":{"type":"boolean"}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false}`)},
	{Name: taskQueueName, Description: "Authorize an Inbox Task for planning and execution.", InputSchema: taskFenceSchema()},
	{Name: taskScheduleName, Description: "Schedule an ordinary Inbox Task.", InputSchema: scheduledTaskSchema()},
	{Name: taskRescheduleName, Description: "Replace timing for a scheduled Inbox Task.", InputSchema: scheduledTaskSchema()},
	{Name: taskUnscheduleName, Description: "Remove future execution from a one-time Inbox Task.", InputSchema: taskFenceSchema()},
	{Name: taskScheduleRunNowName, Description: "Start an already scheduled Inbox Task immediately.", InputSchema: taskFenceSchema()},
	{Name: taskRecurrenceUpdateName, Description: "Edit the future template for a recurring Task.", InputSchema: json.RawMessage(`{"type":"object","properties":{"recurrence_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"title":{"type":"string"},"task_document":{"type":"string","maxLength":65536},"project_id":{"type":"string"},"clear_project":{"type":"boolean"},"starts_at":{"type":"string"},"cron_expression":{"type":"string"},"time_zone":{"type":"string"},"missed_run_policy":{"type":"string","enum":["skip","run_once"]},"overlap_policy":{"type":"string","enum":["skip","queue_one","allow"]}},"required":["recurrence_id","expected_revision"],"additionalProperties":false}`)},
	{Name: taskRecurrencePauseName, Description: "Pause future recurring materialization.", InputSchema: recurrenceFenceSchema()},
	{Name: taskRecurrenceResumeName, Description: "Resume a paused recurrence.", InputSchema: recurrenceFenceSchema()},
	{Name: taskRecurrenceSkipName, Description: "Skip the next exact recurring slot.", InputSchema: recurrenceFenceSchema()},
	{Name: taskRecurrenceEndName, Description: "End all future recurrence.", InputSchema: recurrenceFenceSchema()},
	{Name: taskRecurrenceRunNowName, Description: "Create and start one extra occurrence now.", InputSchema: recurrenceFenceSchema()},
	{Name: taskDelegateName, Description: "Capture and authorize one autonomous Task. For direct execution, supply execution_intent and omit complexity_hint. Otherwise, complexity_hint selects planning complexity.", InputSchema: json.RawMessage(`{"type":"object","properties":{"title":{"type":"string","minLength":1,"maxLength":200},"task_document":{"type":"string","minLength":1,"maxLength":65536},"project":{"oneOf":[{"type":"object","properties":{"kind":{"type":"string","enum":["none"]}},"required":["kind"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["existing"]},"project_id":{"type":"string","minLength":1,"maxLength":255}},"required":["kind","project_id"],"additionalProperties":false}]},"executor_agent_id":{"type":"string","minLength":1,"maxLength":255},"cwd_override":{"type":"string","minLength":1,"maxLength":4096},"complexity_hint":{"type":"string","enum":["simple","medium","difficult"]},"execution_intent":{"type":"object","properties":{"request_markdown":{"type":"string","minLength":1,"maxLength":20000},"complexity":{"type":"string","enum":["simple","medium","difficult"]}},"required":["request_markdown","complexity"],"additionalProperties":false}},"required":["title","task_document","project"],"additionalProperties":false}`)},
	{Name: taskAnswerName, Description: "Answer the exact active Task gate.", InputSchema: json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer"},"expected_generation":{"type":"integer"},"answer_markdown":{"type":"string"},"approval_decision":{"type":"string","enum":["approved","declined"]}},"required":["task_id","gate_id","expected_revision","expected_generation","answer_markdown"],"additionalProperties":false}`)},
	{Name: taskRetryName, Description: "Retry the explicitly named eligible Recovery gate.", InputSchema: json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"gate_id":{"type":"string"},"expected_revision":{"type":"integer"},"expected_generation":{"type":"integer"},"retry_note":{"type":"string"}},"required":["task_id","gate_id","expected_revision","expected_generation"],"additionalProperties":false}`)},
	{Name: taskCancelName, Description: "Cancel a nonterminal Task and fence active work.", InputSchema: json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer"},"expected_generation":{"type":"integer"},"reason":{"type":"string"}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false}`)},
	{Name: taskReopenName, Description: "Reopen a completed Task into Queue with new direction.", InputSchema: json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer"},"expected_generation":{"type":"integer"},"feedback_markdown":{"type":"string"},"request_markdown":{"type":"string"},"complexity":{"type":"string","enum":["simple","medium","difficult"]}},"required":["task_id","expected_revision","expected_generation","feedback_markdown"],"additionalProperties":false}`)},
}

func taskFenceSchema() json.RawMessage {
	return json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1}},"required":["task_id","expected_revision","expected_generation"],"additionalProperties":false}`)
}

func recurrenceFenceSchema() json.RawMessage {
	return json.RawMessage(`{"type":"object","properties":{"recurrence_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1}},"required":["recurrence_id","expected_revision"],"additionalProperties":false}`)
}

func scheduledTaskSchema() json.RawMessage {
	return json.RawMessage(`{"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"expected_generation":{"type":"integer","minimum":1},"scheduled_for":{"type":"string","format":"date-time"},"time_zone":{"type":"string"},"missed_run_policy":{"type":"string","enum":["skip","run_once"]},"recurrence":{"type":"object","properties":{"starts_at":{"type":"string","format":"date-time"},"cron_expression":{"type":"string"},"overlap_policy":{"type":"string","enum":["skip","queue_one","allow"]}},"required":["starts_at","cron_expression"],"additionalProperties":false}},"required":["task_id","expected_revision","expected_generation","scheduled_for"],"additionalProperties":false}`)
}

func isPrimaryTaskTool(name string) bool {
	for _, tool := range taskToolSpecs {
		if tool.Name == name {
			return true
		}
	}
	return false
}

func taskToolHasSideEffect(name string) bool {
	return name == fileDownloadName || isPrimaryTaskTool(name) && name != taskListName && name != taskInspectName
}

func (c *Chat) executePrimaryTaskTool(ctx context.Context, name, requestID, correlationID string, raw json.RawMessage,
	source store.ArtifactSource, sourceTimeZone string) (json.RawMessage, bool) {
	if requestID == "" || len(raw) > modelToolPayloadLimit {
		return toolFailure("invalid_input", "Task command is invalid"), false
	}
	fields, err := strictProjectFields(raw)
	if err != nil {
		return toolFailure("invalid_input", "Task tool arguments are invalid"), false
	}
	command := taskModelCommand(name, requestID, correlationID, raw)
	now := time.Now()
	var result store.TaskCommandResult
	switch name {
	case taskCaptureName, taskDelegateName:
		return c.captureTaskTool(ctx, name, fields, command, now, source, requestID, sourceTimeZone)
	case taskListName:
		return c.listTaskTool(ctx, fields)
	case taskInspectName:
		return c.inspectTask(ctx, raw)
	case taskUpdateName:
		return c.updateTaskTool(ctx, fields, command, now)
	case taskQueueName, taskUnscheduleName, taskScheduleRunNowName, taskCancelName:
		allowed := []string{"task_id", "expected_revision", "expected_generation"}
		if name == taskCancelName {
			allowed = append(allowed, "reason")
		}
		if !onlyProjectFields(fields, allowed...) {
			return taskToolInputFailure()
		}
		id, revision, generation, ok := taskFence(fields)
		if !ok {
			return taskToolInputFailure()
		}
		if (name == taskUnscheduleName || name == taskScheduleRunNowName) && generation != 1 {
			return taskToolInputFailure()
		}
		switch name {
		case taskQueueName:
			result, err = c.database.QueueTask(ctx, id, revision, generation, command, now)
		case taskUnscheduleName:
			result, err = c.database.UnscheduleTask(ctx, id, revision, command, now)
		case taskScheduleRunNowName:
			result, err = c.database.RunScheduledTaskNow(ctx, id, revision, command, now)
		default:
			reason, valid := taskOptionalString(fields, "reason", 4000)
			if !valid {
				return taskToolInputFailure()
			}
			result, err = c.database.CancelTask(ctx, id, revision, generation, stringValue(reason), command, now)
		}
	case taskScheduleName, taskRescheduleName:
		return c.setTaskScheduleTool(ctx, fields, command, now, name == taskRescheduleName, sourceTimeZone)
	case taskRecurrenceUpdateName:
		return c.updateRecurrenceTool(ctx, fields, command, now)
	case taskRecurrencePauseName, taskRecurrenceResumeName, taskRecurrenceSkipName,
		taskRecurrenceEndName, taskRecurrenceRunNowName:
		return c.recurrenceCommandTool(ctx, name, fields, command, now)
	case taskAnswerName, taskRetryName:
		return c.resolveTaskGateTool(ctx, name, fields, command, now)
	case taskReopenName:
		return c.reopenTaskTool(ctx, fields, command, now)
	default:
		return taskToolInputFailure()
	}
	if err != nil {
		return taskToolError(err)
	}
	c.database.NotifyWork()
	return marshalTaskResult(ctx, c.database, result)
}

func taskModelCommand(name, requestID, correlationID string, raw json.RawMessage) store.TaskCommand {
	digest := sha256.Sum256(raw)
	if correlationID == "" {
		correlationID = "correlation:task-tool:" + requestID
	}
	return store.TaskCommand{Name: strings.ReplaceAll(strings.TrimPrefix(name, "task."), ".", "_") + "_task_tool",
		ClientMutationID: requestID, RequestDigest: hex.EncodeToString(digest[:]), CorrelationID: correlationID}
}

func (c *Chat) captureTaskTool(ctx context.Context, name string, fields map[string]json.RawMessage, command store.TaskCommand, now time.Time,
	source store.ArtifactSource, sourceToolCallID, sourceTimeZone string) (json.RawMessage, bool) {
	allowed := []string{"title", "task_document", "project_id", "executor_agent_id", "cwd_override", "schedule"}
	if name == taskDelegateName {
		allowed = []string{"title", "task_document", "project", "executor_agent_id", "cwd_override", "complexity_hint", "execution_intent"}
	}
	if !onlyProjectFields(fields, allowed...) {
		return taskToolInputFailure()
	}
	title, titleOK := taskString(fields, "title", true, 200)
	document, documentOK := taskOptionalString(fields, "task_document", 65536)
	if !titleOK || !documentOK || name == taskDelegateName && (document == nil || *document == "") {
		return taskToolInputFailure()
	}
	options := store.TaskCreateOptions{ExecutorAgentID: store.TaskExecutorAgentID, Source: source,
		SourceToolCallID: sourceToolCallID, SourceClientTimeZone: sourceTimeZone}
	if value, ok := taskOptionalString(fields, "executor_agent_id", 255); !ok {
		return taskToolInputFailure()
	} else if value != nil {
		options.ExecutorAgentID = *value
	}
	if value, ok := taskOptionalString(fields, "cwd_override", 4096); !ok {
		return taskToolInputFailure()
	} else {
		options.CwdOverride = value
	}
	if name == taskDelegateName {
		var placement struct {
			Kind      string `json:"kind"`
			ProjectID string `json:"project_id"`
		}
		rawPlacement, exists := fields["project"]
		placementFields, placementErr := strictProjectFields(rawPlacement)
		if !exists || placementErr != nil || !onlyProjectFields(placementFields, "kind", "project_id") ||
			decodeToolArguments(rawPlacement, &placement) != nil || placement.Kind == "none" && placement.ProjectID != "" ||
			(placement.Kind != "none" && (placement.Kind != "existing" || placement.ProjectID == "")) {
			return taskToolInputFailure()
		}
		options.ProjectID = placement.ProjectID
		complexity, ok := taskOptionalEnum(fields, "complexity_hint", "simple", "medium", "difficult")
		if !ok {
			return taskToolInputFailure()
		}
		if rawIntent, exists := fields["execution_intent"]; exists {
			if complexity != nil {
				return toolFailure("invalid_input", "Omit complexity_hint when execution_intent is supplied"), false
			}
			var intent struct {
				RequestMarkdown string `json:"request_markdown"`
				Complexity      string `json:"complexity"`
			}
			intentFields, intentErr := strictProjectFields(rawIntent)
			if intentErr != nil || !onlyProjectFields(intentFields, "request_markdown", "complexity") ||
				decodeToolArguments(rawIntent, &intent) != nil || strings.TrimSpace(intent.RequestMarkdown) == "" ||
				utf8.RuneCountInString(intent.RequestMarkdown) > 20000 || !taskEnum(intent.Complexity, "simple", "medium", "difficult") {
				return taskToolInputFailure()
			}
			options.InitialRunKind, options.ExecutionComplexity = "executor", intent.Complexity
		} else {
			options.InitialRunKind = "planner"
			if complexity != nil {
				options.ExecutionComplexity = *complexity
			}
		}
	} else {
		if value, ok := taskOptionalString(fields, "project_id", 255); !ok {
			return taskToolInputFailure()
		} else if value != nil {
			options.ProjectID = *value
		}
		if rawSchedule, exists := fields["schedule"]; exists {
			value, err := parseTaskSchedule(rawSchedule, now, sourceTimeZone)
			if err != nil {
				return taskToolInputFailure()
			}
			options.Schedule = value
		}
	}
	if replay, found, err := c.database.LookupTaskCommandReceipt(ctx, command); err != nil || found {
		if err != nil {
			return taskToolError(err)
		}
		if replay.RecurrenceID != "" {
			document, readErr := home.ReadTaskDocument(c.home, replay.Task.ID)
			if readErr != nil {
				return taskToolError(readErr)
			}
			if _, ensureErr := home.EnsureRecurrenceDocument(c.home, replay.RecurrenceID, document.Content); ensureErr != nil {
				return taskToolError(ensureErr)
			}
		}
		return marshalTaskResult(ctx, c.database, replay)
	}
	id, err := store.NewTaskID()
	if err != nil {
		return taskToolError(err)
	}
	content := stringValue(document)
	doc, err := home.CreatePendingTaskDocument(c.home, id, content)
	if err != nil {
		return taskToolError(err)
	}
	result, err := c.database.CreateTaskWithOptions(ctx, id, title, command, options, now)
	if err != nil {
		stored, _ := c.database.TaskExists(ctx, id)
		if stored {
			_ = home.CommitTaskDocument(c.home, id)
		} else {
			_ = home.DiscardPendingTaskDocument(c.home, id)
		}
		return taskToolError(err)
	}
	if err = home.CommitTaskDocument(c.home, id); err != nil {
		return taskToolError(err)
	}
	if result.RecurrenceID != "" {
		if _, err = home.EnsureRecurrenceDocument(c.home, result.RecurrenceID, doc.Content); err != nil {
			return taskToolError(err)
		}
	}
	c.database.NotifyWork()
	return marshalTaskResult(ctx, c.database, result)
}

func (r *TaskExecution) captureScopedTask(ctx context.Context, parent store.Task, run store.TaskRun, raw json.RawMessage) (json.RawMessage, bool) {
	fields, err := strictProjectFields(raw)
	if err != nil || !onlyProjectFields(fields, "title", "task_document", "schedule") {
		return taskToolInputFailure()
	}
	title, titleOK := taskString(fields, "title", true, 200)
	document, documentOK := taskOptionalString(fields, "task_document", 65536)
	if !titleOK || !documentOK {
		return taskToolInputFailure()
	}
	options := store.TaskCreateOptions{ProjectID: parent.ProjectID, ExecutorAgentID: parent.ExecutorAgentID,
		CwdOverride: parent.CwdOverride, Source: parent.Source, SourceClientTimeZone: parent.SourceClientTimeZone}
	if rawSchedule, exists := fields["schedule"]; exists {
		value, parseErr := parseTaskSchedule(rawSchedule, time.Now(), parent.SourceClientTimeZone)
		if parseErr != nil {
			return taskToolInputFailure()
		}
		options.Schedule = value
	}
	digest := sha256.Sum256(raw)
	requestID := run.ID + ":capture:" + hex.EncodeToString(digest[:8])
	options.SourceToolCallID = requestID
	command := taskModelCommand(taskCaptureName, requestID, "correlation:task:"+parent.ID, raw)
	if replay, found, lookupErr := r.database.LookupTaskCommandReceipt(ctx, command); lookupErr != nil || found {
		if lookupErr != nil {
			return taskToolError(lookupErr)
		}
		return marshalTaskResult(ctx, r.database, replay)
	}
	id, err := store.NewTaskID()
	if err != nil {
		return taskToolError(err)
	}
	doc, err := home.CreatePendingTaskDocument(r.root, id, stringValue(document))
	if err != nil {
		return taskToolError(err)
	}
	result, err := r.database.CreateTaskWithOptions(ctx, id, title, command, options, time.Now())
	if err != nil {
		if stored, _ := r.database.TaskExists(ctx, id); stored {
			_ = home.CommitTaskDocument(r.root, id)
		} else {
			_ = home.DiscardPendingTaskDocument(r.root, id)
		}
		return taskToolError(err)
	}
	if err = home.CommitTaskDocument(r.root, id); err != nil {
		return taskToolError(err)
	}
	if result.RecurrenceID != "" {
		if _, err = home.EnsureRecurrenceDocument(r.root, result.RecurrenceID, doc.Content); err != nil {
			return taskToolError(err)
		}
	}
	r.database.NotifyWork()
	return marshalTaskResult(ctx, r.database, result)
}

func (c *Chat) listTaskTool(ctx context.Context, fields map[string]json.RawMessage) (json.RawMessage, bool) {
	if !onlyProjectFields(fields, "project_id", "stage_behavior", "attention_only", "limit", "cursor") {
		return taskToolInputFailure()
	}
	filter := store.TaskListFilter{Scope: "all"}
	if value, ok := taskOptionalString(fields, "project_id", 255); !ok {
		return taskToolInputFailure()
	} else if value != nil {
		filter.ProjectID = *value
	}
	if value, ok := taskOptionalEnum(fields, "stage_behavior", "intake", "dispatch", "active", "human_gate", "terminal_success", "terminal_cancelled"); !ok {
		return taskToolInputFailure()
	} else if value != nil {
		filter.StageKeys = []string{map[string]string{"intake": "inbox", "dispatch": "queue", "active": "doing",
			"human_gate": "waiting", "terminal_success": "done", "terminal_cancelled": "cancelled"}[*value]}
	}
	attention, ok := projectOptionalBool(fields, "attention_only")
	limit, limitOK := projectOptionalInt(fields, "limit", 1, 100)
	cursor, cursorOK := taskOptionalString(fields, "cursor", modelToolPayloadLimit)
	if !ok || !limitOK || !cursorOK {
		return taskToolInputFailure()
	}
	filter.AttentionOnly = attention
	if limit == 0 {
		limit = 50
	}
	page, err := c.database.ListTasks(ctx, filter, limit, cursor)
	if err != nil {
		return taskToolError(err)
	}
	values := make([]map[string]any, 0, len(page.Tasks))
	for _, task := range page.Tasks {
		value, valueErr := taskSummaryValue(ctx, c.database, c.home, task)
		if valueErr != nil {
			return taskToolError(valueErr)
		}
		values = append(values, value)
	}
	payload, _ := json.Marshal(map[string]any{"tasks": values, "has_next_page": page.HasNextPage, "end_cursor": page.EndCursor})
	return boundedModelToolPayload(payload, modelToolResultLimit), true
}

func taskSummaryValue(ctx context.Context, database *store.Store, root *os.Root, task store.Task) (map[string]any, error) {
	value := taskValue(task)
	document, err := home.ReadTaskDocument(root, task.ID)
	if err != nil {
		return nil, err
	}
	value["task_document_preview"] = string([]rune(document.Content)[:min(280, utf8.RuneCountInString(document.Content))])
	value["attention"], value["valid_actions"] = nil, taskValidActions(task, nil)
	if task.ActiveGateID != "" {
		if gate, gateErr := database.TaskGate(ctx, task.ActiveGateID); gateErr == nil {
			value["active_gate"] = map[string]any{"gate_id": gate.ID, "kind": gate.Kind, "prompt": gate.Prompt,
				"context": gate.Context, "suggested_answers": gate.SuggestedAnswers}
			value["attention"], value["valid_actions"] = gate.Kind, taskValidActions(task, &gate)
		}
	}
	if task.RecurrenceID != "" {
		if recurrence, recurrenceErr := database.TaskRecurrence(ctx, task.RecurrenceID); recurrenceErr == nil {
			authority := recurrenceValue(recurrence)
			if recurrenceDocument, readErr := home.ReadRecurrenceDocument(root, recurrence.ID); readErr == nil {
				authority["task_document_preview"] = string([]rune(recurrenceDocument.Content)[:min(1000, utf8.RuneCountInString(recurrenceDocument.Content))])
			}
			authority["pending_coalesced_at"] = timeValue(recurrence.PendingCoalescedAt)
			value["recurrence_authority"] = authority
		}
	}
	return value, nil
}

func taskValidActions(task store.Task, gate *store.TaskGate) []string {
	switch task.StageKey {
	case "", "inbox":
		if task.RecurrenceID != "" {
			return []string{"run_now", "cancel"}
		}
		if task.ScheduledFor != nil {
			return []string{"run_now", "edit", "reschedule", "unschedule", "cancel"}
		}
		return []string{"edit", "queue", "schedule", "cancel"}
	case "queue", "doing":
		return []string{"cancel"}
	case "done", "cancelled":
		return []string{"reopen"}
	case "waiting":
		result := []string{}
		if gate != nil && gate.AllowsResolution("answer") {
			result = append(result, "answer")
		}
		if gate != nil && gate.AllowsResolution("retry") {
			result = append(result, "retry")
		}
		return append(result, "cancel")
	default:
		return []string{}
	}
}

func (c *Chat) updateTaskTool(ctx context.Context, fields map[string]json.RawMessage, command store.TaskCommand, now time.Time) (json.RawMessage, bool) {
	if !onlyProjectFields(fields, "task_id", "expected_revision", "expected_generation", "title", "task_document", "project_id", "clear_project", "executor_agent_id", "cwd_override", "clear_cwd_override") {
		return taskToolInputFailure()
	}
	id, revision, generation, ok := taskFence(fields)
	if !ok {
		return taskToolInputFailure()
	}
	changes := store.TaskUpdate{}
	if value, valid := taskOptionalString(fields, "title", 200); !valid {
		return taskToolInputFailure()
	} else {
		changes.Title = value
	}
	projectID, projectOK := taskOptionalString(fields, "project_id", 255)
	clearProject, clearProjectOK := projectOptionalBool(fields, "clear_project")
	cwd, cwdOK := taskOptionalString(fields, "cwd_override", 4096)
	clearCWD, clearCWDOK := projectOptionalBool(fields, "clear_cwd_override")
	executor, executorOK := taskOptionalString(fields, "executor_agent_id", 255)
	if !projectOK || !clearProjectOK || !cwdOK || !clearCWDOK || !executorOK || projectID != nil && strings.TrimSpace(*projectID) == "" || projectID != nil && clearProject || cwd != nil && clearCWD {
		return taskToolInputFailure()
	}
	changes.ProjectID, changes.SetProject = projectID, projectID != nil || clearProject
	changes.CwdOverride, changes.SetCwd, changes.ExecutorAgentID = cwd, cwd != nil || clearCWD, executor
	var stage *home.TaskDocumentStage
	if document, valid := taskOptionalString(fields, "task_document", 65536); !valid {
		return taskToolInputFailure()
	} else if document != nil {
		current, err := home.ReadTaskDocument(c.home, id)
		if err != nil {
			return taskToolError(err)
		}
		prepared, err := home.PrepareTaskDocumentReplace(c.home, id, current.Digest, *document, command.RequestDigest)
		if err != nil {
			return taskToolError(err)
		}
		stage, changes.DocumentDigest = &prepared, prepared.Document.Digest
	}
	if changes.Title == nil && !changes.SetProject && !changes.SetCwd && changes.ExecutorAgentID == nil && stage == nil {
		return taskToolInputFailure()
	}
	result, err := c.database.UpdateInboxTask(ctx, id, revision, generation, changes, command, now)
	if err != nil {
		if stage != nil {
			if replay, found, lookupErr := c.database.LookupTaskCommandReceipt(ctx, command); found && lookupErr == nil {
				if _, recoverErr := home.RecoverTaskDocumentStage(c.home, replay.Task.ID, command.RequestDigest, replay.DocumentDigest); recoverErr != nil {
					return taskToolError(recoverErr)
				}
				return marshalTaskResult(ctx, c.database, replay)
			}
			_ = home.DiscardTaskDocumentStage(c.home, id, command.RequestDigest)
		}
		return taskToolError(err)
	}
	if stage != nil {
		if _, err = home.CommitTaskDocumentStage(c.home, *stage); err != nil {
			return taskToolError(err)
		}
	}
	return marshalTaskResult(ctx, c.database, result)
}

func (c *Chat) setTaskScheduleTool(ctx context.Context, fields map[string]json.RawMessage, command store.TaskCommand, now time.Time, replace bool, sourceTimeZone string) (json.RawMessage, bool) {
	if !onlyProjectFields(fields, "task_id", "expected_revision", "expected_generation", "scheduled_for", "time_zone", "missed_run_policy", "recurrence") {
		return taskToolInputFailure()
	}
	id, revision, generation, ok := taskFence(fields)
	scheduleFields := make(map[string]json.RawMessage, len(fields)-3)
	for _, key := range []string{"scheduled_for", "time_zone", "missed_run_policy", "recurrence"} {
		if raw, exists := fields[key]; exists {
			scheduleFields[key] = raw
		}
	}
	rawSchedule, _ := json.Marshal(scheduleFields)
	value, err := parseTaskSchedule(rawSchedule, now, sourceTimeZone)
	if !ok || generation != 1 || err != nil {
		return taskToolInputFailure()
	}
	unlock := c.database.LockTaskSchedules()
	defer unlock()
	result, err := c.database.SetTaskSchedule(ctx, id, revision, *value, replace, command, now)
	if err != nil {
		return taskToolError(err)
	}
	if result.RecurrenceID != "" {
		document, readErr := home.ReadTaskDocument(c.home, id)
		if readErr != nil {
			return taskToolError(readErr)
		}
		if _, err = home.EnsureRecurrenceDocument(c.home, result.RecurrenceID, document.Content); err != nil {
			return taskToolError(err)
		}
	}
	if result.ObsoleteRecurrenceID != "" {
		if err = home.DeleteRecurrenceDocument(c.home, result.ObsoleteRecurrenceID); err != nil {
			return taskToolError(err)
		}
	}
	c.database.NotifyWork()
	return marshalTaskResult(ctx, c.database, result)
}

func (c *Chat) updateRecurrenceTool(ctx context.Context, fields map[string]json.RawMessage, command store.TaskCommand, now time.Time) (json.RawMessage, bool) {
	if !onlyProjectFields(fields, "recurrence_id", "expected_revision", "title", "task_document", "project_id", "clear_project", "starts_at", "cron_expression", "time_zone", "missed_run_policy", "overlap_policy") {
		return taskToolInputFailure()
	}
	id, ok := taskString(fields, "recurrence_id", true, 255)
	revision, revisionOK := projectOptionalInt(fields, "expected_revision", 1, int(^uint(0)>>1))
	if !ok || !revisionOK || revision == 0 {
		return taskToolInputFailure()
	}
	changes := store.RecurrenceChanges{}
	if value, valid := taskOptionalString(fields, "title", 500); !valid {
		return taskToolInputFailure()
	} else {
		changes.Title = value
	}
	projectID, projectOK := taskOptionalString(fields, "project_id", 255)
	clearProject, clearOK := projectOptionalBool(fields, "clear_project")
	if projectID != nil && strings.TrimSpace(*projectID) == "" {
		projectID = nil
	}
	if !projectOK || !clearOK || projectID != nil && clearProject {
		return taskToolInputFailure()
	}
	changes.ProjectID, changes.SetProject = projectID, projectID != nil || clearProject
	if value, valid := taskOptionalString(fields, "starts_at", 100); !valid {
		return taskToolInputFailure()
	} else if value != nil {
		parsed, err := schedule.ParseInstant(*value)
		if err != nil {
			return taskToolInputFailure()
		}
		changes.StartsAt = &parsed
	}
	if value, valid := taskOptionalString(fields, "cron_expression", 1000); !valid {
		return taskToolInputFailure()
	} else if value != nil {
		normalized := strings.Join(strings.Fields(*value), " ")
		changes.CronExpression = &normalized
	}
	if value, valid := taskOptionalString(fields, "time_zone", 255); !valid {
		return taskToolInputFailure()
	} else {
		changes.TimeZone = value
	}
	if value, valid := taskOptionalEnum(fields, "missed_run_policy", "skip", "run_once"); !valid {
		return taskToolInputFailure()
	} else if value != nil {
		policy := schedule.MissedRunPolicy(*value)
		changes.MissedRunPolicy = &policy
	}
	if value, valid := taskOptionalEnum(fields, "overlap_policy", "skip", "queue_one", "allow"); !valid {
		return taskToolInputFailure()
	} else if value != nil {
		policy := schedule.OverlapPolicy(*value)
		changes.OverlapPolicy = &policy
	}
	var stage *home.RecurrenceDocumentStage
	if document, valid := taskOptionalString(fields, "task_document", 65536); !valid {
		return taskToolInputFailure()
	} else if document != nil {
		current, err := home.ReadRecurrenceDocument(c.home, id)
		if err != nil {
			return taskToolError(err)
		}
		prepared, err := home.PrepareRecurrenceDocumentReplace(c.home, id, current.Digest, *document, command.RequestDigest)
		if err != nil {
			return taskToolError(err)
		}
		stage = &prepared
		changes.DocumentChanged = true
	}
	if !changes.SetProject && changes.Title == nil && changes.StartsAt == nil && changes.CronExpression == nil &&
		changes.TimeZone == nil && changes.MissedRunPolicy == nil && changes.OverlapPolicy == nil && stage == nil {
		return taskToolInputFailure()
	}
	unlock := c.database.LockTaskSchedules()
	defer unlock()
	result, err := c.database.UpdateTaskRecurrence(ctx, id, int64(revision), changes, command, now)
	if err != nil {
		if stage != nil {
			if replay, found, lookupErr := c.database.LookupTaskCommandReceipt(ctx, command); found && lookupErr == nil {
				if _, recoverErr := home.CommitRecurrenceDocumentStage(c.home, *stage); recoverErr != nil {
					return taskToolError(recoverErr)
				}
				return marshalTaskResult(ctx, c.database, replay)
			}
			_ = home.DiscardRecurrenceDocumentStage(c.home, command.RequestDigest)
		}
		return taskToolError(err)
	}
	if stage != nil {
		if _, err = home.CommitRecurrenceDocumentStage(c.home, *stage); err != nil {
			return taskToolError(err)
		}
	}
	c.database.NotifyWork()
	return marshalTaskResult(ctx, c.database, result)
}

func (c *Chat) recurrenceCommandTool(ctx context.Context, name string, fields map[string]json.RawMessage, command store.TaskCommand, now time.Time) (json.RawMessage, bool) {
	if !onlyProjectFields(fields, "recurrence_id", "expected_revision") {
		return taskToolInputFailure()
	}
	id, ok := taskString(fields, "recurrence_id", true, 255)
	revision, revisionOK := projectOptionalInt(fields, "expected_revision", 1, int(^uint(0)>>1))
	if !ok || !revisionOK || revision == 0 {
		return taskToolInputFailure()
	}
	var result store.TaskCommandResult
	var err error
	switch name {
	case taskRecurrencePauseName:
		result, err = c.database.SetTaskRecurrenceLifecycle(ctx, id, int64(revision), store.RecurrencePaused, command, now)
	case taskRecurrenceResumeName:
		result, err = c.database.SetTaskRecurrenceLifecycle(ctx, id, int64(revision), store.RecurrenceActive, command, now)
	case taskRecurrenceEndName:
		result, err = c.database.SetTaskRecurrenceLifecycle(ctx, id, int64(revision), store.RecurrenceEnded, command, now)
	case taskRecurrenceSkipName:
		result, err = c.database.SkipTaskRecurrenceNext(ctx, id, int64(revision), command, now)
	case taskRecurrenceRunNowName:
		unlock := c.database.LockTaskSchedules()
		defer unlock()
		if replay, found, lookupErr := c.database.LookupTaskCommandReceipt(ctx, command); lookupErr != nil || found {
			if lookupErr != nil {
				return taskToolError(lookupErr)
			}
			if copyErr := home.CopyRecurrenceDocumentToTask(c.home, id, replay.Task.ID); copyErr != nil {
				return taskToolError(copyErr)
			}
			return marshalTaskResult(ctx, c.database, replay)
		}
		taskID, createErr := store.NewTaskID()
		if createErr != nil || home.StageRecurrenceDocumentToTask(c.home, id, taskID) != nil {
			return taskToolError(errors.New("recurrence document is unavailable"))
		}
		result, err = c.database.RunTaskRecurrenceNow(ctx, id, taskID, int64(revision), command, now)
		if err == nil {
			err = home.CommitTaskDocument(c.home, taskID)
		} else {
			if replay, found, lookupErr := c.database.LookupTaskCommandReceipt(ctx, command); found && lookupErr == nil && replay.Task.ID == taskID {
				err = home.CommitTaskDocument(c.home, taskID)
				result = replay
			} else {
				_ = home.DiscardPendingTaskDocument(c.home, taskID)
			}
		}
	}
	if err != nil {
		return taskToolError(err)
	}
	c.database.NotifyWork()
	return marshalTaskResult(ctx, c.database, result)
}

func (c *Chat) resolveTaskGateTool(ctx context.Context, name string, fields map[string]json.RawMessage, command store.TaskCommand, now time.Time) (json.RawMessage, bool) {
	allowed := []string{"task_id", "gate_id", "expected_revision", "expected_generation", "answer_markdown", "approval_decision"}
	if name == taskRetryName {
		allowed = []string{"task_id", "gate_id", "expected_revision", "expected_generation", "retry_note"}
	}
	if !onlyProjectFields(fields, allowed...) {
		return taskToolInputFailure()
	}
	id, revision, generation, ok := taskFence(fields)
	gateID, gateOK := taskString(fields, "gate_id", true, 255)
	if !ok || !gateOK {
		return taskToolInputFailure()
	}
	gate, err := c.database.TaskGate(ctx, gateID)
	if err != nil {
		return taskToolError(err)
	}
	resolution, body := "answer", ""
	var approval *string
	if name == taskRetryName {
		resolution, body = "retry", "Retry requested."
		if value, valid := taskOptionalString(fields, "retry_note", 4000); !valid {
			return taskToolInputFailure()
		} else if value != nil {
			body = strings.TrimSpace(*value)
		}
	} else {
		value, valid := taskString(fields, "answer_markdown", true, 20000)
		if !valid || strings.TrimSpace(value) == "" {
			return taskToolInputFailure()
		}
		body = strings.TrimSpace(value)
		approval, valid = taskOptionalEnum(fields, "approval_decision", "approved", "declined")
		if !valid || gate.Kind == "approval" != (approval != nil) {
			return taskToolInputFailure()
		}
	}
	if !gate.AllowsResolution(resolution) {
		return taskToolError(store.ErrInvalidTransition)
	}
	result, err := c.database.ResolveTaskGate(ctx, id, gateID, revision, generation, body, resolution, approval, command, now)
	if err != nil {
		return taskToolError(err)
	}
	c.database.NotifyWork()
	result.GateID = gateID
	return marshalTaskResult(ctx, c.database, result)
}

func (c *Chat) reopenTaskTool(ctx context.Context, fields map[string]json.RawMessage, command store.TaskCommand, now time.Time) (json.RawMessage, bool) {
	if !onlyProjectFields(fields, "task_id", "expected_revision", "expected_generation", "feedback_markdown", "request_markdown", "complexity") {
		return taskToolInputFailure()
	}
	id, revision, generation, ok := taskFence(fields)
	feedback, feedbackOK := taskString(fields, "feedback_markdown", true, 20000)
	replacement, replacementOK := taskOptionalString(fields, "request_markdown", 20000)
	complexity, complexityOK := taskOptionalEnum(fields, "complexity", "simple", "medium", "difficult")
	if !ok || !feedbackOK || !replacementOK || !complexityOK || strings.TrimSpace(feedback) == "" || replacement != nil && strings.TrimSpace(*replacement) == "" {
		return taskToolInputFailure()
	}
	direction := strings.TrimSpace(feedback)
	var stage *home.TaskDocumentStage
	documentDigest := ""
	if replacement != nil {
		direction += "\n\nUpdated request:\n\n" + strings.TrimSpace(*replacement)
		current, err := home.ReadTaskDocument(c.home, id)
		if err != nil {
			return taskToolError(err)
		}
		prepared, err := home.PrepareTaskDocumentReplace(c.home, id, current.Digest, strings.TrimSpace(*replacement), command.RequestDigest)
		if err != nil {
			return taskToolError(err)
		}
		stage, documentDigest = &prepared, prepared.Document.Digest
	}
	result, err := c.database.ReopenTask(ctx, id, revision, generation, direction, complexity, documentDigest, command, now)
	if err != nil {
		if stage != nil {
			replay, found, lookupErr := c.database.LookupTaskCommandReceipt(ctx, command)
			if lookupErr != nil {
				return taskToolError(lookupErr)
			}
			if found {
				if _, recoverErr := home.RecoverTaskDocumentStage(c.home, replay.Task.ID, command.RequestDigest, replay.DocumentDigest); recoverErr != nil {
					return taskToolError(recoverErr)
				}
				return marshalTaskResult(ctx, c.database, replay)
			}
			_ = home.DiscardTaskDocumentStage(c.home, id, command.RequestDigest)
		}
		return taskToolError(err)
	}
	if stage != nil {
		if _, err = home.CommitTaskDocumentStage(c.home, *stage); err != nil {
			return taskToolError(err)
		}
	}
	c.database.NotifyWork()
	return marshalTaskResult(ctx, c.database, result)
}

func parseTaskSchedule(raw json.RawMessage, now time.Time, defaultTimeZone string) (*schedule.Schedule, error) {
	var value struct {
		ScheduledFor    string `json:"scheduled_for"`
		TimeZone        string `json:"time_zone"`
		MissedRunPolicy string `json:"missed_run_policy"`
		Recurrence      *struct {
			StartsAt       string `json:"starts_at"`
			CronExpression string `json:"cron_expression"`
			OverlapPolicy  string `json:"overlap_policy"`
		} `json:"recurrence"`
	}
	fields, err := strictProjectFields(raw)
	if err != nil || !onlyProjectFields(fields, "scheduled_for", "time_zone", "missed_run_policy", "recurrence") ||
		decodeToolArguments(raw, &value) != nil || value.ScheduledFor == "" {
		return nil, schedule.ErrInvalid
	}
	scheduledFor, err := schedule.ParseInstant(value.ScheduledFor)
	if err != nil {
		return nil, err
	}
	if value.TimeZone == "" {
		if defaultTimeZone == "" {
			defaultTimeZone = "UTC"
		}
		value.TimeZone = defaultTimeZone
	}
	result := schedule.Schedule{ScheduledFor: scheduledFor, TimeZone: value.TimeZone, MissedRunPolicy: schedule.MissedRunOnce}
	if value.MissedRunPolicy != "" {
		result.MissedRunPolicy = schedule.MissedRunPolicy(value.MissedRunPolicy)
	}
	if value.Recurrence != nil {
		recurrenceFields, fieldsErr := strictProjectFields(fields["recurrence"])
		if fieldsErr != nil || !onlyProjectFields(recurrenceFields, "starts_at", "cron_expression", "overlap_policy") {
			return nil, schedule.ErrInvalid
		}
		startsAt, err := schedule.ParseInstant(value.Recurrence.StartsAt)
		if err != nil {
			return nil, err
		}
		overlap := schedule.OverlapSkip
		if value.Recurrence.OverlapPolicy != "" {
			overlap = schedule.OverlapPolicy(value.Recurrence.OverlapPolicy)
		}
		result.Recurrence = &schedule.Recurrence{StartsAt: startsAt,
			CronExpression: value.Recurrence.CronExpression, OverlapPolicy: overlap}
	}
	normalized, err := schedule.Normalize(result, now)
	return &normalized, err
}

func taskFence(fields map[string]json.RawMessage) (string, int64, int64, bool) {
	id, idOK := taskString(fields, "task_id", true, 255)
	revision, revisionOK := taskInt64(fields, "expected_revision")
	generation, generationOK := taskInt64(fields, "expected_generation")
	return id, revision, generation, idOK && revisionOK && generationOK
}

func taskInt64(fields map[string]json.RawMessage, key string) (int64, bool) {
	raw, exists := fields[key]
	var value int64
	if !exists || json.Unmarshal(raw, &value) != nil || value <= 0 {
		return 0, false
	}
	return value, true
}

func taskString(fields map[string]json.RawMessage, key string, required bool, limit int) (string, bool) {
	return projectString(fields, key, required, limit)
}

func taskOptionalString(fields map[string]json.RawMessage, key string, limit int) (*string, bool) {
	return projectOptionalString(fields, key, limit)
}

func taskOptionalEnum(fields map[string]json.RawMessage, key string, values ...string) (*string, bool) {
	value, ok := taskOptionalString(fields, key, 255)
	if !ok || value != nil && !taskEnum(*value, values...) {
		return nil, false
	}
	return value, true
}

func taskEnum(value string, values ...string) bool {
	for _, allowed := range values {
		if value == allowed {
			return true
		}
	}
	return false
}

func taskValue(task store.Task) map[string]any {
	return map[string]any{"task_id": task.ID, "title": task.Title, "stage_id": "stage:personal:" + task.StageKey,
		"generation": task.Generation, "revision": task.Revision, "project_id": nilString(task.ProjectID),
		"executor_agent_id": task.ExecutorAgentID, "cwd_override": task.CwdOverride,
		"scheduled_for": timeValue(task.ScheduledFor), "schedule_time_zone": nilString(task.ScheduleTimeZone),
		"missed_run_policy": nilString(task.MissedRunPolicy),
		"recurrence_id":     nilString(task.RecurrenceID), "recurrence_revision": task.RecurrenceRevision,
		"recurrence_scheduled_for": timeValue(task.RecurrenceScheduledFor)}
}

func recurrenceValue(value store.TaskRecurrence) map[string]any {
	return map[string]any{"recurrence_id": value.ID, "revision": value.Revision, "lifecycle": value.Lifecycle,
		"title": value.Title, "project_id": nilString(value.ProjectID), "starts_at": value.StartsAt.Format(time.RFC3339Nano),
		"cron_expression": value.CronExpression, "time_zone": value.TimeZone, "missed_run_policy": value.MissedRunPolicy,
		"overlap_policy": value.OverlapPolicy, "next_run_at": timeValue(value.NextRunAt)}
}

func nilString(value string) any {
	if value == "" {
		return nil
	}
	return value
}

func timeValue(value *time.Time) any {
	if value == nil {
		return nil
	}
	return value.Format(time.RFC3339Nano)
}

func marshalTaskResult(ctx context.Context, database *store.Store, result store.TaskCommandResult) (json.RawMessage, bool) {
	var project, recurrence any
	if result.Task.ProjectID != "" {
		value, err := database.Project(ctx, result.Task.ProjectID)
		if err != nil {
			return taskToolError(err)
		}
		project = projectValue(value, false)
	}
	recurrenceID := result.RecurrenceID
	if recurrenceID == "" {
		recurrenceID = result.Task.RecurrenceID
	}
	if recurrenceID != "" {
		value, err := database.TaskRecurrence(ctx, recurrenceID)
		if err != nil {
			return taskToolError(err)
		}
		recurrence = recurrenceValue(value)
	}
	runID := result.Event.RunID
	if runID == "" {
		runID = result.Task.CurrentRunID
	}
	payload, err := json.Marshal(map[string]any{"task": taskValue(result.Task), "project": project,
		"recurrence_authority": recurrence, "gate_id": nilString(result.GateID), "run_id": nilString(runID),
		"event_id": result.Event.EventID, "event_sequence": result.Event.ID,
		"recurrence_id": nilString(result.RecurrenceID), "replayed": result.Replayed})
	if err != nil {
		return taskToolError(err)
	}
	return boundedModelToolPayload(payload, modelToolResultLimit), true
}

func taskToolInputFailure() (json.RawMessage, bool) {
	return toolFailure("invalid_input", "Task tool arguments are invalid"), false
}

func taskToolError(err error) (json.RawMessage, bool) {
	code, message := "work_unavailable", "Task authority is unavailable"
	switch {
	case errors.Is(err, store.ErrStaleRevision):
		code, message = "stale_revision", "Task revision is stale"
	case errors.Is(err, store.ErrInvalidTransition):
		code, message = "invalid_transition", "Task action is unavailable"
	case errors.Is(err, store.ErrCommandConflict):
		code, message = "command_conflict", "Task command key conflicts with an earlier request"
	case errors.Is(err, store.ErrInvalidCursor), errors.Is(err, schedule.ErrInvalid):
		code, message = "invalid_input", "Task input is invalid"
	}
	return toolFailure(code, message), false
}
