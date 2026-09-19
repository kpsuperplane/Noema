import type { ToolMarkerGroup } from "./renderModel";
export type ToolDetailRowData = { label: string; value: string };
export type ToolScreenshotData = {
  src: string;
  width: number;
  height: number;
  url: string | null;
};
export type ToolMarkerKind = "web.search" | "web.fetch" | "web.browse" | "thinking" | "mcp";
export type ToolMarkerCallStatus = "pending" | "running" | "complete" | "error" | "cancelled" | "interrupted" | "skipped";

const MAX_SCREENSHOT_DATA_CHARS = 1_200_000;

export function toolMarkerPending(marker: ToolMarkerGroup): boolean {
  const status = toolMarkerStatus(marker);
  return status === "pending" || status === "running";
}

export function toolMarkerStatus(marker: ToolMarkerGroup): ToolMarkerCallStatus {
  if (marker.message) {
    const metadata = marker.message.metadata;
    const status = isRecord(metadata) ? metadata.output_status : undefined;
    return status === "running" ? "running" : status === "failed" ? "error" : "complete";
  }
  if (marker.result) {
    return activityMarkerStatus(marker.result.item.status, marker.result.item.metadata, true);
  }
  if (marker.call) {
    return activityMarkerStatus(marker.call.item.status, marker.call.item.metadata, false);
  }
  return "pending";
}

export function toolMarkerLabel(marker: ToolMarkerGroup): string {
  const displayTitle = markerDisplayString(marker, "detailTitle");
  if (displayTitle) {
    return displayTitle;
  }
  const toolName = toolMarkerIdentity(marker);
  if (toolName) {
    switch (toolMarkerStatus(marker)) {
      case "pending":
      case "running":
        return `Using ${toolName}`;
      case "error":
        return `${toolName} failed`;
      case "cancelled":
        return `${toolName} cancelled`;
      case "interrupted":
        return `${toolName} interrupted`;
      case "skipped":
        return `${toolName} skipped`;
      case "complete":
        return `Used ${toolName}`;
    }
  }
  return marker.call?.item.title ?? marker.result?.item.title ?? "Tool activity";
}

export function toolMarkerName(marker: ToolMarkerGroup): string {
  if (marker.message) return marker.message.text;
  const markerSummary = markerDisplayString(marker, "summary");
  if (markerSummary) {
    return markerSummary;
  }
  const description = toolDisplayString(marker.call?.item.metadata, "description");
  if (description) {
    return description;
  }
  return (
    toolNameFromMetadata(marker.call?.item.metadata) ??
    toolNameFromMetadata(marker.result?.item.metadata) ??
    marker.call?.item.title ??
    marker.result?.item.title ??
    "Tool activity"
  );
}

export function toolMarkerSummary(marker: ToolMarkerGroup): string {
  const name = toolMarkerName(marker);
  const subject = mcpMarkerSubject(marker);
  return subject ? `${name}: ${subject}` : name;
}

function isMcpMarker(marker: ToolMarkerGroup): boolean {
  const name = actionToolNameFromMetadata(marker.call?.item.metadata)
    ?? actionToolNameFromMetadata(marker.result?.item.metadata);
  return name?.startsWith("mcp.") === true && name !== "mcp.connect_service";
}

export function toolMarkerServerIcon(marker: ToolMarkerGroup): string | undefined {
  if (!isMcpMarker(marker)) return undefined;
  const payload = toolActionPayload(marker.result?.item.metadata);
  const meta = isRecord(payload) ? payload._meta : undefined;
  const server = isRecord(meta) ? meta["io.modelcontextprotocol/serverInfo"] : undefined;
  const icons = isRecord(server) && Array.isArray(server.icons) ? server.icons : [];
  for (const icon of icons) {
    if (!isRecord(icon) || typeof icon.src !== "string") continue;
    try {
      const url = new URL(icon.src);
      if (url.protocol === "https:" && !url.username && !url.password) return url.href;
    } catch { /* Ignore invalid server icon URLs. */ }
  }
  return undefined;
}

