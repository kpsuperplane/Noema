//! Shared built-in tool marker text for every client surface.

use serde_json::{Value, json};
use url::Url;

/// Build transient marker data from a saved action envelope.
#[must_use]
pub fn tool_marker_for_action(action_kind: &str, status: &str, action: &Value) -> Option<Value> {
    let name = action.get("name")?.as_str()?.trim();
    let is_result = action_kind.eq_ignore_ascii_case("tool_result");
    let payload = action.get("payload");
    let arguments = action
        .get("arguments")
        .or_else(|| (!is_result).then_some(payload).flatten());
    let arguments = match arguments.and_then(Value::as_object) {
        Some(object) if object.len() == 1 => object.get("arguments").or(arguments),
        _ => arguments,
    };
    tool_marker(
        name,
        status,
        is_result,
        arguments,
        is_result.then_some(payload).flatten(),
    )
}

/// Build transient marker data from saved built-in tool facts.
fn tool_marker(
    name: &str,
    status: &str,
    is_result: bool,
    arguments: Option<&Value>,
    result: Option<&Value>,
) -> Option<Value> {
    if !is_builtin(name) {
        return None;
    }
    let status = MarkerStatus::new(status, is_result);
    if name == "web.browse.open"
        && is_result
        && matches!(status, MarkerStatus::Failed)
        && browser_page(arguments, result).is_none()
    {
        return None;
    }
    let error = text(result, &[&["error"], &["message"], &["details", "message"]]);
    let (identity, summary) = marker_text(name, status, arguments, result, error.as_deref());
    let mut marker = json!({
        "identity": identity,
        "summary": summary,
        "detailTitle": detail_title(&identity, status),
        "status": status.as_str(),
    });
    if matches!(name, "web.search" | "web.fetch") {
        marker["kind"] = json!(name);
    }
    if is_folded(name) {
        marker["visibility"] = json!("fold");
    }
    Some(marker)
}

#[derive(Clone, Copy)]
enum MarkerStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
    Skipped,
}

impl MarkerStatus {
    fn new(value: &str, is_result: bool) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "pending" | "queued" => Self::Pending,
            "running" | "started" | "active" => Self::Running,
            "failed" | "error" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "interrupted" => Self::Interrupted,
            "skipped" => Self::Skipped,
            _ if is_result => Self::Completed,
            _ => Self::Pending,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Clone, Copy)]
enum Subject {
    None,
    Path,
    Task,
    Project,
    Artifact,
    Adapter,
    Service,
}

type Rule = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Subject,
);

