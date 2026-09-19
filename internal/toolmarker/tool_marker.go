// Package toolmarker builds user-facing tool markers.
// It mirrors the Rust runtime marker contract so every transcript surface uses
// the same identity, summary, subject, and lifecycle text.
package toolmarker

import (
	"encoding/json"
	"net/url"
	"reflect"
	"strconv"
	"strings"
	"unicode"
)

// ForAction builds a marker from the saved action envelope used by transcript
// items. It returns false for unlabeled connected tools and hidden actions.
func ForAction(actionKind, status string, action map[string]any) (map[string]any, bool) {
	name, _ := action["name"].(string)
	name = strings.TrimSpace(name)
	if name == "" {
		return nil, false
	}
	isResult := strings.EqualFold(actionKind, "tool_result")
	if label, _ := action["display_label"].(string); strings.TrimSpace(label) != "" && strings.HasPrefix(name, "mcp.") && name != "mcp.connect_service" {
		// The paired result supplies status and outcome text.
		return map[string]any{"identity": ReadableName(name), "summary": label}, true
	}
	payload := action["payload"]
	arguments, hasArguments := action["arguments"]
	if !hasArguments && !isResult {
		arguments = payload
	}
	if object, ok := arguments.(map[string]any); ok && len(object) == 1 {
		if nestedArguments, exists := object["arguments"]; exists {
			arguments = nestedArguments
		}
	}
	var result any
	if isResult {
		result = payload
	}
	return Build(name, status, isResult, arguments, result)
}

// Build builds one Rust-shaped marker from built-in tool facts.
func Build(name, status string, isResult bool, arguments, result any) (map[string]any, bool) {
	name = strings.TrimSpace(name)
	if IsHidden(name) || !isBuiltin(name) {
		return nil, false
	}
	markerStatus := parseStatus(status, isResult)
	if name == "web.browse.open" && isResult && markerStatus == statusFailed && browserPage(arguments, result) == "" {
		return nil, false
	}
	identity, summary := markerText(name, markerStatus, arguments, result, firstText(result,
		[]string{"error"}, []string{"message"}, []string{"details", "message"}))
	marker := map[string]any{
		"identity":    identity,
		"summary":     summary,
		"detailTitle": detailTitle(identity, markerStatus),
		"status":      markerStatus.String(),
	}
	if label, value := markerSubject(name, arguments, result); value != "" {
		marker["subjectLabel"] = label
		marker["subject"] = value
	}
	if strings.HasPrefix(name, "task.") && markerStatus == statusCompleted && nonNilField(result, "run_id") {
		marker["detail"] = "A Task run started in the background."
	}
	if name == "web.search" || name == "web.fetch" {
		marker["kind"] = name
	} else if strings.HasPrefix(name, "web.browse.") {
		marker["kind"] = "web.browse"
		if host := browserPage(arguments, result); host != "" {
			marker["host"] = host
		}
	}
	if isFolded(name) {
		marker["visibility"] = "fold"
	}
	return marker, true
}

// IsHidden reports whether a built-in action must stay out of user-facing
// marker output.
func IsHidden(name string) bool {
	return name == "task.delegate" || name == "web.browse.close"
}

type markerStatus string

const (
	statusPending     markerStatus = "pending"
	statusRunning     markerStatus = "running"
	statusCompleted   markerStatus = "completed"
	statusFailed      markerStatus = "failed"
	statusCancelled   markerStatus = "cancelled"
	statusInterrupted markerStatus = "interrupted"
	statusSkipped     markerStatus = "skipped"
)

func (s markerStatus) String() string { return string(s) }

func parseStatus(value string, isResult bool) markerStatus {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "pending", "queued":
		return statusPending
	case "running", "started", "active":
		return statusRunning
	case "failed", "error":
		return statusFailed
	case "cancelled":
		return statusCancelled
	case "interrupted":
		return statusInterrupted
	case "skipped":
		return statusSkipped
	case "completed", "complete":
		if isResult {
			return statusCompleted
		}
	}
	if isResult {
		return statusCompleted
	}
	return statusPending
}

type markerRule struct {
	name, identity, running, completed, failed string
	subject                                    subjectKind
}

type subjectKind uint8