function mcpMarkerSubject(marker: ToolMarkerGroup): string | undefined {
  if (!isMcpMarker(marker)) return undefined;
  const payload = toolActionPayload(marker.result?.item.metadata);
  let result = isRecord(payload) ? payload.structuredContent : undefined;
  if (!isRecord(result) && isRecord(payload) && Array.isArray(payload.content) && payload.content.length === 1) {
    const content = payload.content[0];
    if (isRecord(content) && content.type === "text" && typeof content.text === "string") {
      try { result = JSON.parse(content.text); } catch { /* Unstructured output has no page title. */ }
    }
  }
  const input = toolActionPayload(marker.call?.item.metadata);
  // Use explicit resource fields, never the page body or arbitrary result text.
  const title = toolMarkerStatus(marker) === "complete" && isRecord(result) ? stringValue(result.title) : null;
  if (title) return title;
  if (isRecord(input)) {
    for (const key of ["title", "name", "url", "page_id", "id", "query"]) {
      const value = stringValue(input[key]);
      if (value) return value;
    }
  }
  return undefined;
}

export function toolMarkerKind(marker: ToolMarkerGroup): ToolMarkerKind | undefined {
  if (marker.message) return "thinking";
  if (isMcpMarker(marker)) return "mcp";
  const displayKind = markerDisplayString(marker, "kind");
  if (displayKind === "web.search" || displayKind === "web.fetch" || displayKind === "web.browse") {
    return displayKind;
  }
  const toolName = actionToolNameFromMetadata(marker.result?.item.metadata) ?? actionToolNameFromMetadata(marker.call?.item.metadata);
  if (toolName === "web.search" || toolName === "web.fetch") {
    return toolName;
  }
  return toolName?.startsWith("web.browse.") ? "web.browse" : undefined;
}

export function toolMarkerFaviconHost(marker: ToolMarkerGroup): string | undefined {
  if (toolMarkerKind(marker) !== "web.browse") {
    return undefined;
  }
  const markerHost = markerDisplayString(marker, "host");
  if (markerHost) {
    return markerHost;
  }
  const url = browserMarkerUrl(marker);
  if (!url) {
    return undefined;
  }
  try {
    return new URL(url).hostname.replace(/^www\./i, "");
  } catch {
    return undefined;
  }
}

function toolMarkerIdentity(marker: ToolMarkerGroup): string | null {
  return (
    markerDisplayString(marker, "identity") ??
    toolNameFromMetadata(marker.call?.item.metadata) ??
    toolNameFromMetadata(marker.result?.item.metadata) ??
    marker.call?.item.title ??
    marker.result?.item.title ??
    null
  );
}

export function formatToolDetail(fallback: string, metadata: unknown): string {
  const preview = safeToolMetadataPreview(metadata);
  if (!preview.length) {
    return fallback;
  }
  return preview.join("\n");
}

export function toolDetailRows(marker: ToolMarkerGroup): ToolDetailRowData[] {
  if (marker.message) return [];
  const completeRows = completeToolDetailRows(marker);
  if (completeRows) {
    return dedupeToolDetailRows(completeRows);
  }
  if (isSuccessfulWebSearchMarker(marker) || isSuccessfulWebFetchMarker(marker)) {
    return dedupeToolDetailRows(webFallbackDetailRows(marker));
  }

  const rows: ToolDetailRowData[] = [];
  const call = marker.call?.item;
  const result = marker.result?.item;

  rows.push(...toolPayloadRows("Input", toolActionPayload(call?.metadata)));

  const resultPreview = toolPayloadPreview(toolActionPayload(result?.metadata));
  const resultDisplay = toolDisplayString(result?.metadata, "result");
  if (resultPreview) {
    rows.push({
      label: result?.status === "FAILED" ? "Error" : "Output",
      value: resultPreview
    });
  } else if (resultDisplay && !isLowInformationToolDetail(resultDisplay)) {
    rows.push({ label: "Result", value: resultDisplay });
  }

  return dedupeToolDetailRows(rows);
}