#[rustfmt::skip]
const RULES: &[Rule] = &[
    ("task.files.read", "Read Task file", "Reading", "Read", "Could not read", Subject::Path),
    ("task.files.write", "Write Task file", "Writing", "Wrote", "Could not write", Subject::Path),
    ("task.files.delete", "Delete Task file", "Deleting", "Deleted", "Could not delete", Subject::Path),
    ("task.capture", "Create Task", "Creating Task", "Created Task", "Could not create Task", Subject::Task),
    ("task.delegate", "Delegate Task", "Delegating Task", "Delegated Task", "Could not delegate Task", Subject::Task),
    ("task.update", "Update Task", "Updating Task", "Updated Task", "Could not update Task", Subject::Task),
    ("task.queue", "Queue Task", "Queueing Task", "Queued Task", "Could not queue Task", Subject::Task),
    ("task.schedule", "Schedule Task", "Scheduling Task", "Scheduled Task", "Could not schedule Task", Subject::Task),
    ("task.reschedule", "Reschedule Task", "Rescheduling Task", "Rescheduled Task", "Could not reschedule Task", Subject::Task),
    ("task.unschedule", "Remove Task schedule", "Removing Task schedule", "Removed Task schedule", "Could not remove Task schedule", Subject::Task),
    ("task.schedule.run_now", "Start scheduled Task", "Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task", Subject::Task),
    ("task.recurrence.run_now", "Start scheduled Task", "Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task", Subject::Task),
    ("task.recurrence.update", "Update Task schedule", "Updating Task schedule", "Updated Task schedule", "Could not update Task schedule", Subject::Task),
    ("task.recurrence.pause", "Pause Task schedule", "Pausing Task schedule", "Paused Task schedule", "Could not pause Task schedule", Subject::Task),
    ("task.recurrence.resume", "Resume Task schedule", "Resuming Task schedule", "Resumed Task schedule", "Could not resume Task schedule", Subject::Task),
    ("task.recurrence.skip_next", "Skip next Task run", "Skipping next Task run", "Skipped next Task run", "Could not skip next Task run", Subject::Task),
    ("task.recurrence.end", "End Task schedule", "Ending Task schedule", "Ended Task schedule", "Could not end Task schedule", Subject::Task),
    ("task.answer", "Answer Task question", "Answering Task question", "Answered Task question", "Could not answer Task question", Subject::Task),
    ("task.retry", "Retry Task", "Retrying Task", "Retried Task", "Could not retry Task", Subject::Task),
    ("task.cancel", "Cancel Task", "Cancelling Task", "Cancelled Task", "Could not cancel Task", Subject::Task),
    ("task.reopen", "Reopen Task", "Reopening Task", "Reopened Task", "Could not reopen Task", Subject::Task),
    ("task.read_artifact", "Read Task artifact", "Reading artifact", "Read artifact", "Could not read artifact", Subject::Artifact),
    ("project.create", "Create project", "Creating project", "Created project", "Could not create project", Subject::Project),
    ("project.update", "Update project", "Updating project", "Updated project", "Could not update project", Subject::Project),
    ("project.archive", "Archive project", "Archiving project", "Archived project", "Could not archive project", Subject::Project),
    ("project.reopen", "Reopen project", "Reopening project", "Reopened project", "Could not reopen project", Subject::Project),
    ("read_memory_page", "Read memory page", "Reading memory page", "Read memory page", "Could not read memory page", Subject::Path),
    ("artifact.create_local_file", "Create local file", "Creating", "Created", "Could not create", Subject::Artifact),
    ("adapter.definition_template", "Definition template", "Loading definition template", "Loaded definition template", "Could not load definition template", Subject::None),
    ("adapter.propose_definition", "Propose definition", "Proposing definition", "Proposed definition", "Could not propose definition", Subject::Adapter),
    ("mcp.connect_service", "Connect service", "Connecting", "Connected", "Could not connect", Subject::Service),
];

fn marker_text(
    name: &str,
    status: MarkerStatus,
    arguments: Option<&Value>,
    result: Option<&Value>,
    error: Option<&str>,
) -> (String, String) {
    if let Some(value) = special_marker(name, status, arguments, result, error) {
        return value;
    }
    if let Some(rule) = RULES.iter().find(|rule| rule.0 == name) {
        let target = subject(rule.5, arguments, result);
        return named(
            rule.1,
            status,
            Copy::new(rule.2, rule.3, rule.4).target(target.as_deref()),
            error,
        );
    }
    let identity = readable_identity(name);
    (identity.clone(), identity)
}

