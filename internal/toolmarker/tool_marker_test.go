package toolmarker

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestRustMarkerRuleTablePreservesEveryBuiltInLabel(t *testing.T) {
	want := map[string][4]string{
		"task.files.read":             {"Read Task file", "Reading", "Read", "Could not read"},
		"task.files.write":            {"Write Task file", "Writing", "Wrote", "Could not write"},
		"task.files.delete":           {"Delete Task file", "Deleting", "Deleted", "Could not delete"},
		"task.capture":                {"Create Task", "Creating Task", "Created Task", "Could not create Task"},
		"task.update":                 {"Update Task", "Updating Task", "Updated Task", "Could not update Task"},
		"task.queue":                  {"Queue Task", "Queueing Task", "Queued Task", "Could not queue Task"},
		"task.schedule":               {"Schedule Task", "Scheduling Task", "Scheduled Task", "Could not schedule Task"},
		"task.reschedule":             {"Reschedule Task", "Rescheduling Task", "Rescheduled Task", "Could not reschedule Task"},
		"task.unschedule":             {"Remove Task schedule", "Removing Task schedule", "Removed Task schedule", "Could not remove Task schedule"},
		"task.schedule.run_now":       {"Start scheduled Task", "Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task"},
		"task.recurrence.run_now":     {"Start scheduled Task", "Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task"},
		"task.recurrence.update":      {"Update Task schedule", "Updating Task schedule", "Updated Task schedule", "Could not update Task schedule"},
		"task.recurrence.pause":       {"Pause Task schedule", "Pausing Task schedule", "Paused Task schedule", "Could not pause Task schedule"},
		"task.recurrence.resume":      {"Resume Task schedule", "Resuming Task schedule", "Resumed Task schedule", "Could not resume Task schedule"},
		"task.recurrence.skip_next":   {"Skip next Task run", "Skipping next Task run", "Skipped next Task run", "Could not skip next Task run"},
		"task.recurrence.end":         {"End Task schedule", "Ending Task schedule", "Ended Task schedule", "Could not end Task schedule"},
		"task.answer":                 {"Answer Task question", "Answering Task question", "Answered Task question", "Could not answer Task question"},
		"task.retry":                  {"Retry Task", "Retrying Task", "Retried Task", "Could not retry Task"},
		"task.cancel":                 {"Cancel Task", "Cancelling Task", "Cancelled Task", "Could not cancel Task"},
		"task.reopen":                 {"Reopen Task", "Reopening Task", "Reopened Task", "Could not reopen Task"},
		"task.read_artifact":          {"Read Task artifact", "Reading artifact", "Read artifact", "Could not read artifact"},
		"project.create":              {"Create project", "Creating project", "Created project", "Could not create project"},
		"project.update":              {"Update project", "Updating project", "Updated project", "Could not update project"},
		"project.archive":             {"Archive project", "Archiving project", "Archived project", "Could not archive project"},
		"project.reopen":              {"Reopen project", "Reopening project", "Reopened project", "Could not reopen project"},
		"read_memory_page":            {"Read memory page", "Reading memory page", "Read memory page", "Could not read memory page"},
		"artifact.create_local_file":  {"Create local file", "Creating", "Created", "Could not create"},
		"adapter.definition_template": {"Definition template", "Loading definition template", "Loaded definition template", "Could not load definition template"},
		"adapter.propose_definition":  {"Propose definition", "Proposing definition", "Proposed definition", "Could not propose definition"},
		"mcp.connect_service":         {"Connect service", "Connecting", "Connected", "Could not connect"},
	}
	if len(rules) != len(want) {
		t.Fatalf("marker rule count = %d, want %d", len(rules), len(want))
	}
	for _, rule := range rules {
		t.Run(rule.name, func(t *testing.T) {
			expected, ok := want[rule.name]
			if !ok || rule.identity != expected[0] || rule.running != expected[1] || rule.completed != expected[2] || rule.failed != expected[3] {
				t.Fatalf("rule = %#v, want labels %#v", rule, expected)
			}
			arguments := map[string]any{}
			switch rule.subject {
			case subjectPath:
				arguments["path"] = "TASK.md"
			case subjectTask:
				arguments["title"] = "Task title"
			case subjectProject:
				arguments["title"] = "Project title"
			case subjectArtifact:
				arguments["filename"] = "result.txt"
			case subjectAdapter:
				arguments["source_reference"] = "github"
			case subjectService:
				arguments["service_url"] = "https://service.example"
			}
			running, ok := Build(rule.name, "running", false, arguments, nil)
			if !ok || running["identity"] != rule.identity || !strings.HasPrefix(running["summary"].(string), rule.running) {
				t.Fatalf("running marker = %#v", running)
			}
			completed, ok := Build(rule.name, "completed", true, arguments, map[string]any{})
			if !ok || completed["identity"] != rule.identity || !strings.HasPrefix(completed["summary"].(string), rule.completed) {
				t.Fatalf("completed marker = %#v", completed)
			}
			failed, ok := Build(rule.name, "failed", true, arguments, map[string]any{"error": "failed"})
			if !ok || failed["identity"] != rule.identity || !strings.HasPrefix(failed["summary"].(string), rule.failed) {
				t.Fatalf("failed marker = %#v", failed)
			}
		})
	}
}