const (
	subjectNone subjectKind = iota
	subjectPath
	subjectTask
	subjectProject
	subjectArtifact
	subjectAdapter
	subjectService
)

var rules = []markerRule{
	{"task.files.read", "Read Task file", "Reading", "Read", "Could not read", subjectPath},
	{"task.files.write", "Write Task file", "Writing", "Wrote", "Could not write", subjectPath},
	{"task.files.delete", "Delete Task file", "Deleting", "Deleted", "Could not delete", subjectPath},
	{"task.capture", "Create Task", "Creating Task", "Created Task", "Could not create Task", subjectTask},
	{"task.update", "Update Task", "Updating Task", "Updated Task", "Could not update Task", subjectTask},
	{"task.queue", "Queue Task", "Queueing Task", "Queued Task", "Could not queue Task", subjectTask},
	{"task.schedule", "Schedule Task", "Scheduling Task", "Scheduled Task", "Could not schedule Task", subjectTask},
	{"task.reschedule", "Reschedule Task", "Rescheduling Task", "Rescheduled Task", "Could not reschedule Task", subjectTask},
	{"task.unschedule", "Remove Task schedule", "Removing Task schedule", "Removed Task schedule", "Could not remove Task schedule", subjectTask},
	{"task.schedule.run_now", "Start scheduled Task", "Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task", subjectTask},
	{"task.recurrence.run_now", "Start scheduled Task", "Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task", subjectTask},
	{"task.recurrence.update", "Update Task schedule", "Updating Task schedule", "Updated Task schedule", "Could not update Task schedule", subjectTask},
	{"task.recurrence.pause", "Pause Task schedule", "Pausing Task schedule", "Paused Task schedule", "Could not pause Task schedule", subjectTask},
	{"task.recurrence.resume", "Resume Task schedule", "Resuming Task schedule", "Resumed Task schedule", "Could not resume Task schedule", subjectTask},
	{"task.recurrence.skip_next", "Skip next Task run", "Skipping next Task run", "Skipped next Task run", "Could not skip next Task run", subjectTask},
	{"task.recurrence.end", "End Task schedule", "Ending Task schedule", "Ended Task schedule", "Could not end Task schedule", subjectTask},
	{"task.answer", "Answer Task question", "Answering Task question", "Answered Task question", "Could not answer Task question", subjectTask},
	{"task.retry", "Retry Task", "Retrying Task", "Retried Task", "Could not retry Task", subjectTask},
	{"task.cancel", "Cancel Task", "Cancelling Task", "Cancelled Task", "Could not cancel Task", subjectTask},
	{"task.reopen", "Reopen Task", "Reopening Task", "Reopened Task", "Could not reopen Task", subjectTask},
	{"task.read_artifact", "Read Task artifact", "Reading artifact", "Read artifact", "Could not read artifact", subjectArtifact},
	{"project.create", "Create project", "Creating project", "Created project", "Could not create project", subjectProject},
	{"project.update", "Update project", "Updating project", "Updated project", "Could not update project", subjectProject},
	{"project.archive", "Archive project", "Archiving project", "Archived project", "Could not archive project", subjectProject},
	{"project.reopen", "Reopen project", "Reopening project", "Reopened project", "Could not reopen project", subjectProject},
	{"read_memory_page", "Read memory page", "Reading memory page", "Read memory page", "Could not read memory page", subjectPath},
	{"artifact.create_local_file", "Create local file", "Creating", "Created", "Could not create", subjectArtifact},
	{"adapter.definition_template", "Definition template", "Loading definition template", "Loaded definition template", "Could not load definition template", subjectNone},
	{"adapter.propose_definition", "Propose definition", "Proposing definition", "Proposed definition", "Could not propose definition", subjectAdapter},
	{"mcp.connect_service", "Connect service", "Connecting", "Connected", "Could not connect", subjectService},
}

func markerText(name string, status markerStatus, arguments, result any, errorText string) (string, string) {
	if identity, summary, ok := specialMarker(name, status, arguments, result, errorText); ok {
		return identity, summary
	}
	for _, rule := range rules {
		if rule.name != name {
			continue
		}
		target := subject(rule.subject, arguments, result)
		return named(rule.identity, status, copyText{rule.running, rule.completed, rule.failed}.target(target), errorText)
	}
	identity := readableIdentity(name)
	return identity, identity
}