fn special_marker(
    name: &str,
    status: MarkerStatus,
    arguments: Option<&Value>,
    result: Option<&Value>,
    error: Option<&str>,
) -> Option<(String, String)> {
    let marker = match name {
        "web.search" => {
            let query = text(arguments, &[&["query"]]).or_else(|| text(result, &[&["query"]]));
            let target = query
                .as_ref()
                .map_or_else(|| "the web".to_string(), |value| format!("“{value}”"));
            named(
                "Web Search",
                status,
                Copy::new(
                    format!("Searching the web for {target}"),
                    query.unwrap_or_else(|| "Web search".into()),
                    format!("Could not search the web for {target}"),
                ),
                error,
            )
        }
        "web.fetch" => named(
            "Fetched Web Page",
            status,
            Copy::new("Reading", "Read", "Could not read")
                .target(web_target(arguments, result).as_deref()),
            error,
        ),
        name if name == "web.browse" || name.starts_with("web.browse.") => {
            browser_marker(name, status, arguments, result, error)
        }
        "task.files.list" => {
            let path = text(arguments, &[&["path"]]).unwrap_or_else(|| ".".into());
            let scope = if path == "." {
                String::new()
            } else {
                format!(" in {path}")
            };
            named(
                "List Task files",
                status,
                Copy::new(
                    format!("Listing Task files{scope}"),
                    outcome(
                        format!("Listed Task files{scope}"),
                        count(result, &["entries"], "item"),
                    ),
                    format!("Could not list Task files{scope}"),
                ),
                error,
            )
        }
        "task.list" => {
            let current = arguments
                .and_then(Value::as_object)
                .is_none_or(serde_json::Map::is_empty);
            let copy = if current {
                Copy::new(
                    "Checking current Task",
                    "Checked current Task",
                    "Could not check current Task",
                )
            } else {
                Copy::new(
                    "Listing Tasks",
                    outcome("Listed Tasks".into(), count(result, &["tasks"], "Task")),
                    "Could not list Tasks",
                )
            };
            named("List Tasks", status, copy, error)
        }
        "task.finish_planning" => named(
            "Finish planning",
            status,
            Copy::new(
                "Finishing plan",
                outcome(
                    "Plan ready".into(),
                    text(result, &[&["complexity"]])
                        .or_else(|| text(arguments, &[&["complexity"]])),
                ),
                "Could not finish plan",
            ),
            error,
        ),
        "task.finish_review" => {
            let decision =
                text(result, &[&["decision"]]).or_else(|| text(arguments, &[&["decision"]]));
            named(
                "Finish review",
                status,
                Copy::new(
                    "Finishing review",
                    review_decision(decision.as_deref()),
                    "Could not finish review",
                ),
                error,
            )
        }
        "task.report_blocked" => {
            let question =
                text(result, &[&["question"]]).or_else(|| text(arguments, &[&["question"]]));
            named(
                "Report Task blocked",
                status,
                Copy::new(
                    "Reporting Task blocked",
                    append("Task blocked:", question.as_deref()),
                    "Could not report Task blocked",
                ),
                error,
            )
        }
        "task.read_submission_evidence" => named(
            "Read submission evidence",
            status,
            Copy::new(
                "Checking submission evidence",
                outcome(
                    "Checked submission evidence".into(),
                    count(result, &["items"], "item"),
                ),
                "Could not check submission evidence",
            ),
            error,
        ),
        name if is_folded(name) => {
            let identity = readable_identity(name);
            (identity.clone(), identity)
        }
        "project.list" => named(
            "List projects",
            status,
            Copy::new(
                "Listing projects",
                outcome(
                    "Listed projects".into(),
                    count(result, &["projects"], "project"),
                ),
                "Could not list projects",
            ),
            error,
        ),
        "search_memory" => {
            let query = text(arguments, &[&["query"]]);
            let target = query.map_or_else(|| "memory".into(), |value| format!("“{value}”"));
            named(
                "Search memory",
                status,
                Copy::new(
                    format!("Searching memory for {target}"),
                    outcome(
                        format!("Searched memory for {target}"),
                        count(result, &["pages"], "page"),
                    ),
                    format!("Could not search memory for {target}"),
                ),
                error,
            )
        }
        "file.parse" => {
            let path = text(arguments, &[&["path"]]).or_else(|| text(result, &[&["path"]]));
            let chars = result
                .and_then(|value| value.get("returned_chars"))
                .and_then(Value::as_u64)
                .map(|value| quantity(value, "character"));
            named(
                "Parse file",
                status,
                Copy::new(
                    append("Parsing", path.as_deref()),
                    outcome(append("Parsed", path.as_deref()), chars),
                    append("Could not parse", path.as_deref()),
                ),
                error,
            )
        }
        "file.download" => {
            let path = text(arguments, &[&["path"]]).or_else(|| text(result, &[&["path"]]));
            let host = text(arguments, &[&["url"]])
                .or_else(|| text(result, &[&["url"], &["final_url"]]))
                .and_then(|value| web_host(&value));
            let target = path
                .map(|path| {
                    host.as_ref()
                        .map_or(path.clone(), |host| format!("{path} from {host}"))
                })
                .or(host);
            named(
                "Download file",
                status,
                Copy::new("Downloading", "Downloaded", "Could not download")
                    .target(target.as_deref()),
                error,
            )
        }
        "update_own_name" => {
            let target =
                text(result, &[&["display_name"]]).or_else(|| text(arguments, &[&["name"]]));
            named(
                "Save name",
                status,
                Copy::new(
                    append("Saving name as", target.as_deref()),
                    append("Saved name as", target.as_deref()),
                    "Could not save name",
                ),
                error,
            )
        }
        _ => return None,
    };
    Some(marker)
}