func TestRustMarkerSpecialCasesPreserveQueriesCountsAndLifecycle(t *testing.T) {
	marker, ok := Build("web.search", "cancelled", true,
		map[string]any{"query": "Noema tools"}, map[string]any{"query": "Noema tools"})
	if !ok || marker["kind"] != "web.search" || marker["status"] != "cancelled" ||
		marker["summary"] != "Cancelled: searching the web for “Noema tools”" {
		t.Fatalf("cancelled web marker = %#v", marker)
	}
	marker, ok = Build("web.search", "running", false, nil, nil)
	if !ok || marker["summary"] != "Searching the web" {
		t.Fatalf("empty web marker = %#v", marker)
	}
	marker, ok = Build("task.files.list", "completed", true,
		map[string]any{"path": "."}, map[string]any{"entries": []any{map[string]any{"name": "one"}, map[string]any{"name": "two"}}})
	if !ok || marker["summary"] != "Listed Task files · 2 items" {
		t.Fatalf("file list marker = %#v", marker)
	}
	marker, ok = Build("task.finish_execution", "completed", true, nil, nil)
	if !ok || marker["visibility"] != "fold" {
		t.Fatalf("folded marker = %#v", marker)
	}
}

func TestRustMarkerBrowserPrivacyAndHiddenActions(t *testing.T) {
	if marker, ok := Build("gmail.search", "completed", true, nil, nil); ok || marker != nil {
		t.Fatalf("connected tool marker = %#v, %v", marker, ok)
	}
	marker, ok := Build("web.browse.interact", "completed", true,
		map[string]any{"action": "fill", "value": "private text"}, map[string]any{"title": "Sign in"})
	if !ok || marker["summary"] != "Filled page field" || strings.Contains(stringValue(marker), "private text") {
		t.Fatalf("browser marker = %#v", marker)
	}
	marker, ok = Build("web.browse.open", "failed", false,
		map[string]any{"url": "https://www.example.com/docs"}, nil)
	if !ok || marker["summary"] != "Could not open example.com" {
		t.Fatalf("browser open marker = %#v", marker)
	}
	marker, ok = Build("web.browse.open", "completed", true, nil,
		map[string]any{"snapshot": map[string]any{"url": "https://example.com/lifecycle"}})
	if !ok || marker["summary"] != "Opened example.com" || marker["kind"] != "web.browse" || marker["host"] != "example.com" {
		t.Fatalf("browser result marker = %#v", marker)
	}
	if marker, ok := Build("web.browse.open", "failed", true, nil, map[string]any{"error": "connection failed"}); ok || marker != nil {
		t.Fatalf("missing browser target marker = %#v, %v", marker, ok)
	}
	if marker, ok := Build("task.delegate", "completed", true, nil, nil); ok || marker != nil {
		t.Fatalf("delegation marker = %#v, %v", marker, ok)
	}
	if marker, ok := Build("web.browse.close", "completed", true, nil, nil); ok || marker != nil {
		t.Fatalf("browser close marker = %#v, %v", marker, ok)
	}
}

func TestRustMarkerSubjectsAndActionEnvelope(t *testing.T) {
	marker, ok := ForAction("tool_call", "started", map[string]any{
		"name":    "task.capture",
		"payload": map[string]any{"arguments": map[string]any{"title": "New task"}},
	})
	if !ok || marker["status"] != "running" || marker["subjectLabel"] != "Task" || marker["subject"] != "New task" {
		t.Fatalf("nested action marker = %#v", marker)
	}
	marker, ok = Build("task.capture", "completed", true, nil,
		map[string]any{"task": map[string]any{"title": "New task"}, "run_id": "run:one"})
	if !ok || marker["detail"] != "A Task run started in the background." || marker["subject"] != "New task" {
		t.Fatalf("task result marker = %#v", marker)
	}
	marker, ok = Build("mcp.connect_service", "completed", true,
		map[string]any{"service_url": "https://docs.example/mcp"}, nil)
	if !ok || marker["subjectLabel"] != "Service" || marker["subject"] != "docs.example" {
		t.Fatalf("service marker = %#v", marker)
	}
}

func stringValue(value any) string {
	encoded, _ := json.Marshal(value)
	return string(encoded)
}
