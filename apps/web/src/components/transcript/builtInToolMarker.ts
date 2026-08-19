export type ToolMarkerCallStatus =
  | "pending"
  | "running"
  | "complete"
  | "error"
  | "cancelled"
  | "interrupted"
  | "skipped";

export type BuiltInToolMarkerPresentation = {
  identity: string;
  name: string;
};

type PresentationInput = {
  toolName: string;
  status: ToolMarkerCallStatus;
  argumentsPayload: unknown;
  resultPayload: unknown;
};

type ActionCopy = {
  running: string;
  complete: string;
  failed: string;
};

const TASK_ACTIONS: Readonly<Record<string, ActionCopy>> = {
  "task.capture": action("Creating Task", "Created Task", "Could not create Task"),
  "task.delegate": action("Delegating Task", "Delegated Task", "Could not delegate Task"),
  "task.update": action("Updating Task", "Updated Task", "Could not update Task"),
  "task.queue": action("Queueing Task", "Queued Task", "Could not queue Task"),
  "task.schedule": action("Scheduling Task", "Scheduled Task", "Could not schedule Task"),
  "task.reschedule": action("Rescheduling Task", "Rescheduled Task", "Could not reschedule Task"),
  "task.unschedule": action("Removing Task schedule", "Removed Task schedule", "Could not remove Task schedule"),
  "task.schedule.run_now": action("Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task"),
  "task.recurrence.update": action("Updating Task schedule", "Updated Task schedule", "Could not update Task schedule"),
  "task.recurrence.pause": action("Pausing Task schedule", "Paused Task schedule", "Could not pause Task schedule"),
  "task.recurrence.resume": action("Resuming Task schedule", "Resumed Task schedule", "Could not resume Task schedule"),
  "task.recurrence.skip_next": action("Skipping next Task run", "Skipped next Task run", "Could not skip next Task run"),
  "task.recurrence.end": action("Ending Task schedule", "Ended Task schedule", "Could not end Task schedule"),
  "task.recurrence.run_now": action("Starting scheduled Task", "Started scheduled Task", "Could not start scheduled Task"),
  "task.answer": action("Answering Task question", "Answered Task question", "Could not answer Task question"),
  "task.retry": action("Retrying Task", "Retried Task", "Could not retry Task"),
  "task.cancel": action("Cancelling Task", "Cancelled Task", "Could not cancel Task"),
  "task.reopen": action("Reopening Task", "Reopened Task", "Could not reopen Task")
};

const PROJECT_ACTIONS: Readonly<Record<string, ActionCopy>> = {
  "project.create": action("Creating project", "Created project", "Could not create project"),
  "project.update": action("Updating project", "Updated project", "Could not update project"),
  "project.archive": action("Archiving project", "Archived project", "Could not archive project"),
  "project.reopen": action("Reopening project", "Reopened project", "Could not reopen project")
};

const FOLDED_TASK_TOOLS = new Set([
  "task.continue_execution",
  "task.finish_execution",
  "task.submit_plan",
  "task.submit_result",
  "task.submit_review"
]);

export function isFoldedTaskToolName(name: string): boolean {
  return FOLDED_TASK_TOOLS.has(name);
}

export function isBuiltInToolName(name: string): boolean {
  return (
    name.startsWith("web.") ||
    name.startsWith("task.") ||
    name.startsWith("project.") ||
    name.startsWith("adapter.") ||
    name.startsWith("artifact.") ||
    name.startsWith("file.") ||
    name === "search_memory" ||
    name === "read_memory_page" ||
    name === "update_own_name" ||
    name === "mcp.connect_service"
  );
}

