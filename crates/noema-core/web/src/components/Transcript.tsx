import {
  Attachment,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle
} from "@/components/ui/attachment";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { Button } from "@/components/ui/button";
import { Marker, MarkerContent, MarkerIcon } from "@/components/ui/marker";
import { ErrorMarker } from "./ErrorMarker";
import { AnimatedMessageText } from "./MessageTextAnimation";
import {
  Message as MessagePrimitive,
  MessageAvatar,
  MessageContent
} from "@/components/ui/message";
import {
  MessageScroller,
  MessageScrollerButton,
  MessageScrollerContent,
  MessageScrollerItem,
  MessageScrollerProvider,
  MessageScrollerViewport,
  useMessageScroller
} from "@/components/ui/message-scroller";
import { cn } from "@/lib/utils";
import { BrainIcon, WrenchIcon } from "lucide-react";
import * as React from "react";
import { readableKind, statusLabel } from "../format";
import { memoryCardsFromStructuredItem, type MemoryCardData } from "../memoryCards";
import type { TranscriptEntry, TurnTranscriptItem } from "../types";

type ActivityTranscriptEntry = Extract<TranscriptEntry, { type: "activity" }>;
type ActivityTranscriptItem = Extract<TurnTranscriptItem, { kind: "activity" }>;

type ToolMarkerGroup = {
  id: string;
  call?: ActivityTranscriptEntry;
  result?: ActivityTranscriptEntry;
};

type RenderTranscriptEntry =
  | { kind: "entry"; id: string; entry: TranscriptEntry }
  | { kind: "typing"; id: string }
  | {
      kind: "memory_marker";
      id: string;
      extraction?: ActivityTranscriptItem;
      proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
    }
  | { kind: "tool_marker"; id: string; marker: ToolMarkerGroup };

type TranscriptLane = "human" | "assistant";

type MemoryDetailRowData = { label: string; value: string };

export type MemoryDetailItem = {
  title: string;
  rows: MemoryDetailRowData[];
};

type RenderTranscriptLaneCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" }
  | { kind: "tool_marker" };

export function Transcript({
  entries,
  pending,
  expandedActivities,
  onToggleActivity
}: {
  entries: TranscriptEntry[];
  pending: boolean;
  expandedActivities: Set<string>;
  onToggleActivity: (id: string) => void;
}) {
  const renderedEntries = renderableTranscriptEntries(entries, pending);
  const followBottomRef = React.useRef(true);
  const scrollKey = transcriptScrollKey(renderedEntries);
  const handleViewportScroll = React.useCallback((event: React.UIEvent<HTMLDivElement>) => {
    followBottomRef.current = isScrolledToBottom(event.currentTarget);
  }, []);

  return (
    <MessageScrollerProvider autoScroll defaultScrollPosition="end" scrollPreviousItemPeek={56}>
      <MessageScroller className="min-h-0 overflow-hidden">
        <MessageScrollerViewport aria-label="Conversation transcript" onScroll={handleViewportScroll}>
          <MessageScrollerContent className="mx-auto flex min-h-full w-[var(--chat-column-width)] flex-col gap-3 px-0.5 py-6">
            {renderedEntries.map((entry, index) => {
              const lane =
                entry.kind === "entry"
                  ? renderedTranscriptLane({ kind: "entry", entryType: entry.entry.type })
                  : renderedTranscriptLane({ kind: entry.kind });
              const previousEntry = renderedEntries[index - 1];
              const previousLane =
                previousEntry && previousEntry.kind === "entry"
                  ? renderedTranscriptLane({ kind: "entry", entryType: previousEntry.entry.type })
                  : previousEntry
                    ? renderedTranscriptLane({ kind: previousEntry.kind })
                    : null;
              const showAvatar = previousLane !== lane;
              const messageId = renderedEntryMessageId(entry);

              return (
                <MessageScrollerItem
                  key={messageId}
                  className={cn("flex w-full", lane === "human" && "justify-end")}
                  messageId={messageId}
                  scrollAnchor={shouldAnchorRenderedEntry(entry)}
                >
                  {renderTranscriptRenderEntry(entry, expandedActivities, onToggleActivity, showAvatar)}
                </MessageScrollerItem>
              );
            })}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
      <TranscriptBottomFollower followBottomRef={followBottomRef} scrollKey={scrollKey} />
    </MessageScrollerProvider>
  );
}