fn browser_marker(
    name: &str,
    status: MarkerStatus,
    arguments: Option<&Value>,
    result: Option<&Value>,
    error: Option<&str>,
) -> (String, String) {
    let page = browser_page(arguments, result);
    let copy = match name {
        "web.browse.open" => {
            Copy::new("Opening", "Opened", "Could not open").target(page.as_deref())
        }
        "web.browse.navigate" => {
            Copy::new("Navigating to", "Navigated to", "Could not navigate to")
                .target(page.as_deref())
        }
        "web.browse.snapshot" => {
            Copy::new("Capturing page", "Captured", "Could not capture").target(page.as_deref())
        }
        "web.browse.wait" => Copy::new("Waiting for", "Waited for", "Could not wait for")
            .target(text(arguments, &[&["text"]]).as_deref().or(Some("page"))),
        "web.browse.history" => Copy::new(
            "Checking browser history",
            "Checked browser history",
            "Could not check browser history",
        ),
        "web.browse.switch_provider" => Copy::new(
            "Switching browser provider",
            "Switched browser provider",
            "Could not switch browser provider",
        )
        .target(page.as_deref()),
        "web.browse.close" => Copy::new(
            "Closing browser",
            "Closed browser",
            "Could not close browser",
        ),
        "web.browse.interact" => match text(arguments, &[&["action"]]).as_deref() {
            Some("click") => Copy::new(
                "Clicking page element",
                "Clicked page element",
                "Could not click page element",
            ),
            Some("fill" | "type") => Copy::new(
                "Filling page field",
                "Filled page field",
                "Could not fill page field",
            ),
            Some("press") => Copy::new("Pressing key", "Pressed key", "Could not press key"),
            Some("select") => Copy::new(
                "Selecting page option",
                "Selected page option",
                "Could not select page option",
            ),
            _ => Copy::new(
                "Using page control",
                "Used page control",
                "Could not use page control",
            ),
        },
        _ => Copy::new("Using browser", "Used browser", "Browser action failed"),
    };
    named(
        &format!(
            "Browser {}",
            name.rsplit('.')
                .next()
                .unwrap_or("action")
                .replace('_', " ")
        ),
        status,
        copy,
        error,
    )
}

fn browser_page(arguments: Option<&Value>, result: Option<&Value>) -> Option<String> {
    text(result, &[&["title"]]).or_else(|| {
        text(result, &[&["url"]])
            .or_else(|| text(arguments, &[&["url"]]))
            .and_then(|value| web_host(&value))
    })
}

struct Copy {
    running: String,
    completed: String,
    failed: String,
}

