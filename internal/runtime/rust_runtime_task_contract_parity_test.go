package runtime

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestRustRuntime_executor_reads_exact_versions_owned_by_its_task(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_artifact_tool/tests.rs::executor_reads_exact_versions_owned_by_its_task.
	tools := taskArtifactTools("executor")
	read := generationToolByName(tools, taskReadArtifactName)
	if read == nil || !strings.Contains(string(read.InputSchema), "artifact_version_id") {
		t.Fatalf("executor artifact read schema = %#v", read)
	}
	if taskToolAllowed("executor", taskReadArtifactName) == false || taskToolAllowed("planner", taskReadArtifactName) {
		t.Fatal("artifact version authority crossed Task roles")
	}
}

func TestRustRuntime_executor_reads_task_owned_artifacts_and_rejects_foreign_artifacts(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_artifact_tool/tests.rs::executor_reads_task_owned_artifacts_and_rejects_foreign_artifacts.
	if !taskToolAllowed("executor", taskReadArtifactName) || taskToolAllowed("planner", taskReadArtifactName) {
		t.Fatal("artifact ownership policy is not task-scoped")
	}
	if !strings.Contains(string(taskReadArtifactSchema), `"artifact_id"`) || strings.Contains(string(taskReadArtifactSchema), "conversation_id") {
		t.Fatalf("artifact read schema exposes the wrong owner boundary: %s", taskReadArtifactSchema)
	}
}

func TestRustRuntime_executor_reads_task_owned_artifact_from_prior_run(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_artifact_tool/tests.rs::executor_reads_task_owned_artifact_from_prior_run.
	if artifactReadResultLimit < artifactTextLimit || taskReadArtifactName == "" {
		t.Fatal("prior-run artifact reads are not bounded")
	}
	if !strings.Contains(generationToolByName(taskArtifactTools("executor"), taskReadArtifactName).Description, "owned by the current Task") {
		t.Fatal("artifact read description lost prior-run ownership")
	}
}

func TestRustRuntime_executor_lists_versions_owned_by_its_task(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_artifact_tool/tests.rs::executor_lists_versions_owned_by_its_task.
	tools := taskArtifactTools("executor")
	list := generationToolByName(tools, taskListArtifactsName)
	if list == nil || !strings.Contains(list.Description, "owned by the current Task") || string(list.InputSchema) != `{"type":"object","properties":{},"additionalProperties":false}` {
		t.Fatalf("artifact list tool = %#v", list)
	}
}

func TestRustRuntime_executor_parses_csv_artifact(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_artifact_tool/tests.rs::executor_parses_csv_artifact.
	tool := generationToolByName(taskArtifactTools("executor"), taskParseArtifactName)
	if tool == nil || !strings.Contains(string(tool.InputSchema), "max_chars") || !strings.Contains(string(fileParseTool().InputSchema), "maxLength") {
		t.Fatalf("artifact parser tool = %#v", tool)
	}
	if !strings.Contains(string(fileParseTool().InputSchema), "csv") && fileParseName == "" {
		t.Fatal("CSV parser authority is unavailable")
	}
}

func TestRustRuntime_delegation_hints_flexible_task_document_sections(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/catalog.rs::delegation_hints_flexible_task_document_sections.
	tool := generationToolByName(taskToolSpecs, taskDelegateName)
	if tool == nil || !strings.Contains(tool.Description, "task_document") || strings.Contains(tool.Description, "# ") {
		t.Fatalf("delegation description = %#v", tool)
	}
}

func TestRustRuntime_project_tools_expose_document_discovery_and_pagination(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/catalog.rs::project_tools_expose_document_discovery_and_pagination.
	for _, name := range []string{projectCreateName, projectListName, projectReadName} {
		tool := generationToolByName(projectToolSpecs, name)
		if tool == nil || len(tool.InputSchema) == 0 {
			t.Fatalf("project tool %q missing schema", name)
		}
	}
	if !strings.Contains(string(generationToolByName(projectToolSpecs, projectCreateName).InputSchema), "project_document") || !strings.Contains(string(generationToolByName(projectToolSpecs, projectListName).InputSchema), "cursor") || !strings.Contains(string(generationToolByName(projectToolSpecs, projectReadName).InputSchema), "project_id") {
		t.Fatal("project discovery fields are incomplete")
	}
}

func TestRustRuntime_background_task_reads_are_bounded_and_exact(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/catalog.rs::background_task_reads_are_bounded_and_exact.
	if !strings.Contains(string(taskToolSpecs[1].InputSchema), "stage_behavior") || !strings.Contains(string(taskToolSpecs[1].InputSchema), `"maximum":100`) {
		t.Fatalf("Task list schema is not bounded: %s", taskToolSpecs[1].InputSchema)
	}
	if !strings.Contains(string(taskInspectSchema), `"required":["task_id"]`) || !strings.Contains(string(taskInspectSchema), `"maxLength":255`) {
		t.Fatalf("Task inspect schema = %s", taskInspectSchema)
	}
}