function renderableTranscriptEntries(entries: TranscriptEntry[], pending: boolean): RenderTranscriptEntry[] {
  const renderedEntries = groupTranscriptMarkers(entries);
  if (shouldShowTypingIndicator(entries, pending)) {
    renderedEntries.push({ kind: "typing", id: "typing-indicator" });
  }
  return renderedEntries;
}

type TranscriptEntryAnchorCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" }
  | { kind: "tool_marker" };

export function shouldAnchorTranscriptEntry(entry: TranscriptEntryAnchorCandidate): boolean {
  void entry;
  return false;
}

function shouldAnchorRenderedEntry(entry: RenderTranscriptEntry): boolean {
  if (entry.kind === "entry") {
    return shouldAnchorTranscriptEntry({ kind: "entry", entryType: entry.entry.type });
  }
  return shouldAnchorTranscriptEntry({ kind: entry.kind });
}

export function transcriptEntryLane(entryType: TranscriptEntry["type"]): TranscriptLane {
  return entryType === "user" ? "human" : "assistant";
}

export function renderedTranscriptLane(entry: RenderTranscriptLaneCandidate): TranscriptLane {
  if (entry.kind === "entry") {
    return transcriptEntryLane(entry.entryType);
  }
  return "assistant";
}

export function shouldShowTypingIndicator(entries: TranscriptEntry[], pending: boolean): boolean {
  if (!pending) {
    return false;
  }

  let lastUserIndex = -1;
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    if (entries[index]?.type === "user") {
      lastUserIndex = index;
      break;
    }
  }
  if (lastUserIndex === -1) {
    return false;
  }

  return !entries
    .slice(lastUserIndex + 1)
    .some((entry) => entry.type === "assistant" || entry.type === "assistant_stream");
}