type copyText struct{ running, completed, failed string }

func (c copyText) target(target string) copyText {
	return copyText{appendText(c.running, target), appendText(c.completed, target), appendText(c.failed, target)}
}

func named(identity string, status markerStatus, copy copyText, errorText string) (string, string) {
	summary := copy.running
	switch status {
	case statusFailed:
		summary = outcome(copy.failed, errorText)
	case statusCancelled:
		summary = "Cancelled: " + lowerInitial(copy.running)
	case statusInterrupted:
		summary = "Interrupted: " + lowerInitial(copy.running)
	case statusSkipped:
		summary = "Skipped: " + lowerInitial(copy.running)
	case statusCompleted:
		summary = copy.completed
	}
	return identity, summary
}

func specialMarker(name string, status markerStatus, arguments, result any, errorText string) (string, string, bool) {
	switch name {
	case "web.search":
		query := firstText(arguments, []string{"query"})
		if query == "" {
			query = firstText(result, []string{"query"})
		}
		copy := copyText{"Searching the web", "Web search", "Could not search the web"}
		if query != "" {
			target := "“" + query + "”"
			copy = copyText{"Searching the web for " + target, query, "Could not search the web for " + target}
		}
		identity, summary := named("Web Search", status, copy, errorText)
		return identity, summary, true
	case "web.fetch":
		identity, summary := named("Fetched Web Page", status, copyText{"Reading", "Read", "Could not read"}.target(webTarget(arguments, result)), errorText)
		return identity, summary, true
	case "web.browse":
		identity, summary := browserMarker(name, status, arguments, result, errorText)
		return identity, summary, true
	case "task.files.list":
		path := firstText(arguments, []string{"path"})
		if path == "" {
			path = "."
		}
		scope := ""
		if path != "." {
			scope = " in " + path
		}
		completed := outcome("Listed Task files"+scope, arrayQuantity(result, []string{"entries"}, "item"))
		identity, summary := named("List Task files", status, copyText{"Listing Task files" + scope, completed, "Could not list Task files" + scope}, errorText)
		return identity, summary, true
	case "task.list":
		current := isEmptyObject(arguments)
		copy := copyText{}
		if current {
			copy = copyText{"Checking current Task", "Checked current Task", "Could not check current Task"}
		} else {
			copy = copyText{"Listing Tasks", outcome("Listed Tasks", arrayQuantity(result, []string{"tasks"}, "Task")), "Could not list Tasks"}
		}
		identity, summary := named("List Tasks", status, copy, errorText)
		return identity, summary, true
	case "task.finish_planning":
		complexity := firstText(result, []string{"complexity"})
		if complexity == "" {
			complexity = firstText(arguments, []string{"complexity"})
		}
		identity, summary := named("Finish planning", status, copyText{"Finishing plan", outcome("Plan ready", complexity), "Could not finish plan"}, errorText)
		return identity, summary, true
	case "task.finish_review":
		decision := firstText(result, []string{"decision"})
		if decision == "" {
			decision = firstText(arguments, []string{"decision"})
		}
		identity, summary := named("Finish review", status, copyText{"Finishing review", reviewDecision(decision), "Could not finish review"}, errorText)
		return identity, summary, true
	case "task.report_blocked":
		question := firstText(result, []string{"question"})
		if question == "" {
			question = firstText(arguments, []string{"question"})
		}
		identity, summary := named("Report Task blocked", status, copyText{"Reporting Task blocked", appendText("Task blocked:", question), "Could not report Task blocked"}, errorText)
		return identity, summary, true
	case "project.list":
		identity, summary := named("List projects", status, copyText{"Listing projects", outcome("Listed projects", arrayQuantity(result, []string{"projects"}, "project")), "Could not list projects"}, errorText)
		return identity, summary, true
	case "search_memory":
		query := firstText(arguments, []string{"query"})
		target := "memory"
		if query != "" {
			target = "“" + query + "”"
		}
		identity, summary := named("Search memory", status, copyText{"Searching memory for " + target, outcome("Searched memory for "+target, arrayQuantity(result, []string{"pages"}, "page")), "Could not search memory for " + target}, errorText)
		return identity, summary, true
	case "file.parse":
		path := firstText(arguments, []string{"path"})
		if path == "" {
			path = firstText(result, []string{"path"})
		}
		chars := quantityValue(result, []string{"returned_chars"}, "character")
		identity, summary := named("Parse file", status, copyText{appendText("Parsing", path), outcome(appendText("Parsed", path), chars), appendText("Could not parse", path)}, errorText)
		return identity, summary, true
	case "file.download":
		path := firstText(arguments, []string{"path"})
		if path == "" {
			path = firstText(result, []string{"path"})
		}
		host := firstText(arguments, []string{"url"})
		if host == "" {
			host = firstText(result, []string{"url"})
			if host == "" {
				host = firstText(result, []string{"final_url"})
			}
		}
		host = webHost(host)
		target := path
		if target != "" && host != "" {
			target += " from " + host
		} else if target == "" {
			target = host
		}
		identity, summary := named("Download file", status, copyText{"Downloading", "Downloaded", "Could not download"}.target(target), errorText)
		return identity, summary, true
	case "update_own_name":
		target := firstText(result, []string{"display_name"})
		if target == "" {
			target = firstText(arguments, []string{"name"})
		}
		identity, summary := named("Save name", status, copyText{appendText("Saving name as", target), appendText("Saved name as", target), "Could not save name"}, errorText)
		return identity, summary, true
	}
	if strings.HasPrefix(name, "web.browse.") {
		identity, summary := browserMarker(name, status, arguments, result, errorText)
		return identity, summary, true
	}
	if isFolded(name) {
		identity := readableIdentity(name)
		return identity, identity, true
	}
	return "", "", false
}