export function builtInToolMarkerPresentation({
  toolName,
  status,
  argumentsPayload,
  resultPayload
}: PresentationInput): BuiltInToolMarkerPresentation | undefined {
  if (!isBuiltInToolName(toolName)) return undefined;

  const args = recordValue(argumentsPayload);
  const result = recordValue(resultPayload);
  const error = resultError(result);

  if (toolName === "web.search") {
    const query = firstText(args, [["query"]]) ?? firstText(result, [["query"]]);
    const subject = query ? `“${bounded(query)}”` : "the web";
    if (status === "complete") return presentation("Web Search", query ?? "Web search");
    return presentation("Web Search", stateful(status, {
      running: `Searching the web for ${subject}`,
      complete: query ?? "Web search",
      failed: `Could not search the web for ${subject}`
    }, error));
  }

  if (toolName === "web.fetch") {
    const target = webPageTarget(args, result);
    return presentation("Fetched Web Page", stateful(status, {
      running: appendTarget("Reading", target),
      complete: appendTarget("Read", target),
      failed: appendTarget("Could not read", target)
    }, error));
  }

  if (toolName.startsWith("web.browse.")) {
    return presentation(browserIdentity(toolName), browserMarkerName(toolName, status, args, result, error));
  }

  if (toolName === "task.files.list") {
    const path = firstText(args, [["path"]]) ?? ".";
    const count = arrayLength(result, ["entries"]);
    return presentation("List Task files", stateful(status, {
      running: `Listing files in ${bounded(path)}`,
      complete: joinOutcome(`Listed files in ${bounded(path)}`, countLabel(count, "item")),
      failed: `Could not list files in ${bounded(path)}`
    }, error));
  }

  if (toolName === "task.files.read") {
    const path = firstText(args, [["path"]]) ?? firstText(result, [["path"]]);
    return presentation("Read Task file", stateful(status, {
      running: appendTarget("Reading", path),
      complete: appendTarget("Read", path),
      failed: appendTarget("Could not read", path)
    }, error));
  }

  if (toolName === "task.files.write") {
    const path = firstText(args, [["path"]]) ?? firstText(result, [["path"]]);
    return presentation("Write Task file", stateful(status, {
      running: appendTarget("Writing", path),
      complete: appendTarget("Wrote", path),
      failed: appendTarget("Could not write", path)
    }, error));
  }

  if (toolName === "task.files.delete") {
    const path = firstText(args, [["path"]]);
    return presentation("Delete Task file", stateful(status, {
      running: appendTarget("Deleting", path),
      complete: appendTarget("Deleted", path),
      failed: appendTarget("Could not delete", path)
    }, error));
  }

  if (toolName === "task.list") {
    const scoped = !args || Object.keys(args).length === 0;
    const count = arrayLength(result, ["tasks"]);
    const copy = scoped ? action("Checking current Task", "Checked current Task", "Could not check current Task") : action("Listing Tasks", "Listed Tasks", "Could not list Tasks");
    const name = stateful(status, copy, error);
    return presentation("List Tasks", status === "complete" ? joinOutcome(name, countLabel(count, "Task")) : name);
  }

  if (toolName === "task.finish_planning") {
    const complexity = firstText(result, [["complexity"]]) ?? firstText(args, [["complexity"]]);
    return presentation("Finish planning", stateful(status, {
      running: "Finishing plan",
      complete: joinOutcome("Plan ready", complexity),
      failed: "Could not finish plan"
    }, error));
  }

  if (toolName === "task.finish_review") {
    const decision = firstText(result, [["decision"]]) ?? firstText(args, [["decision"]]);
    return presentation("Finish review", stateful(status, {
      running: "Finishing review",
      complete: reviewDecisionLabel(decision),
      failed: "Could not finish review"
    }, error));
  }

  if (toolName === "task.report_blocked") {
    const question = firstText(result, [["question"]]) ?? firstText(args, [["question"]]);
    return presentation("Report Task blocked", stateful(status, {
      running: "Reporting Task blocked",
      complete: appendTarget("Task blocked:", question),
      failed: "Could not report Task blocked"
    }, error));
  }

  if (toolName === "task.read_artifact") {
    const title = firstText(result, [["title"]]);
    return presentation("Read Task artifact", stateful(status, {
      running: appendTarget("Reading artifact", title),
      complete: appendTarget("Read artifact", title),
      failed: appendTarget("Could not read artifact", title)
    }, error));
  }

  if (toolName === "task.read_submission_evidence") {
    const count = arrayLength(result, ["items"]);
    return presentation("Read submission evidence", stateful(status, {
      running: "Checking submission evidence",
      complete: joinOutcome("Checked submission evidence", countLabel(count, "item")),
      failed: "Could not check submission evidence"
    }, error));
  }

  const taskAction = TASK_ACTIONS[toolName];
  if (taskAction) {
    return presentation(readableIdentity(toolName), stateful(status, targetAction(taskAction, taskTitle(args, result)), error));
  }

  if (FOLDED_TASK_TOOLS.has(toolName)) {
    return presentation(readableIdentity(toolName), readableIdentity(toolName));
  }

  if (toolName === "project.list") {
    const count = arrayLength(result, ["projects"]);
    return presentation("List projects", stateful(status, {
      running: "Listing projects",
      complete: joinOutcome("Listed projects", countLabel(count, "project")),
      failed: "Could not list projects"
    }, error));
  }

  const projectAction = PROJECT_ACTIONS[toolName];
  if (projectAction) {
    return presentation(readableIdentity(toolName), stateful(status, targetAction(projectAction, projectTitle(args, result)), error));
  }

  if (toolName === "search_memory") {
    const query = firstText(args, [["query"]]);
    const count = arrayLength(result, ["pages"]);
    const target = query ? `“${bounded(query)}”` : "memory";
    return presentation("Search memory", stateful(status, {
      running: `Searching memory for ${target}`,
      complete: joinOutcome(`Searched memory for ${target}`, countLabel(count, "page")),
      failed: `Could not search memory for ${target}`
    }, error));
  }

  if (toolName === "read_memory_page") {
    const page = firstText(args, [["page"]]) ?? firstText(result, [["page_ref"]]);
    return presentation("Read memory page", stateful(status, {
      running: appendTarget("Reading memory page", page),
      complete: appendTarget("Read memory page", page),
      failed: appendTarget("Could not read memory page", page)
    }, error));
  }

  if (toolName === "file.parse") {
    const path = firstText(args, [["path"]]) ?? firstText(result, [["path"]]);
    const chars = numberValue(result?.returned_chars);
    return presentation("Parse file", stateful(status, {
      running: appendTarget("Parsing", path),
      complete: joinOutcome(appendTarget("Parsed", path), countLabel(chars, "character")),
      failed: appendTarget("Could not parse", path)
    }, error));
  }

  if (toolName === "file.download") {
    const path = firstText(args, [["path"]]) ?? firstText(result, [["path"]]);
    const host = webHost(firstText(args, [["url"]]) ?? firstText(result, [["url"], ["final_url"]]));
    const target = path ? `${bounded(path)}${host ? ` from ${host}` : ""}` : host;
    return presentation("Download file", stateful(status, {
      running: appendTarget("Downloading", target),
      complete: appendTarget("Downloaded", target),
      failed: appendTarget("Could not download", target)
    }, error));
  }

  if (toolName === "artifact.create_local_file") {
    const target = firstText(result, [["title"]]) ?? firstText(args, [["filename"], ["title"]]);
    return presentation("Create local file", stateful(status, {
      running: appendTarget("Creating", target),
      complete: appendTarget("Created", target),
      failed: appendTarget("Could not create", target)
    }, error));
  }

  if (toolName === "adapter.definition_template") {
    return presentation("Definition template", stateful(status, action("Loading definition template", "Loaded definition template", "Could not load definition template"), error));
  }

  if (toolName === "adapter.propose_definition") {
    const target = firstText(result, [["display_name"]]) ?? firstText(args, [["source_reference"]]);
    return presentation("Propose definition", stateful(status, targetAction(action("Proposing definition", "Proposed definition", "Could not propose definition"), target), error));
  }

  if (toolName === "update_own_name") {
    const target = firstText(result, [["display_name"]]) ?? firstText(args, [["name"]]);
    return presentation("Save name", stateful(status, {
      running: appendTarget("Saving name as", target),
      complete: appendTarget("Saved name as", target),
      failed: "Could not save name"
    }, error));
  }

  if (toolName === "mcp.connect_service") {
    const target = firstText(result, [["display_name"]]) ?? webHost(firstText(args, [["service_url"]]));
    return presentation("Connect service", stateful(status, {
      running: appendTarget("Connecting", target),
      complete: appendTarget("Connected", target),
      failed: appendTarget("Could not connect", target)
    }, error));
  }

  return presentation(readableIdentity(toolName), readableIdentity(toolName));
}