export function toolHumanDetailRows(marker: ToolMarkerGroup): ToolDetailRowData[] {
  const rows: ToolDetailRowData[] = [];
  const call = marker.call?.item;
  const result = marker.result?.item;
  const callDisplay = displayFromMetadata(call?.metadata);
  const resultDisplay = displayFromMetadata(result?.metadata);
  const input = actionInput(call?.metadata, result?.metadata);
  const output = toolActionPayload(result?.metadata);
  const target = humanToolTarget(marker, input, callDisplay, resultDisplay);

  if (target && toolMarkerKind(marker) !== "web.browse") {
    rows.push(target);
  }

  const displayResult = usefulDisplayValue(resultDisplay, "result");
  if (toolMarkerStatus(marker) === "error") {
    rows.push({
      label: "What happened",
      value: humanToolError(output) ?? displayResult ?? toolMarkerSummary(marker)
    });
  } else if (displayResult && displayResult !== target?.value) {
    rows.push({ label: "Result", value: displayResult });
  } else {
    const detail = markerDisplayString(marker, "detail");
    const resultFact = detail ? { label: "Status", value: detail } : humanToolResult(output);
    if (resultFact && resultFact.value !== target?.value) {
      rows.push(resultFact);
    }
  }

  const scope = usefulDisplayValue(resultDisplay, "scope") ?? usefulDisplayValue(callDisplay, "scope");
  if (scope && scope !== target?.value) {
    rows.push({ label: "Scope", value: scope });
  }

  const purpose = usefulDisplayValue(callDisplay, "purpose");
  if (rows.length === 0 && purpose) {
    rows.push({ label: "Purpose", value: purpose });
  }

  return dedupeToolDetailRows(rows);
}

function actionInput(callMetadata: unknown, resultMetadata: unknown): unknown {
  const callAction = actionFromMetadata(callMetadata);
  const resultAction = actionFromMetadata(resultMetadata);
  if (callAction && "arguments" in callAction) {
    return callAction.arguments;
  }
  if (resultAction && "arguments" in resultAction) {
    return resultAction.arguments;
  }
  return toolActionPayload(callMetadata);
}

function humanToolTarget(
  marker: ToolMarkerGroup,
  input: unknown,
  callDisplay: Record<string, unknown> | null,
  resultDisplay: Record<string, unknown> | null
): ToolDetailRowData | null {
  const markerSubject = markerDisplayString(marker, "subject");
  if (markerSubject) {
    return {
      label: markerDisplayString(marker, "subjectLabel") ?? "Item",
      value: markerSubject
    };
  }

  const payload = isRecord(input) ? input : null;
  const title = nestedString(payload, ["task", "title"]) ?? nestedString(payload, ["project", "name"]) ?? stringValue(payload?.title);
  if (title) {
    return { label: "Item", value: title };
  }

  for (const [key, label] of [
    ["query", "Search"],
    ["path", "File"],
    ["name", "Name"]
  ] as const) {
    const value = stringValue(payload?.[key]);
    if (value) {
      return { label, value };
    }
  }

  const target = usefulDisplayValue(resultDisplay, "target") ?? usefulDisplayValue(callDisplay, "target");
  const url = target ?? stringValue(payload?.url);
  return url ? { label: "Item", value: url } : null;
}

function humanToolResult(output: unknown): ToolDetailRowData | null {
  if (!isRecord(output)) return null;
  for (const [key, noun] of [
    ["tasks", "Task"],
    ["projects", "project"],
    ["entries", "item"],
    ["results", "result"],
    ["messages", "message"],
    ["events", "event"],
    ["pages", "page"]
  ] as const) {
    const values = output[key];
    if (Array.isArray(values)) {
      return {
        label: "Result",
        value: `${values.length} ${noun}${values.length === 1 ? "" : "s"}`
      };
    }
  }
  return null;
}

function humanToolError(output: unknown): string | null {
  if (!isRecord(output)) return null;
  return stringValue(output.message) ?? stringValue(output.error) ?? nestedString(output, ["details", "message"]);
}

function usefulDisplayValue(display: Record<string, unknown> | null, key: string): string | null {
  const value = stringValue(display?.[key]);
  return value && !isLowInformationToolDetail(value) ? value : null;
}

function nestedString(value: Record<string, unknown> | null, path: readonly string[]): string | null {
  const result = path.reduce<unknown>((current, key) => (isRecord(current) ? current[key] : undefined), value);
  return stringValue(result);
}