func browserMarker(name string, status markerStatus, arguments, result any, errorText string) (string, string) {
	page := browserPage(arguments, result)
	copy := copyText{}
	switch name {
	case "web.browse.open":
		copy = copyText{"Opening", "Opened", "Could not open"}.target(page)
	case "web.browse.navigate":
		copy = copyText{"Navigating to", "Navigated to", "Could not navigate to"}.target(page)
	case "web.browse.snapshot":
		copy = copyText{"Capturing page", "Captured", "Could not capture"}.target(page)
	case "web.browse.wait":
		target := firstText(arguments, []string{"text"})
		if target == "" {
			target = "page"
		}
		copy = copyText{"Waiting for", "Waited for", "Could not wait for"}.target(target)
	case "web.browse.history":
		copy = copyText{"Checking browser history", "Checked browser history", "Could not check browser history"}
	case "web.browse.switch_provider":
		copy = copyText{"Switching browser provider", "Switched browser provider", "Could not switch browser provider"}.target(page)
	case "web.browse.close":
		copy = copyText{"Closing browser", "Closed browser", "Could not close browser"}
	case "web.browse.interact":
		switch firstText(arguments, []string{"action"}) {
		case "upload_file":
			copy = copyText{"Uploading file", "Uploaded file", "Could not upload file"}
		case "click":
			copy = copyText{"Clicking page element", "Clicked page element", "Could not click page element"}
		case "fill", "type":
			copy = copyText{"Filling page field", "Filled page field", "Could not fill page field"}
		case "press":
			copy = copyText{"Pressing key", "Pressed key", "Could not press key"}
		case "select":
			copy = copyText{"Selecting page option", "Selected page option", "Could not select page option"}
		default:
			copy = copyText{"Using page control", "Used page control", "Could not use page control"}
		}
	default:
		copy = copyText{"Using browser", "Used browser", "Browser action failed"}
	}
	last := name
	if index := strings.LastIndexByte(last, '.'); index >= 0 {
		last = last[index+1:]
	}
	last = strings.ReplaceAll(last, "_", " ")
	return named("Browser "+last, status, copy, errorText)
}