function browserMarkerName(
  toolName: string,
  status: ToolMarkerCallStatus,
  args: Record<string, unknown> | null,
  result: Record<string, unknown> | null,
  error: string | undefined
): string {
  const page = firstText(result, [["title"]]) ?? webHost(firstText(result, [["url"]]) ?? firstText(args, [["url"]]));
  if (toolName === "web.browse.open" || toolName === "web.browse.navigate") {
    const verb = toolName.endsWith("open")
      ? action("Opening", "Opened", "Could not open")
      : action("Navigating to", "Navigated to", "Could not navigate to");
    return stateful(status, targetAction(verb, page), error);
  }
  if (toolName === "web.browse.interact") {
    const interaction = browserInteraction(firstText(args, [["action"]]));
    return stateful(status, interaction, error);
  }
  if (toolName === "web.browse.snapshot") {
    return stateful(status, targetAction(action("Capturing page", "Captured", "Could not capture"), page), error);
  }
  if (toolName === "web.browse.wait") {
    const condition = firstText(args, [["text"]]);
    return stateful(status, targetAction(action("Waiting for", "Waited for", "Could not wait for"), condition ?? "page"), error);
  }
  if (toolName === "web.browse.history") {
    return stateful(status, action("Checking browser history", "Checked browser history", "Could not check browser history"), error);
  }
  if (toolName === "web.browse.close") {
    return stateful(status, action("Closing browser", "Closed browser", "Could not close browser"), error);
  }
  return stateful(status, action("Using browser", "Used browser", "Browser action failed"), error);
}