function completeToolDetailRows(marker: ToolMarkerGroup): ToolDetailRowData[] | null {
  const callAction = actionFromMetadata(marker.call?.item.metadata);
  const resultAction = actionFromMetadata(marker.result?.item.metadata);
  if (callAction?.detail_mode !== "complete" && resultAction?.detail_mode !== "complete") {
    return null;
  }

  const rows: ToolDetailRowData[] = [];
  const correlation =
    stringValue(callAction?.correlation_id) ?? stringValue(resultAction?.correlation_id) ?? stringValue(callAction?.id) ?? stringValue(resultAction?.call_id);
  if (correlation) {
    rows.push({ label: "Correlation", value: correlation });
  }

  const input =
    callAction && "arguments" in callAction ? callAction.arguments : resultAction && "arguments" in resultAction ? resultAction.arguments : callAction?.payload;
  if (input !== undefined && !isOmittedPayload(input)) {
    rows.push({ label: "Input", value: completeToolValue(input) });
  }

  const success = resultAction?.success;
  if (typeof success === "boolean") {
    rows.push({ label: "Success", value: String(success) });
  }
  if (resultAction && "payload" in resultAction && resultAction.payload !== undefined && !isOmittedPayload(resultAction.payload)) {
    rows.push({
      label: success === false ? "Error" : "Output",
      value: completeToolValue(resultAction.payload)
    });
  }
  return rows;
}

function actionFromMetadata(metadata: unknown): Record<string, unknown> | null {
  return isRecord(metadata) && isRecord(metadata.action) ? metadata.action : null;
}

function completeToolValue(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(withoutRenderedScreenshotData(value), null, 2) ?? String(value);
  } catch {
    return String(value);
  }
}

function withoutRenderedScreenshotData(value: unknown): unknown {
  if (!isRecord(value) || !isRecord(value.screenshot) || typeof value.screenshot.data !== "string") {
    return value;
  }
  return {
    ...value,
    screenshot: {
      ...value.screenshot,
      data: "[rendered above]"
    }
  };
}

export function toolMarkerExpandable(marker: ToolMarkerGroup): boolean {
  if (marker.message) return false;
  const kind = toolMarkerKind(marker);
  if (kind === "web.search" || kind === "web.fetch") {
    return false;
  }
  return toolHumanDetailRows(marker).length > 0 || screenshotPayload(marker) !== null;
}

export function toolMarkerScreenshot(marker: ToolMarkerGroup): ToolScreenshotData | null {
  const screenshot = screenshotPayload(marker);
  return screenshot
    ? {
        src: `data:image/png;base64,${screenshot.data}`,
        width: screenshot.width,
        height: screenshot.height,
        url: browserMarkerUrl(marker)
      }
    : null;
}

function browserMarkerUrl(marker: ToolMarkerGroup): string | null {
  const result = toolActionPayload(marker.result?.item.metadata);
  const call = toolActionPayload(marker.call?.item.metadata);
  const resultRecord = isRecord(result) ? result : null;
  const callRecord = isRecord(call) ? call : null;
  return nestedString(resultRecord, ["snapshot", "url"]) ?? stringValue(resultRecord?.url) ?? stringValue(callRecord?.url);
}

function screenshotPayload(marker: ToolMarkerGroup): { data: string; width: number; height: number } | null {
  const payload = toolActionPayload(marker.result?.item.metadata);
  if (!isRecord(payload) || !isRecord(payload.screenshot)) return null;
  const { media_type: mediaType, data, width, height } = payload.screenshot;
  if (
    mediaType !== "image/png" ||
    typeof data !== "string" ||
    data.length === 0 ||
    data.length > MAX_SCREENSHOT_DATA_CHARS ||
    !validScreenshotDimension(width) ||
    !validScreenshotDimension(height)
  )
    return null;
  return { data, width, height };
}

function validScreenshotDimension(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0 && value <= 32_768;
}

function safeToolMetadataPreview(metadata: unknown): string[] {
  if (!isRecord(metadata)) {
    return [];
  }

  const display = isRecord(metadata.display) ? metadata.display : null;
  const displayRows = displayToolMetadataPreview(display);
  if (displayRows.length) {
    return displayRows;
  }

  const rows: string[] = [];
  const action = isRecord(metadata.action) ? metadata.action : null;
  const actionName = stringValue(action?.name);
  if (actionName) {
    rows.push(readableToolName(actionName));
  }
  const success = action?.success;
  if (typeof success === "boolean") {
    rows.push(`Result: ${success ? "Completed" : "Failed"}`);
  }

  return rows;
}