func markerSubject(name string, arguments, result any) (string, string) {
	switch {
	case name == "web.search":
		value := firstText(arguments, []string{"query"})
		if value == "" {
			value = firstText(result, []string{"query"})
		}
		return "Search", value
	case name == "web.fetch":
		return "Page", webTarget(arguments, result)
	case strings.HasPrefix(name, "task.files."), strings.HasPrefix(name, "file."):
		return "File", subject(subjectPath, arguments, result)
	case strings.HasPrefix(name, "task."):
		return "Task", subject(subjectTask, arguments, result)
	case strings.HasPrefix(name, "project."):
		return "Project", subject(subjectProject, arguments, result)
	case strings.HasPrefix(name, "artifact."):
		return "File", subject(subjectArtifact, arguments, result)
	case strings.HasPrefix(name, "adapter."), name == "mcp.connect_service":
		if strings.HasPrefix(name, "adapter.") {
			return "Service", subject(subjectAdapter, arguments, result)
		}
		return "Service", subject(subjectService, arguments, result)
	case name == "search_memory":
		return "Search", firstText(arguments, []string{"query"})
	case name == "read_memory_page":
		return "Memory", subject(subjectPath, arguments, result)
	case name == "update_own_name":
		value := firstText(result, []string{"display_name"})
		if value == "" {
			value = firstText(arguments, []string{"name"})
		}
		return "Name", value
	}
	return "", ""
}

func subject(kind subjectKind, arguments, result any) string {
	switch kind {
	case subjectPath:
		value := firstText(arguments, []string{"path"}, []string{"page"})
		if value == "" {
			value = firstText(result, []string{"path"}, []string{"page_ref"})
		}
		return value
	case subjectTask:
		value := firstText(result, []string{"task", "title"})
		if value == "" {
			value = firstText(arguments, []string{"title"})
		}
		return value
	case subjectProject:
		value := firstText(result, []string{"project", "title"}, []string{"project", "name"})
		if value == "" {
			value = firstText(arguments, []string{"title"}, []string{"name"})
		}
		return value
	case subjectArtifact:
		value := firstText(result, []string{"title"})
		if value == "" {
			value = firstText(arguments, []string{"filename"}, []string{"title"})
		}
		return value
	case subjectAdapter:
		value := firstText(result, []string{"display_name"})
		if value == "" {
			value = firstText(arguments, []string{"source_reference"})
		}
		return value
	case subjectService:
		value := firstText(result, []string{"display_name"})
		if value == "" {
			value = webHost(firstText(arguments, []string{"service_url"}))
		}
		return value
	}
	return ""
}

func detailTitle(identity string, status markerStatus) string {
	switch status {
	case statusPending, statusRunning:
		return "Using " + identity
	case statusCompleted:
		return "Used " + identity
	case statusFailed:
		return identity + " failed"
	case statusCancelled:
		return identity + " cancelled"
	case statusInterrupted:
		return identity + " interrupted"
	case statusSkipped:
		return identity + " skipped"
	}
	return "Using " + identity
}

func isBuiltin(name string) bool {
	for _, prefix := range []string{"web.", "task.", "project.", "adapter.", "artifact.", "file."} {
		if strings.HasPrefix(name, prefix) {
			return true
		}
	}
	return name == "search_memory" || name == "read_memory_page" || name == "update_own_name" || name == "mcp.connect_service"
}

func isFolded(name string) bool {
	switch name {
	case "task.continue_execution", "task.finish_execution", "task.submit_plan", "task.submit_result", "task.submit_review":
		return true
	default:
		return false
	}
}

func firstText(value any, paths ...[]string) string {
	for _, path := range paths {
		current := value
		valid := true
		for _, key := range path {
			object, ok := current.(map[string]any)
			if !ok {
				valid = false
				break
			}
			current, ok = object[key]
			if !ok {
				valid = false
				break
			}
		}
		if valid {
			if text, ok := current.(string); ok && strings.TrimSpace(text) != "" {
				return bounded(text)
			}
		}
	}
	return ""
}

func arrayQuantity(value any, path []string, noun string) string {
	current := value
	for _, key := range path {
		object, ok := current.(map[string]any)
		if !ok {
			return ""
		}
		current = object[key]
	}
	length, ok := arrayLength(current)
	if !ok {
		return ""
	}
	return quantity(length, noun)
}

func quantityValue(value any, path []string, noun string) string {
	current := value
	for _, key := range path {
		object, ok := current.(map[string]any)
		if !ok {
			return ""
		}
		current = object[key]
	}
	switch number := current.(type) {
	case json.Number:
		value, err := strconv.Atoi(string(number))
		if err != nil {
			return ""
		}
		return quantity(value, noun)
	case int:
		return quantity(number, noun)
	case int64:
		return quantity(int(number), noun)
	case uint64:
		return quantity(int(number), noun)
	case float64:
		return quantity(int(number), noun)
	default:
		return ""
	}
}