impl Copy {
    fn new(
        running: impl Into<String>,
        completed: impl Into<String>,
        failed: impl Into<String>,
    ) -> Self {
        Self {
            running: running.into(),
            completed: completed.into(),
            failed: failed.into(),
        }
    }

    fn target(self, target: Option<&str>) -> Self {
        Self::new(
            append(&self.running, target),
            append(&self.completed, target),
            append(&self.failed, target),
        )
    }
}

fn named(
    identity: &str,
    status: MarkerStatus,
    copy: Copy,
    error: Option<&str>,
) -> (String, String) {
    let summary = match status {
        MarkerStatus::Failed => outcome(copy.failed, error.map(bounded)),
        MarkerStatus::Cancelled => format!("Cancelled: {}", lower_initial(&copy.running)),
        MarkerStatus::Interrupted => format!("Interrupted: {}", lower_initial(&copy.running)),
        MarkerStatus::Skipped => format!("Skipped: {}", lower_initial(&copy.running)),
        MarkerStatus::Completed => copy.completed,
        MarkerStatus::Pending | MarkerStatus::Running => copy.running,
    };
    (identity.to_string(), summary)
}

fn subject(kind: Subject, arguments: Option<&Value>, result: Option<&Value>) -> Option<String> {
    match kind {
        Subject::None => None,
        Subject::Path => text(arguments, &[&["path"], &["page"]])
            .or_else(|| text(result, &[&["path"], &["page_ref"]])),
        Subject::Task => {
            text(result, &[&["task", "title"]]).or_else(|| text(arguments, &[&["title"]]))
        }
        Subject::Project => text(result, &[&["project", "title"], &["project", "name"]])
            .or_else(|| text(arguments, &[&["title"], &["name"]])),
        Subject::Artifact => {
            text(result, &[&["title"]]).or_else(|| text(arguments, &[&["filename"], &["title"]]))
        }
        Subject::Adapter => {
            text(result, &[&["display_name"]]).or_else(|| text(arguments, &[&["source_reference"]]))
        }
        Subject::Service => text(result, &[&["display_name"]])
            .or_else(|| text(arguments, &[&["service_url"]]).and_then(|value| web_host(&value))),
    }
}

fn detail_title(identity: &str, status: MarkerStatus) -> String {
    match status {
        MarkerStatus::Pending | MarkerStatus::Running => format!("Using {identity}"),
        MarkerStatus::Completed => format!("Used {identity}"),
        MarkerStatus::Failed => format!("{identity} failed"),
        MarkerStatus::Cancelled => format!("{identity} cancelled"),
        MarkerStatus::Interrupted => format!("{identity} interrupted"),
        MarkerStatus::Skipped => format!("{identity} skipped"),
    }
}

fn is_builtin(name: &str) -> bool {
    [
        "web.",
        "task.",
        "project.",
        "adapter.",
        "artifact.",
        "file.",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
        || matches!(
            name,
            "search_memory" | "read_memory_page" | "update_own_name" | "mcp.connect_service"
        )
}

fn is_folded(name: &str) -> bool {
    matches!(
        name,
        "task.continue_execution"
            | "task.finish_execution"
            | "task.submit_plan"
            | "task.submit_result"
            | "task.submit_review"
    )
}

fn text(value: Option<&Value>, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let value = path.iter().try_fold(value?, |value, key| value.get(*key))?;
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(bounded)
    })
}

fn count(value: Option<&Value>, path: &[&str], noun: &str) -> Option<String> {
    let value = path.iter().try_fold(value?, |value, key| value.get(*key))?;
    Some(quantity(u64::try_from(value.as_array()?.len()).ok()?, noun))
}

fn quantity(value: u64, noun: &str) -> String {
    format!("{value} {noun}{}", if value == 1 { "" } else { "s" })
}

fn web_target(arguments: Option<&Value>, result: Option<&Value>) -> Option<String> {
    text(result, &[&["title"]]).or_else(|| {
        text(arguments, &[&["url"]])
            .or_else(|| text(result, &[&["final_url"], &["url"]]))
            .and_then(|value| web_host(&value))
    })
}