function toolNameFromMetadata(metadata: unknown): string | null {
  if (!isRecord(metadata)) {
    return null;
  }

  const display = metadata.display;
  if (isRecord(display) && typeof display.name === "string" && display.name.trim()) {
    return display.name;
  }
  const action = metadata.action;
  if (isRecord(action) && typeof action.name === "string" && action.name.trim()) {
    return readableToolName(action.name);
  }
  if (typeof metadata.name === "string" && metadata.name.trim()) {
    return readableToolName(metadata.name);
  }
  if (typeof metadata.tool_name === "string" && metadata.tool_name.trim()) {
    return readableToolName(metadata.tool_name);
  }
  return null;
}

function isSuccessfulWebSearchMarker(marker: ToolMarkerGroup): boolean {
  return isSuccessfulCanonicalToolResult(marker.result?.item.metadata, "web.search");
}

function isSuccessfulWebFetchMarker(marker: ToolMarkerGroup): boolean {
  return isSuccessfulCanonicalToolResult(marker.result?.item.metadata, "web.fetch");
}

function isSuccessfulCanonicalToolResult(metadata: unknown, toolName: string): boolean {
  if (!isRecord(metadata) || !isRecord(metadata.action)) {
    return false;
  }
  return actionToolNameFromMetadata(metadata) === toolName && metadata.action.success === true;
}

function webFallbackDetailRows(marker: ToolMarkerGroup): ToolDetailRowData[] {
  const display = displayFromMetadata(marker.result?.item.metadata);
  if (!display) {
    return [];
  }
  const rows: ToolDetailRowData[] = [];
  const fallbackFrom = stringValue(display.fallbackFrom);
  const fallbackReason = stringValue(display.fallbackReason);
  if (fallbackFrom) {
    rows.push({ label: "Fallback from", value: fallbackFrom });
  }
  if (fallbackReason) {
    rows.push({ label: "Fallback reason", value: fallbackReason });
  }
  return rows;
}

export function toolMarkerTarget(marker: ToolMarkerGroup): string | undefined {
  const firstPartyTarget = firstPartyToolMarkerTarget(marker);
  if (firstPartyTarget) {
    return firstPartyTarget;
  }
  if (isSuccessfulWebSearchMarker(marker) || isSuccessfulWebFetchMarker(marker)) {
    return (
      usefulToolDisplayString(marker.call?.item.metadata, "target") ??
      usefulToolDisplayString(marker.result?.item.metadata, "result") ??
      usefulToolDisplayString(marker.result?.item.metadata, "target")
    );
  }
  return (
    usefulToolDisplayString(marker.result?.item.metadata, "result") ??
    usefulToolDisplayString(marker.result?.item.metadata, "target") ??
    usefulToolDisplayString(marker.call?.item.metadata, "target")
  );
}

function firstPartyToolMarkerTarget(marker: ToolMarkerGroup): string | undefined {
  const toolName = actionToolNameFromMetadata(marker.result?.item.metadata) ?? actionToolNameFromMetadata(marker.call?.item.metadata);
  if (toolName === "web.search") {
    return (
      toolPayloadString(marker.call?.item.metadata, "query") ??
      toolPayloadString(marker.result?.item.metadata, "query") ??
      toolDisplayValue(marker.call?.item.metadata, "target", "Web search")
    );
  }
  if (toolName === "web.fetch") {
    if (marker.result?.item.status === "FAILED") {
      return undefined;
    }
    return (
      toolDisplayValue(marker.call?.item.metadata, "target", "Fetched web page") ?? toolDisplayValue(marker.result?.item.metadata, "target", "Fetched web page")
    );
  }
  if (toolName === "update_own_name") {
    return (
      toolPayloadString(marker.result?.item.metadata, "display_name") ??
      toolDisplayValue(marker.result?.item.metadata, "result", "Saved name") ??
      toolPayloadString(marker.call?.item.metadata, "name") ??
      toolDisplayValue(marker.call?.item.metadata, "target", "Name")
    );
  }
  return undefined;
}

function actionToolNameFromMetadata(metadata: unknown): string | null {
  if (!isRecord(metadata) || !isRecord(metadata.action)) {
    return null;
  }

  return typeof metadata.action.name === "string" && metadata.action.name.trim() ? metadata.action.name.trim() : null;
}