function groupTranscriptMarkers(entries: TranscriptEntry[]): RenderTranscriptEntry[] {
  const rendered: RenderTranscriptEntry[] = [];

  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index];
    const nextEntry = entries[index + 1];
    const followingEntry = entries[index + 2];

    if (
      entry.type === "activity" &&
      entry.item.activity_kind === "memory_extraction" &&
      entry.item.status === "STARTED" &&
      nextEntry &&
      nextEntry.type === "card" &&
      nextEntry.item.schema === "memory_proposals" &&
      followingEntry &&
      followingEntry.type === "activity" &&
      followingEntry.item.activity_kind === "memory_extraction" &&
      followingEntry.item.id === entry.item.id &&
      sameTurn(entry, nextEntry) &&
      sameTurn(entry, followingEntry)
    ) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}:${followingEntry.id}`,
        extraction: followingEntry.item,
        proposal: nextEntry.item
      });
      index += 2;
      continue;
    }

    if (
      entry.type === "activity" &&
      entry.item.activity_kind === "memory_extraction" &&
      nextEntry &&
      nextEntry.type === "card" &&
      nextEntry.item.schema === "memory_proposals" &&
      sameTurn(entry, nextEntry)
    ) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}`,
        extraction: entry.item,
        proposal: nextEntry.item
      });
      index += 1;
      continue;
    }

    if (
      entry.type === "card" &&
      entry.item.schema === "memory_proposals" &&
      nextEntry &&
      nextEntry.type === "activity" &&
      nextEntry.item.activity_kind === "memory_extraction" &&
      sameTurn(entry, nextEntry)
    ) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}`,
        extraction: nextEntry.item,
        proposal: entry.item
      });
      index += 1;
      continue;
    }

    if (entry.type === "activity" && entry.item.activity_kind === "memory_extraction") {
      rendered.push({
        kind: "memory_marker",
        id: entry.id,
        extraction: entry.item
      });
      continue;
    }

    if (entry.type === "card" && entry.item.schema === "memory_proposals") {
      rendered.push({
        kind: "memory_marker",
        id: entry.id,
        proposal: entry.item
      });
      continue;
    }

    if (entry.type === "activity" && entry.item.activity_kind === "tool_call") {
      if (
        nextEntry &&
        nextEntry.type === "activity" &&
        nextEntry.item.activity_kind === "tool_result" &&
        sameTurn(entry, nextEntry)
      ) {
        const id = `${entry.id}:${nextEntry.id}`;
        rendered.push({
          kind: "tool_marker",
          id,
          marker: { id, call: entry, result: nextEntry }
        });
        index += 1;
        continue;
      }

      rendered.push({
        kind: "tool_marker",
        id: entry.id,
        marker: { id: entry.id, call: entry }
      });
      continue;
    }

    if (entry.type === "activity" && entry.item.activity_kind === "tool_result") {
      rendered.push({
        kind: "tool_marker",
        id: entry.id,
        marker: { id: entry.id, result: entry }
      });
      continue;
    }

    rendered.push({ kind: "entry", id: entry.id, entry });
  }

  return rendered;
}

function sameTurn(left: TranscriptEntry, right: TranscriptEntry) {
  return !left.turnId || !right.turnId || left.turnId === right.turnId;
}

function toolMarkerTone(marker: ToolMarkerGroup): "default" | "error" {
  return marker.result?.item.status === "FAILED" ? "error" : "default";
}

function toolMarkerLabel(marker: ToolMarkerGroup): string {
  const toolName = toolNameFromMetadata(marker.call?.item.metadata) ?? toolNameFromMetadata(marker.result?.item.metadata);
  if (toolName) {
    return `Used ${toolName}`;
  }
  return marker.call?.item.title ?? marker.result?.item.title ?? "Tool activity";
}

function toolNameFromMetadata(metadata: unknown): string | null {
  if (!isRecord(metadata)) {
    return null;
  }

  const action = metadata.action;
  if (isRecord(action) && typeof action.name === "string" && action.name.trim()) {
    return action.name;
  }
  if (typeof metadata.name === "string" && metadata.name.trim()) {
    return metadata.name;
  }
  if (typeof metadata.tool_name === "string" && metadata.tool_name.trim()) {
    return metadata.tool_name;
  }
  return null;
}

function formatToolDetail(fallback: string, metadata: unknown): string {
  const metadataText = formatMetadata(metadata);
  if (!metadataText) {
    return fallback;
  }
  return `${fallback}\n${metadataText}`;
}

function formatMetadata(metadata: unknown): string | null {
  if (metadata === null || metadata === undefined) {
    return null;
  }
  if (typeof metadata === "string") {
    return metadata;
  }
  try {
    return JSON.stringify(metadata, null, 2);
  } catch {
    return String(metadata);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function transcriptScrollKey(entries: RenderTranscriptEntry[]): string {
  return entries.map(renderedEntryScrollFingerprint).join("|");
}

function renderedEntryScrollFingerprint(entry: RenderTranscriptEntry): string {
  if (entry.kind === "entry") {
    return transcriptEntryScrollFingerprint(entry.entry);
  }
  if (entry.kind === "typing") {
    return entry.id;
  }
  if (entry.kind === "memory_marker") {
    return [
      entry.id,
      entry.extraction?.status ?? "",
      entry.extraction?.summary ?? "",
      entry.proposal?.id ?? ""
    ].join(":");
  }
  return [
    entry.id,
    entry.marker.call?.item.status ?? "",
    entry.marker.call?.item.summary ?? "",
    entry.marker.result?.item.status ?? "",
    entry.marker.result?.item.summary ?? ""
  ].join(":");
}

function renderedEntryMessageId(entry: RenderTranscriptEntry): string {
  if (entry.kind === "entry") {
    return transcriptEntryRenderId(entry.entry);
  }
  return entry.id;
}

function transcriptEntryRenderId(entry: TranscriptEntry): string {
  if ((entry.type === "assistant" || entry.type === "assistant_stream") && entry.streamId) {
    return entry.streamId;
  }
  return entry.id;
}

function transcriptEntryScrollFingerprint(entry: TranscriptEntry): string {
  const renderId = transcriptEntryRenderId(entry);

  if (entry.type === "user" || entry.type === "assistant" || entry.type === "assistant_stream") {
    return `${renderId}:${entry.text.length}`;
  }
  if (entry.type === "activity") {
    return `${renderId}:${entry.item.status}:${entry.item.summary ?? ""}`;
  }
  if (entry.type === "card") {
    return `${renderId}:${entry.item.schema}`;
  }
  return `${renderId}:${entry.message.length}`;
}

function isScrolledToBottom(element: HTMLElement) {
  return element.scrollHeight - element.scrollTop - element.clientHeight <= 8;
}

function TranscriptBottomFollower({
  followBottomRef,
  scrollKey
}: {
  followBottomRef: React.MutableRefObject<boolean>;
  scrollKey: string;
}) {
  const { scrollToEnd } = useMessageScroller();

  React.useLayoutEffect(() => {
    if (followBottomRef.current) {
      scrollToEnd({ behavior: "auto" });
    }
  }, [followBottomRef, scrollKey, scrollToEnd]);

  return null;
}

function renderTranscriptRenderEntry(
  entry: RenderTranscriptEntry,
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void,
  showAvatar: boolean
) {
  if (entry.kind === "memory_marker") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <MemoryMarker
          id={entry.id}
          extraction={entry.extraction}
          proposal={entry.proposal}
          open={expandedActivities.has(entry.id)}
          onToggle={() => onToggleActivity(entry.id)}
        />
      </TranscriptRow>
    );
  }
  if (entry.kind === "tool_marker") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <ToolMarker
          marker={entry.marker}
          open={expandedActivities.has(entry.id)}
          onToggle={() => onToggleActivity(entry.id)}
        />
      </TranscriptRow>
    );
  }
  if (entry.kind === "typing") {
    return <TypingMessage showAvatar={showAvatar} />;
  }
  return renderTranscriptEntry(entry.entry, expandedActivities, onToggleActivity, showAvatar);
}

function renderTranscriptEntry(
  entry: TranscriptEntry,
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void,
  showAvatar: boolean
) {
  if (entry.type === "user") {
    return <Message animate={shouldAnimateMessageText(entry)} role="user" text={entry.text} showAvatar={showAvatar} />;
  }
  if (entry.type === "assistant") {
    return <Message animate={shouldAnimateMessageText(entry)} role="assistant" text={entry.text} showAvatar={showAvatar} />;
  }
  if (entry.type === "assistant_stream") {
    return <Message animate={shouldAnimateMessageText(entry)} role="assistant" text={entry.text} showAvatar={showAvatar} />;
  }
  if (entry.type === "activity") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <ActivityRow item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
      </TranscriptRow>
    );
  }
  if (entry.type === "card") {
    return (
      <TranscriptRow lane="assistant" showAvatar={showAvatar}>
        <StructuredCard item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
      </TranscriptRow>
    );
  }
  return (
    <TranscriptRow lane="assistant" showAvatar={showAvatar}>
      <ErrorNotice message={entry.message} recoverable={entry.recoverable} />
    </TranscriptRow>
  );
}

export function shouldAnimateMessageText(entry: Extract<TranscriptEntry, { text: string }>): boolean {
  return entry.type !== "user" && entry.source !== "replay";
}

function TranscriptRow({
  lane,
  showAvatar = true,
  children
}: {
  lane: TranscriptLane;
  showAvatar?: boolean;
  children: React.ReactNode;
}) {
  const role = lane === "human" ? "user" : "assistant";

  return (
    <MessagePrimitive align={lane === "human" ? "end" : "start"} className="max-w-[760px]">
      <MessageAvatar aria-hidden={!showAvatar} className={cn(!showAvatar && "invisible")}>
        <Avatar size="sm">
          <AvatarFallback>{role === "user" ? "ME" : "N"}</AvatarFallback>
        </Avatar>
      </MessageAvatar>
      <MessageContent>{children}</MessageContent>
    </MessagePrimitive>
  );
}

function Message({
  animate,
  role,
  text,
  showAvatar
}: {
  animate: boolean;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
}) {
  return (
    <TranscriptRow lane={role === "user" ? "human" : "assistant"} showAvatar={showAvatar}>
      <Bubble variant={role === "user" ? "default" : "muted"}>
        <BubbleContent className="leading-[1.7] whitespace-pre-wrap">
          <AnimatedMessageText animate={animate} text={text} />
        </BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}

function TypingMessage({ showAvatar }: { showAvatar: boolean }) {
  return (
    <TranscriptRow lane="assistant" showAvatar={showAvatar}>
      <Bubble variant="muted">
        <BubbleContent
          className="flex min-h-9 w-[58px] items-center justify-center gap-1.5 px-3 py-2"
          aria-label="Noema is typing"
          role="status"
        >
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.24s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70 [animation-delay:-0.12s]" />
          <span className="size-1.5 animate-bounce rounded-full bg-muted-foreground/70" />
        </BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}

function ActivityRow({
  item,
  open,
  onToggle
}: {
  item: Extract<TurnTranscriptItem, { kind: "activity" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const isMemorySave = item.activity_kind === "memory_save";
  const title = isMemorySave ? "Memory saved" : item.title;
  const status = statusLabel(item.status);

  return (
    <Attachment className="w-full max-w-full">
      <Button
        type="button"
        variant="ghost"
        className="grid h-auto w-full grid-cols-[28px_minmax(0,1fr)_auto] items-center gap-2.5 px-[13px] py-[11px] text-left whitespace-normal text-foreground"
        onClick={onToggle}
        aria-expanded={open}
      >
        <AttachmentMedia
          className={cn(
            "size-7 font-mono text-xs font-bold text-[var(--pine-700)]",
            item.status === "FAILED" && "text-[var(--red-700)]",
            item.status === "STARTED" && "text-[var(--blue-700)]"
          )}
        >
          {isMemorySave ? "M" : "A"}
        </AttachmentMedia>
        <span>
          <AttachmentTitle>{title}</AttachmentTitle>
          {item.summary ? <AttachmentDescription>{item.summary}</AttachmentDescription> : null}
        </span>
        <em className="font-mono text-[11px] text-muted-foreground not-italic">{status}</em>
      </Button>
      {open ? (
        <AttachmentContent className="px-[13px] pb-[13px] pl-[51px] max-[760px]:pl-[13px]">
          <dl className="m-0 grid gap-[9px] border-t border-[var(--border-subtle)] pt-2.5">
            <div className="grid gap-0.5">
              <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">What happened</dt>
              <dd className="m-0 text-[13px] text-muted-foreground">{item.summary || title}</dd>
            </div>
            <div className="grid gap-0.5">
              <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">Source</dt>
              <dd className="m-0 text-[13px] text-muted-foreground">
                {isMemorySave ? "You used an explicit remember request in this chat." : readableKind(item.activity_kind)}
              </dd>
            </div>
            <div className="grid gap-0.5">
              <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">Next step</dt>
              <dd className="m-0 text-[13px] text-muted-foreground">
                {isMemorySave ? "Continue chatting. Deeper memory settings come in the next slice." : "No action needed."}
              </dd>
            </div>
          </dl>
        </AttachmentContent>
      ) : null}
    </Attachment>
  );
}

function StructuredCard({
  item,
  open,
  onToggle
}: {
  item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
  open: boolean;
  onToggle: () => void;
}) {
  if (item.schema === "memory_proposals") {
    return <MemoryMarker id={item.id} proposal={item} open={open} onToggle={onToggle} />;
  }

  const memories = memoryCardsFromStructuredItem(item);
  if (memories) {
    return <MemoryStructuredCard schema={item.schema} memories={memories} />;
  }

  return (
    <Attachment className="max-w-full">
      <AttachmentContent>
        <AttachmentTitle>{item.schema}</AttachmentTitle>
        <AttachmentDescription>Structured card placeholder</AttachmentDescription>
      </AttachmentContent>
    </Attachment>
  );
}

function MemoryMarker({
  id,
  extraction,
  proposal,
  open,
  onToggle
}: {
  id: string;
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
  proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const memories = proposal ? memoryCardsFromStructuredItem(proposal) ?? [] : [];
  const failed = extraction?.status === "FAILED";
  const started = extraction?.status === "STARTED";
  const label = failed ? "Memory update failed" : started ? "Memory proposed" : "Memory updated";
  const tone = failed ? "error" : started ? "default" : "success";

  return (
    <div className="grid w-full max-w-full gap-2">
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${id}-details`}
        onClick={onToggle}
        tone={tone}
        className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
      >
        <MarkerIcon>
          <BrainIcon />
        </MarkerIcon>
        <MarkerContent>{label}</MarkerContent>
      </Marker>
      {open ? (
        <MemoryDetailAttachment id={`${id}-details`} extraction={extraction} memories={memories} failed={failed} />
      ) : null}
    </div>
  );
}

