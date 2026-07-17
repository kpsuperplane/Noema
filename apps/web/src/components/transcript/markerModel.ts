import type { ToolMarkerGroup } from "./renderModel";
export type ToolDetailRowData = { label: string; value: string };

export function toolMarkerPending(marker: ToolMarkerGroup): boolean {
  return marker.call?.item.status === "STARTED" && !marker.result;
}

export function toolMarkerLabel(marker: ToolMarkerGroup): string {
  const toolName =
    firstPartyToolMarkerName(marker) ??
    toolNameFromMetadata(marker.call?.item.metadata) ??
    toolNameFromMetadata(marker.result?.item.metadata);
  if (toolName) {
    if (marker.call && !marker.result && marker.call.item.status === "STARTED") {
      return `Using ${toolName}`;
    }
    return `Used ${toolName}`;
  }
  return marker.call?.item.title ?? marker.result?.item.title ?? "Tool activity";
}

export function toolMarkerName(marker: ToolMarkerGroup): string {
  const firstPartyName = firstPartyToolMarkerName(marker);
  if (firstPartyName) {
    return firstPartyName;
  }
  return (
    toolNameFromMetadata(marker.call?.item.metadata) ??
    toolNameFromMetadata(marker.result?.item.metadata) ??
    marker.call?.item.title ??
    marker.result?.item.title ??
    "Tool activity"
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
  const completeRows = completeToolDetailRows(marker);
  if (completeRows) {
    return completeRows;
  }
  if (isSuccessfulWebSearchMarker(marker) || isSuccessfulWebFetchMarker(marker)) {
    return webFallbackDetailRows(marker);
  }

  const rows: ToolDetailRowData[] = [];
  const call = marker.call?.item;
  const result = marker.result?.item;

  rows.push(...toolPayloadRows("Input", toolActionPayload(call?.metadata)));

  const resultPreview = toolPayloadPreview(toolActionPayload(result?.metadata));
  const resultDisplay = toolDisplayString(result?.metadata, "result");
  if (resultPreview) {
    rows.push({ label: result?.status === "FAILED" ? "Error" : "Output", value: resultPreview });
  } else if (resultDisplay && !isLowInformationToolDetail(resultDisplay)) {
    rows.push({ label: "Result", value: resultDisplay });
  }

  return dedupeToolDetailRows(rows);
}

function completeToolDetailRows(marker: ToolMarkerGroup): ToolDetailRowData[] | null {
  const callAction = actionFromMetadata(marker.call?.item.metadata);
  const resultAction = actionFromMetadata(marker.result?.item.metadata);
  if (callAction?.detail_mode !== "complete" && resultAction?.detail_mode !== "complete") {
    return null;
  }

  const rows: ToolDetailRowData[] = [];
  const correlation = stringValue(callAction?.correlation_id) ??
    stringValue(resultAction?.correlation_id) ??
    stringValue(callAction?.id) ??
    stringValue(resultAction?.call_id);
  if (correlation) {
    rows.push({ label: "Correlation", value: correlation });
  }

  const input = callAction && "arguments" in callAction
    ? callAction.arguments
    : resultAction && "arguments" in resultAction
      ? resultAction.arguments
      : callAction?.payload;
  if (input !== undefined) {
    rows.push({ label: "Input", value: completeToolValue(input) });
  }

  const success = resultAction?.success;
  if (typeof success === "boolean") {
    rows.push({ label: "Success", value: success ? "true" : "false" });
  }
  if (resultAction && "payload" in resultAction && resultAction.payload !== undefined) {
    rows.push({ label: success === false ? "Error" : "Output", value: completeToolValue(resultAction.payload) });
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
    return JSON.stringify(value, null, 2) ?? String(value);
  } catch {
    return String(value);
  }
}

export function toolMarkerExpandable(marker: ToolMarkerGroup): boolean {
  return toolDetailRows(marker).length > 0;
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

function firstPartyToolMarkerName(marker: ToolMarkerGroup): string | undefined {
  const toolName = actionToolNameFromMetadata(marker.result?.item.metadata) ?? actionToolNameFromMetadata(marker.call?.item.metadata);
  if (!toolName) {
    return undefined;
  }
  if (toolName === "update_own_name" && marker.result?.item.status === "COMPLETED") {
    return "Saved name";
  }
  return firstPartyReadableToolName(toolName) ?? undefined;
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
      toolDisplayValue(marker.call?.item.metadata, "target", "Fetched web page") ??
      toolDisplayValue(marker.result?.item.metadata, "target", "Fetched web page")
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

function firstPartyReadableToolName(name: string): string | null {
  const trimmed = name.trim();
  if (trimmed === "web.search") {
    return "Web Search";
  }
  if (trimmed === "web.fetch") {
    return "Fetched Web Page";
  }
  if (trimmed === "update_own_name") {
    return "Save name";
  }
  return null;
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
  if (payload === null || payload === undefined || isEmptyJsonContainer(payload)) {
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
      return { label: humanPayloadLabel(key), value: truncateToolDetail(value) };
    }
  }
  return null;
}

function toolPayloadPreview(payload: unknown): string | null {
  if (payload === null || payload === undefined || isEmptyJsonContainer(payload)) {
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

function isLowInformationToolDetail(value: string): boolean {
  return (
    value === "Use an enabled connected tool" ||
    value === "Uses a connected tool" ||
    value === "Completed" ||
    value === "Done"
  );
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
  const firstPartyName = firstPartyReadableToolName(trimmed);
  if (firstPartyName) {
    return firstPartyName;
  }
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