function displayToolMetadataPreview(display: Record<string, unknown> | null): string[] {
  if (!display) {
    return [];
  }
  const rows: string[] = [];
  const name = stringValue(display.name);
  if (name) {
    rows.push(name);
  }
  appendDisplayRow(rows, "Purpose", stringValue(display.purpose));
  appendDisplayRow(rows, "Access", stringValue(display.access));
  appendDisplayRow(rows, "Target", stringValue(display.target));
  appendDisplayRow(rows, "Scope", stringValue(display.scope));
  appendDisplayRow(rows, "Approval", stringValue(display.approval));
  appendDisplayRow(rows, "Provider", stringValue(display.provider));
  appendDisplayRow(rows, "Reliability", stringValue(display.reliability));
  appendDisplayRow(rows, "Fallback from", stringValue(display.fallbackFrom));
  appendDisplayRow(rows, "Fallback reason", stringValue(display.fallbackReason));
  appendDisplayRow(rows, "Result", stringValue(display.result));
  return rows;
}

function displayFromMetadata(metadata: unknown): Record<string, unknown> | null {
  if (!isRecord(metadata) || !isRecord(metadata.display)) {
    return null;
  }
  return metadata.display;
}

function appendDisplayRow(rows: string[], label: string, value: string | null) {
  if (value) {
    rows.push(`${label}: ${value}`);
  }
}

function toolActionPayload(metadata: unknown): unknown {
  if (!isRecord(metadata) || !isRecord(metadata.action)) {
    return undefined;
  }
  const payload = metadata.action.payload;
  if (isRecord(payload) && Object.keys(payload).length === 1 && "arguments" in payload) {
    return payload.arguments;
  }
  return payload;
}

function toolPayloadRows(fallbackLabel: string, payload: unknown): ToolDetailRowData[] {
  if (payload === null || payload === undefined || isEmptyJsonContainer(payload) || isOmittedPayload(payload)) {
    return [];
  }
  if (!isRecord(payload)) {
    const value = toolPayloadPreview(payload);
    return value ? [{ label: fallbackLabel, value }] : [];
  }

  const directRow = directPayloadRow(payload);
  if (directRow) {
    return [directRow];
  }

  const scalarSummary = conciseScalarSummary(payload);
  return scalarSummary ? [{ label: fallbackLabel, value: scalarSummary }] : [];
}

function directPayloadRow(payload: Record<string, unknown>): ToolDetailRowData | null {
  for (const key of ["query", "name", "url", "path", "error"]) {
    const value = stringValue(payload[key]);
    if (value) {
      return {
        label: humanPayloadLabel(key),
        value: truncateToolDetail(value)
      };
    }
  }
  return null;
}

function toolPayloadPreview(payload: unknown): string | null {
  if (payload === null || payload === undefined || isEmptyJsonContainer(payload) || isOmittedPayload(payload)) {
    return null;
  }

  const contentText = toolContentText(payload);
  if (contentText) {
    return truncateToolDetail(contentText);
  }

  if (isRecord(payload)) {
    const directRow = directPayloadRow(payload);
    if (directRow) {
      return directRow.value;
    }
    return conciseScalarSummary(payload);
  }

  return formatScalarPreview(payload);
}

function toolContentText(payload: unknown): string | null {
  if (!isRecord(payload) || !Array.isArray(payload.content)) {
    return null;
  }
  const text = payload.content
    .flatMap((item): string[] => {
      if (typeof item === "string" && item.trim()) {
        return [item.trim()];
      }
      if (isRecord(item) && typeof item.text === "string" && item.text.trim()) {
        return [item.text.trim()];
      }
      return [];
    })
    .join("\n\n")
    .trim();
  return text || null;
}

function conciseScalarSummary(value: Record<string, unknown>): string | null {
  const parts = Object.entries(value)
    .flatMap(([key, rawValue]): string[] => {
      const value = scalarPreview(rawValue);
      return value ? [`${key}: ${value}`] : [];
    })
    .slice(0, 3);
  const summary = parts.join(", ");
  return summary ? truncateToolDetail(summary) : null;
}