function browserInteraction(actionName: string | undefined): ActionCopy {
  switch (actionName) {
    case "click":
      return action("Clicking page element", "Clicked page element", "Could not click page element");
    case "fill":
    case "type":
      return action("Filling page field", "Filled page field", "Could not fill page field");
    case "press":
      return action("Pressing key", "Pressed key", "Could not press key");
    case "select":
      return action("Selecting page option", "Selected page option", "Could not select page option");
    default:
      return action("Using page control", "Used page control", "Could not use page control");
  }
}

function stateful(status: ToolMarkerCallStatus, copy: ActionCopy, error?: string): string {
  if (status === "error") return joinOutcome(copy.failed, error);
  if (status === "cancelled") return `Cancelled: ${lowerInitial(copy.running)}`;
  if (status === "interrupted") return `Interrupted: ${lowerInitial(copy.running)}`;
  if (status === "skipped") return `Skipped: ${lowerInitial(copy.running)}`;
  return status === "complete" ? copy.complete : copy.running;
}

function targetAction(copy: ActionCopy, target: string | undefined): ActionCopy {
  return {
    running: appendTarget(copy.running, target),
    complete: appendTarget(copy.complete, target),
    failed: appendTarget(copy.failed, target)
  };
}

function taskTitle(args: Record<string, unknown> | null, result: Record<string, unknown> | null): string | undefined {
  return firstText(result, [["task", "title"]]) ?? firstText(args, [["title"]]);
}

function projectTitle(args: Record<string, unknown> | null, result: Record<string, unknown> | null): string | undefined {
  return firstText(result, [["project", "title"], ["project", "name"]]) ?? firstText(args, [["title"], ["name"]]);
}

function webPageTarget(args: Record<string, unknown> | null, result: Record<string, unknown> | null): string | undefined {
  return firstText(result, [["title"]]) ?? webHost(firstText(args, [["url"]]) ?? firstText(result, [["final_url"], ["url"]]));
}

function resultError(result: Record<string, unknown> | null): string | undefined {
  return firstText(result, [["error"], ["message"], ["details", "message"]]);
}

function firstText(value: Record<string, unknown> | null, paths: readonly (readonly string[])[]): string | undefined {
  for (const path of paths) {
    let candidate: unknown = value;
    for (const key of path) candidate = recordValue(candidate)?.[key];
    if (typeof candidate === "string" && candidate.trim()) return bounded(candidate);
  }
  return undefined;
}

function arrayLength(value: Record<string, unknown> | null, path: readonly string[]): number | undefined {
  let candidate: unknown = value;
  for (const key of path) candidate = recordValue(candidate)?.[key];
  return Array.isArray(candidate) ? candidate.length : undefined;
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function countLabel(count: number | undefined, noun: string): string | undefined {
  if (count === undefined) return undefined;
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

function reviewDecisionLabel(decision: string | undefined): string {
  switch (decision) {
    case "approve": return "Review approved";
    case "request_changes": return "Changes requested";
    case "needs_human": return "Human decision needed";
    default: return "Review complete";
  }
}

function browserIdentity(name: string): string {
  return `Browser ${name.split(".").at(-1)?.replaceAll("_", " ") ?? "action"}`;
}

function readableIdentity(name: string): string {
  return name
    .split(".")
    .filter(Boolean)
    .flatMap((part) => part.split(/[_-]+/u))
    .filter(Boolean)
    .map((part, index) => index === 0 ? `${part[0]?.toUpperCase() ?? ""}${part.slice(1)}` : part.toLowerCase())
    .join(" ");
}

function webHost(value: string | undefined): string | undefined {
  if (!value) return undefined;
  try {
    const url = new URL(value);
    return url.hostname.replace(/^www\./u, "") || undefined;
  } catch {
    return bounded(value);
  }
}

function action(running: string, complete: string, failed: string): ActionCopy {
  return { running, complete, failed };
}

function presentation(identity: string, name: string): BuiltInToolMarkerPresentation {
  return { identity, name };
}

function appendTarget(prefix: string, target: string | undefined): string {
  return target ? `${prefix} ${bounded(target)}` : prefix;
}

function joinOutcome(prefix: string, outcome: string | undefined): string {
  return outcome ? `${prefix} · ${bounded(outcome)}` : prefix;
}

function lowerInitial(value: string): string {
  return `${value[0]?.toLowerCase() ?? ""}${value.slice(1)}`;
}

function bounded(value: string): string {
  const trimmed = value.trim();
  return trimmed.length > 140 ? `${trimmed.slice(0, 137)}...` : trimmed;
}

function recordValue(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}
