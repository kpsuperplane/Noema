import {
  renderedEntryMessageId,
  shouldAnimateRenderedEntryArrivalForSeen,
  transcriptEntryRenderId,
  type RenderTranscriptEntry
} from "./renderModel";
import type { TranscriptEntry } from "@/shared/types";

export const BOTTOM_SCROLL_THRESHOLD_PX = 80;

export function transcriptScrollKey(entries: RenderTranscriptEntry[]): string {
  return entries.map(renderedEntryScrollFingerprint).join("|");
}

export function transcriptArrivalScrollKey(
  entries: RenderTranscriptEntry[],
  seenMessageIds: ReadonlySet<string>
): string {
  return entries
    .filter((entry) => shouldAnimateRenderedEntryArrivalForSeen(entry, renderedEntryMessageId(entry), seenMessageIds))
    .map(renderedEntryMessageId)
    .join("|");
}

export function initialSeenArrivalMessageIds(entries: RenderTranscriptEntry[]): ReadonlySet<string> {
  return new Set(entries.map(renderedEntryMessageId));
}

export function isScrolledToBottom(element: HTMLElement) {
  return element.scrollHeight - element.scrollTop - element.clientHeight < BOTTOM_SCROLL_THRESHOLD_PX;
}

function renderedEntryScrollFingerprint(entry: RenderTranscriptEntry): string {
  if (entry.kind === "entry") {
    return transcriptEntryScrollFingerprint(entry.entry);
  }
  if (entry.kind === "typing") {
    return entry.id;
  }
  return [
    entry.id,
    entry.marker.call?.item.status ?? "",
    entry.marker.call?.item.summary ?? "",
    entry.marker.result?.item.status ?? "",
    entry.marker.result?.item.summary ?? ""
  ].join(":");
}

function transcriptEntryScrollFingerprint(entry: TranscriptEntry): string {
  const renderId = transcriptEntryRenderId(entry);

  if (
    entry.type === "user" ||
    entry.type === "input" ||
    entry.type === "assistant" ||
    entry.type === "assistant_stream"
  ) {
    return `${renderId}:${entry.text.length}`;
  }
  if (entry.type === "activity") {
    return `${renderId}:${entry.item.status}:${entry.item.summary ?? ""}`;
  }
  if (entry.type === "card") {
    return `${renderId}:${entry.item.schema}`;
  }
  if (entry.type === "multiple_choice_prompt") {
    return `${renderId}:${entry.item.prompt}:${entry.item.options.map((option) => option.id).join(",")}`;
  }
  if (entry.type === "multiple_choice_selection") {
    return `${renderId}:${entry.item.selected_options.map((option) => option.id).join(",")}`;
  }
  if (entry.type === "artifact") {
    return `${renderId}:${entry.item.artifact_id}:${entry.item.artifact_version_id ?? ""}`;
  }
  if (entry.type === "task") {
    return `${renderId}:${entry.item.task_id}:${entry.item.stage_id}:${entry.item.revision}`;
  }
  return `${renderId}:${entry.message.length}`;
}