function formatScalarPreview(value: unknown): string | null {
  if (typeof value === "string") {
    return truncateToolDetail(value);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return null;
}

function scalarPreview(value: unknown): string | null {
  if (typeof value === "string") {
    return truncateToolDetail(value);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return null;
}

function humanPayloadLabel(key: string): string {
  switch (key) {
    case "query":
      return "Query";
    case "name":
      return "Name";
    case "url":
      return "URL";
    case "path":
      return "Path";
    case "error":
      return "Error";
    default:
      return key;
  }
}

function truncateToolDetail(value: string): string {
  const trimmed = value.trim();
  return trimmed.length > 280 ? `${trimmed.slice(0, 280)}...` : trimmed;
}

function isEmptyJsonContainer(value: unknown): boolean {
  if (Array.isArray(value)) {
    return value.length === 0;
  }
  return isRecord(value) && Object.keys(value).length === 0;
}

function isOmittedPayload(value: unknown): boolean {
  return isRecord(value) && value.omitted === true && Object.keys(value).length === 1;
}

function isLowInformationToolDetail(value: string): boolean {
  return value === "Use an enabled connected tool" || value === "Uses a connected tool" || value === "Completed" || value === "Done";
}

function dedupeToolDetailRows(rows: ToolDetailRowData[]): ToolDetailRowData[] {
  const seen = new Set<string>();
  return rows.filter((row) => {
    const key = `${row.label}\n${row.value}`;
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}

function toolDisplayString(metadata: unknown, key: string): string | undefined {
  if (!isRecord(metadata) || !isRecord(metadata.display)) {
    return undefined;
  }
  return stringValue(metadata.display[key]) ?? undefined;
}

function toolDisplayValue(metadata: unknown, key: string, legacyLabel: string): string | undefined {
  const value = toolDisplayString(metadata, key);
  if (!value) {
    return undefined;
  }
  return legacyDisplayValue(value, legacyLabel);
}

function legacyDisplayValue(value: string, legacyLabel: string): string {
  const delimiter = `${legacyLabel}:`;
  return value.startsWith(delimiter) ? value.slice(delimiter.length).trim() : value;
}

function usefulToolDisplayString(metadata: unknown, key: string): string | undefined {
  const value = toolDisplayString(metadata, key);
  return value && !isLowInformationToolDetail(value) ? value : undefined;
}

function toolPayloadString(metadata: unknown, key: string): string | undefined {
  const payload = toolActionPayload(metadata);
  if (!isRecord(payload)) {
    return undefined;
  }
  return stringValue(payload[key]) ?? undefined;
}

function readableToolName(name: string): string {
  const trimmed = name.trim();
  if (trimmed === "search_memory") {
    return "Search memory";
  }
  const lastSegment = trimmed.split(".").filter(Boolean).at(-1) ?? trimmed;
  return lastSegment
    .split(/[_-]+/g)
    .filter(Boolean)
    .map((part, index) => {
      const lower = part.toLowerCase();
      return index === 0 ? lower.charAt(0).toUpperCase() + lower.slice(1) : lower;
    })
    .join(" ");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function activityMarkerStatus(activityStatus: string, metadata: unknown, isResult: boolean): ToolMarkerCallStatus {
  const status = (markerString(metadata, "status") ?? activityStatus).toLowerCase();
  switch (status) {
    case "pending":
    case "queued":
      return "pending";
    case "started":
    case "running":
      return "running";
    case "failed":
      return "error";
    case "cancelled":
      return "cancelled";
    case "interrupted":
      return "interrupted";
    case "skipped":
      return "skipped";
    case "completed":
      return isResult ? "complete" : "pending";
    default:
      return isResult ? "complete" : "pending";
  }
}

function markerDisplay(marker: ToolMarkerGroup): Record<string, unknown> | null {
  return markerFromMetadata(marker.result?.item.metadata) ?? markerFromMetadata(marker.call?.item.metadata);
}

function markerDisplayString(marker: ToolMarkerGroup, key: string): string | undefined {
  return stringValue(markerDisplay(marker)?.[key]) ?? undefined;
}

function markerFromMetadata(metadata: unknown): Record<string, unknown> | null {
  if (!isRecord(metadata) || !isRecord(metadata.display) || !isRecord(metadata.display.marker)) {
    return null;
  }
  return metadata.display.marker;
}

function markerString(metadata: unknown, key: string): string | undefined {
  return stringValue(markerFromMetadata(metadata)?.[key]) ?? undefined;
}