function ToolMarker({
  marker,
  open,
  onToggle
}: {
  marker: ToolMarkerGroup;
  open: boolean;
  onToggle: () => void;
}) {
  const tone = toolMarkerTone(marker);

  return (
    <div className="grid w-full max-w-full gap-2">
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${marker.id}-details`}
        onClick={onToggle}
        tone={tone}
        className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
      >
        <MarkerIcon>
          <WrenchIcon />
        </MarkerIcon>
        <MarkerContent>{toolMarkerLabel(marker)}</MarkerContent>
      </Marker>
      {open ? <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} /> : null}
    </div>
  );
}

function ToolDetailAttachment({ id, marker }: { id: string; marker: ToolMarkerGroup }) {
  const result = marker.result?.item;
  const call = marker.call?.item;
  const failed = result?.status === "FAILED";
  const title = toolMarkerLabel(marker);
  const description = result?.summary ?? call?.summary ?? statusLabel(result?.status ?? call?.status ?? "COMPLETED");

  return (
    <Attachment id={id} state={failed ? "error" : "done"} className="max-w-full">
      <AttachmentMedia className={failed ? "text-[var(--red-700)]" : "text-[var(--blue-700)]"}>
        <WrenchIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{description}</AttachmentDescription>
        <dl className="mt-3 grid gap-2 border-t border-[var(--border-subtle)] pt-2.5">
          {call ? <ToolDetailRow label="Call" value={formatToolDetail(call.title, call.metadata)} /> : null}
          {result ? (
            <ToolDetailRow label="Result" value={formatToolDetail(result.summary ?? result.title, result.metadata)} />
          ) : null}
          <ToolDetailRow label="Status" value={statusLabel(result?.status ?? call?.status ?? "COMPLETED")} />
        </dl>
      </AttachmentContent>
    </Attachment>
  );
}

function ToolDetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-0.5">
      <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">{label}</dt>
      <dd className="m-0 max-h-40 overflow-auto whitespace-pre-wrap break-words text-[13px] text-muted-foreground">
        {value}
      </dd>
    </div>
  );
}

function MemoryStructuredCard({ schema, memories }: { schema: string; memories: MemoryCardData[] }) {
  const count = memories.length;
  const title = count === 1 ? "Memory saved" : `${count} memories saved`;
  const source = schema === "memory_proposals" ? "Same-call proposal" : "Explicit request";

  return (
    <Attachment className="max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{source}</AttachmentDescription>
        <MemoryDetailList memories={memories} />
      </AttachmentContent>
    </Attachment>
  );
}

export function memoryDetailItems(memories: MemoryCardData[]): MemoryDetailItem[] {
  return memories.map((memory) => {
    const rows: MemoryDetailRowData[] = [{ label: "Memory", value: memory.content }];
    if (memory.memoryType) {
      rows.push({ label: "Type", value: memory.memoryType });
    }
    if (memory.sensitivity) {
      rows.push({ label: "Sensitivity", value: memory.sensitivity });
    }
    if (memory.status) {
      rows.push({ label: "Status", value: memory.status });
    }
    if (typeof memory.confidence === "number") {
      rows.push({ label: "Confidence", value: `${Math.round(memory.confidence * 100)}%` });
    }
    if (memory.evidenceExcerpt) {
      rows.push({ label: "Evidence", value: memory.evidenceExcerpt });
    }
    if (memory.id) {
      rows.push({ label: "Memory ID", value: memory.id });
    }
    return { title: memory.title, rows };
  });
}

function MemoryDetailAttachment({
  id,
  extraction,
  memories,
  failed
}: {
  id: string;
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
  memories: MemoryCardData[];
  failed: boolean;
}) {
  const memoryCount = memories.length;
  const title = failed ? "Memory update failed" : memoryCount === 1 ? "Memory saved" : `${memoryCount} memories saved`;
  const status = extraction ? statusLabel(extraction.status) : null;
  const description = extraction?.summary ?? (status ? `Memory extraction ${status.toLowerCase()}` : "Memory proposal");

  return (
    <Attachment id={id} state={failed ? "error" : "done"} className="max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{description}</AttachmentDescription>
        <MemoryDetailList memories={memories} />
      </AttachmentContent>
    </Attachment>
  );
}

function MemoryDetailList({ memories }: { memories: MemoryCardData[] }) {
  const items = memoryDetailItems(memories);
  if (items.length === 0) {
    return null;
  }

  return (
    <div className="mt-3 grid gap-3 border-t border-[var(--border-subtle)] pt-2.5">
      {items.map((item, index) => (
        <section className="grid gap-2" key={`${item.title}:${index}`}>
          {items.length > 1 ? (
            <p className="m-0 text-[13px] font-medium text-foreground [overflow-wrap:anywhere]">{item.title}</p>
          ) : null}
          <dl className="grid gap-2">
            {item.rows.map((row) => (
              <MemoryDetailRow key={row.label} label={row.label} value={row.value} />
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}

function MemoryDetailRow({ label, value }: MemoryDetailRowData) {
  return (
    <div className="grid gap-0.5">
      <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">{label}</dt>
      <dd className="m-0 max-h-40 overflow-auto whitespace-pre-wrap break-words text-[13px] text-muted-foreground">
        {value}
      </dd>
    </div>
  );
}

function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return <ErrorMarker message={message} label={recoverable ? "Notice" : "Error"} recoverable={recoverable} />;
}