fn web_host(value: &str) -> Option<String> {
    Url::parse(value)
        .ok()
        .and_then(|url| {
            url.host_str()
                .map(|host| host.trim_start_matches("www.").to_string())
        })
        .or_else(|| Some(bounded(value)))
}

fn review_decision(value: Option<&str>) -> &'static str {
    match value {
        Some("approve") => "Review approved",
        Some("request_changes") => "Changes requested",
        Some("needs_human") => "Human decision needed",
        _ => "Review complete",
    }
}

fn readable_identity(name: &str) -> String {
    let value = name.replace(['.', '_', '-'], " ").to_ascii_lowercase();
    let mut chars = value.chars();
    chars.next().map_or(value.clone(), |first| {
        first.to_ascii_uppercase().to_string() + chars.as_str()
    })
}

fn append(prefix: &str, target: Option<&str>) -> String {
    target.map_or_else(
        || prefix.to_string(),
        |target| format!("{prefix} {}", bounded(target)),
    )
}

fn outcome(prefix: String, value: Option<String>) -> String {
    value.map_or(prefix.clone(), |value| {
        format!("{prefix} · {}", bounded(&value))
    })
}

fn lower_initial(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().to_string() + chars.as_str()
    })
}

fn bounded(value: &str) -> String {
    let value = value.trim();
    if value.chars().count() <= 140 {
        value.to_string()
    } else {
        value.chars().take(137).collect::<String>() + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_preserves_web_search_query_and_exact_status() {
        let marker = tool_marker(
            "web.search",
            "cancelled",
            true,
            Some(&json!({"query": "Noema tools"})),
            Some(&json!({"query": "Noema tools"})),
        )
        .expect("marker");
        assert_eq!(marker["kind"], "web.search");
        assert_eq!(
            marker["summary"],
            "Cancelled: searching the web for “Noema tools”"
        );
        assert_eq!(marker["status"], "cancelled");
    }

    #[test]
    fn marker_names_task_outcomes_and_folds_lifecycle_noise() {
        let list = tool_marker(
            "task.files.list",
            "running",
            false,
            Some(&json!({"path": "."})),
            None,
        )
        .expect("file list marker");
        assert_eq!(list["summary"], "Listing Task files");

        let file = tool_marker(
            "task.files.write",
            "completed",
            true,
            Some(&json!({"path": "RESULT.md"})),
            Some(&json!({"path": "RESULT.md"})),
        )
        .expect("file marker");
        assert_eq!(file["summary"], "Wrote RESULT.md");
        let lifecycle = tool_marker("task.finish_execution", "completed", true, None, None)
            .expect("lifecycle marker");
        assert_eq!(lifecycle["visibility"], "fold");
    }

    #[test]
    fn marker_excludes_connected_tools_and_sensitive_browser_input() {
        assert!(
            tool_marker(
                "gmail_personal.search_messages",
                "completed",
                true,
                None,
                None
            )
            .is_none()
        );
        let marker = tool_marker(
            "web.browse.interact",
            "completed",
            true,
            Some(&json!({"action": "fill", "value": "private text"})),
            Some(&json!({"title": "Sign in"})),
        )
        .expect("browser marker");
        assert_eq!(marker["summary"], "Filled page field");
        assert!(!marker.to_string().contains("private text"));

        let open = tool_marker(
            "web.browse.open",
            "failed",
            false,
            Some(&json!({"url": "https://www.example.com/docs"})),
            None,
        )
        .expect("browser-open call marker");
        assert_eq!(open["summary"], "Could not open example.com");
        assert!(
            tool_marker(
                "web.browse.open",
                "failed",
                true,
                None,
                Some(&json!({"error": "connection failed"})),
            )
            .is_none()
        );
    }
}