func arrayLength(value any) (int, bool) {
	if value == nil {
		return 0, false
	}
	rv := reflect.ValueOf(value)
	if rv.Kind() != reflect.Array && rv.Kind() != reflect.Slice {
		return 0, false
	}
	return rv.Len(), true
}

func quantity(value int, noun string) string {
	return strconv.Itoa(value) + " " + noun + func() string {
		if value == 1 {
			return ""
		}
		return "s"
	}()
}

func webTarget(arguments, result any) string {
	if title := firstText(result, []string{"title"}); title != "" {
		return title
	}
	value := firstText(arguments, []string{"url"})
	if value == "" {
		value = firstText(result, []string{"final_url"}, []string{"url"})
	}
	return webHost(value)
}

func browserPage(arguments, result any) string {
	value := firstText(result, []string{"snapshot", "url"}, []string{"url"})
	if value == "" {
		value = firstText(arguments, []string{"url"})
	}
	return webHost(value)
}

func webHost(value string) string {
	value = strings.TrimSpace(value)
	if value == "" {
		return ""
	}
	parsed, err := url.Parse(value)
	if err == nil && parsed.Hostname() != "" {
		host := parsed.Hostname()
		for strings.HasPrefix(host, "www.") {
			host = strings.TrimPrefix(host, "www.")
		}
		return host
	}
	return bounded(value)
}

func reviewDecision(value string) string {
	switch value {
	case "approve":
		return "Review approved"
	case "request_changes":
		return "Changes requested"
	case "needs_human":
		return "Human decision needed"
	default:
		return "Review complete"
	}
}

func readableIdentity(name string) string {
	value := strings.ToLower(strings.NewReplacer(".", " ", "_", " ", "-", " ").Replace(name))
	runes := []rune(value)
	if len(runes) == 0 {
		return value
	}
	runes[0] = unicode.ToUpper(runes[0])
	return string(runes)
}

func appendText(prefix, target string) string {
	if target == "" {
		return prefix
	}
	return prefix + " " + bounded(target)
}

func outcome(prefix, value string) string {
	if value == "" {
		return prefix
	}
	return prefix + " · " + bounded(value)
}

func lowerInitial(value string) string {
	runes := []rune(value)
	if len(runes) == 0 {
		return ""
	}
	return strings.ToLower(string(runes[0])) + string(runes[1:])
}

func bounded(value string) string {
	runes := []rune(strings.TrimSpace(value))
	if len(runes) <= 140 {
		return string(runes)
	}
	return string(runes[:137]) + "..."
}

func isEmptyObject(value any) bool {
	object, ok := value.(map[string]any)
	return !ok || len(object) == 0
}

func nonNilField(value any, key string) bool {
	object, ok := value.(map[string]any)
	if !ok {
		return false
	}
	field, exists := object[key]
	return exists && field != nil
}

// ReadableName preserves Rust's connected-tool display label.
func ReadableName(name string) string {
	switch name {
	case "search_memory":
		return "Search memory"
	case "update_own_name":
		return "Save name"
	case "web.search":
		return "Web Search"
	case "web.fetch":
		return "Fetched Web Page"
	case "file.parse":
		return "Parsed File"
	case "file.download":
		return "Downloaded File"
	}
	last := name[strings.LastIndex(name, ".")+1:]
	if strings.HasPrefix(name, "web.browse.") {
		return "Browser " + last
	}
	parts := strings.FieldsFunc(last, func(r rune) bool { return r == '_' || r == '-' })
	for i, part := range parts {
		chars := []byte(part)
		for j, ch := range chars {
			if ch >= 'A' && ch <= 'Z' {
				chars[j] += 'a' - 'A'
			}
		}
		if i == 0 && len(chars) > 0 && chars[0] >= 'a' && chars[0] <= 'z' {
			chars[0] -= 'a' - 'A'
		}
		parts[i] = string(chars)
	}
	return strings.Join(parts, " ")
}