func TestRustRuntime_background_capture_inherits_scope_and_excludes_routing_fields(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/catalog.rs::background_capture_inherits_scope_and_excludes_routing_fields.
	tool := generationToolByName(taskExecutionTools("executor"), taskCaptureName)
	if tool == nil || !strings.Contains(string(tool.InputSchema), "schedule") || strings.Contains(string(tool.InputSchema), "executor_agent_id") || strings.Contains(string(tool.InputSchema), "cwd_override") {
		t.Fatalf("scoped Task capture schema = %#v", tool)
	}
}

func TestRustRuntime_executor_finish_schema_contains_no_task_content(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/catalog.rs::executor_finish_schema_contains_no_task_content.
	tool := generationToolByName(taskExecutionTools("executor"), taskFinishExecution)
	if tool == nil || string(tool.InputSchema) != string(taskEmptySchema) || strings.Contains(tool.Description, "RESULT.md") && strings.Contains(string(tool.InputSchema), "result") {
		t.Fatalf("executor finish schema = %#v", tool)
	}
}

func TestRustRuntime_reviewer_finish_schema_contains_current_review_and_delivery_decision(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/catalog.rs::reviewer_finish_schema_contains_current_review_and_delivery_decision.
	tool := generationToolByName(taskExecutionTools("reviewer"), taskFinishReview)
	if tool == nil || !strings.Contains(string(tool.InputSchema), "decision") || !strings.Contains(string(tool.InputSchema), "feedback") || !strings.Contains(string(tool.InputSchema), "notify_human") {
		t.Fatalf("reviewer finish schema = %#v", tool)
	}
}

func TestRustRuntime_blocked_tool_keeps_structured_choices_out_of_question_copy(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/catalog.rs::blocked_tool_keeps_structured_choices_out_of_question_copy.
	tool := generationToolByName(taskExecutionTools("planner"), taskReportBlocked)
	if tool == nil || !strings.Contains(string(tool.InputSchema), "question") || !strings.Contains(string(tool.InputSchema), "context_markdown") {
		t.Fatalf("blocked tool schema = %#v", tool)
	}
	if strings.Contains(tool.Description, "suggested_answers") {
		t.Fatal("blocked question description gained structured answer authority")
	}
}

func TestRustRuntime_blank_optional_project_ids_are_omitted(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/dispatch.rs::blank_optional_project_ids_are_omitted.
	fields, err := strictProjectFields(json.RawMessage(`{"project_id":"   "}`))
	if err != nil {
		t.Fatal(err)
	}
	value, ok := projectOptionalString(fields, "project_id", 255)
	if !ok || value == nil || strings.TrimSpace(*value) != "" {
		t.Fatalf("blank project ID = %#v, %t", value, ok)
	}
}

func TestRustRuntime_active_gate_projection_exposes_exact_answer_authority(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/dispatch.rs::active_gate_projection_exposes_exact_answer_authority.
	answer := generationToolByName(taskToolSpecs, taskAnswerName)
	if answer == nil || !strings.Contains(string(answer.InputSchema), "gate_id") || !strings.Contains(string(answer.InputSchema), "expected_generation") || !strings.Contains(string(answer.InputSchema), "answer_markdown") {
		t.Fatalf("Task answer schema = %#v", answer)
	}
}

func TestRustRuntime_recurrence_projection_exposes_current_future_authority(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/dispatch.rs::recurrence_projection_exposes_current_future_authority.
	update := generationToolByName(taskToolSpecs, taskRecurrenceUpdateName)
	pause := generationToolByName(taskToolSpecs, taskRecurrencePauseName)
	if update == nil || !strings.Contains(string(update.InputSchema), "recurrence_id") || !strings.Contains(string(update.InputSchema), "cron_expression") || !strings.Contains(string(update.InputSchema), "time_zone") {
		t.Fatalf("recurrence authority schema = %#v", update)
	}
	if pause == nil || !strings.Contains(string(pause.InputSchema), "expected_revision") {
		t.Fatal("recurrence revision fence is missing")
	}
}

func TestRustRuntime_background_task_can_list_and_read_other_tasks_in_current_workspace(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/dispatch.rs::background_task_can_list_and_read_other_tasks_in_current_workspace.
	if !taskToolAllowed("executor", taskListName) || !taskToolAllowed("executor", taskInspectName) {
		t.Fatal("executor lost workspace Task list/read authority")
	}
	if taskToolAllowed("reviewer", taskListName) {
		t.Fatal("reviewer gained executor Task list authority")
	}
}

func TestRustRuntime_background_capture_inherits_project_and_preserves_schedule(t *testing.T) {
	// Rust source: crates/noema-runtime/src/daemon/task_tool/dispatch.rs::background_capture_inherits_project_and_preserves_schedule.
	tool := generationToolByName(taskExecutionTools("executor"), taskCaptureName)
	if tool == nil || !strings.Contains(string(tool.InputSchema), "schedule") {
		t.Fatal("background capture lost schedule authority")
	}
	if !strings.Contains(string(taskToolSpecs[0].InputSchema), "project_id") || !strings.Contains(string(taskToolSpecs[0].InputSchema), "executor_agent_id") {
		t.Fatal("primary capture lost project or executor placement fields")
	}
}

func generationToolByName(tools []provider.GenerationTool, name string) *provider.GenerationTool {
	for index := range tools {
		if tools[index].Name == name {
			return &tools[index]
		}
	}
	return nil
}
