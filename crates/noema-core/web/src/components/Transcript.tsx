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
  MessageScrollerViewport
} from "@/components/ui/message-scroller";
import { cn } from "@/lib/utils";
import { BrainIcon } from "lucide-react";
import type * as React from "react";
import { readableKind, statusLabel } from "../format";
import { memoryCardsFromStructuredItem, type MemoryCardData } from "../memoryCards";
import type { TranscriptEntry, TurnTranscriptItem } from "../types";

type RenderTranscriptEntry =
  | { kind: "entry"; id: string; entry: TranscriptEntry }
  | { kind: "typing"; id: string }
  | {
      kind: "memory_marker";
      id: string;
      extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
      proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
    };

type TranscriptLane = "human" | "assistant";

type RenderTranscriptLaneCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" };

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

  return (
    <MessageScrollerProvider autoScroll defaultScrollPosition="end" scrollPreviousItemPeek={56}>
      <MessageScroller className="min-h-0 overflow-hidden">
        <MessageScrollerViewport aria-label="Conversation transcript">
          <MessageScrollerContent className="mx-auto flex min-h-full w-[var(--chat-column-width)] flex-col gap-3 px-0.5 py-6">
            {renderedEntries.map((entry) => {
              const lane =
                entry.kind === "entry"
                  ? renderedTranscriptLane({ kind: "entry", entryType: entry.entry.type })
                  : renderedTranscriptLane({ kind: entry.kind });

              return (
                <MessageScrollerItem
                  key={entry.id}
                  className={cn("flex w-full", lane === "human" && "justify-end")}
                  messageId={entry.id}
                  scrollAnchor={shouldAnchorRenderedEntry(entry)}
                >
                  {renderTranscriptRenderEntry(entry, expandedActivities, onToggleActivity)}
                </MessageScrollerItem>
              );
            })}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
    </MessageScrollerProvider>
  );
}

function renderableTranscriptEntries(entries: TranscriptEntry[], pending: boolean): RenderTranscriptEntry[] {
  const renderedEntries = groupMemoryMarkers(entries);
  if (shouldShowTypingIndicator(entries, pending)) {
    renderedEntries.push({ kind: "typing", id: "typing-indicator" });
  }
  return renderedEntries;
}

type TranscriptEntryAnchorCandidate =
  | { kind: "entry"; entryType: TranscriptEntry["type"] }
  | { kind: "typing" }
  | { kind: "memory_marker" };

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

  return !entries.slice(lastUserIndex + 1).some((entry) => entry.type === "assistant");
}

function groupMemoryMarkers(entries: TranscriptEntry[]): RenderTranscriptEntry[] {
  const rendered: RenderTranscriptEntry[] = [];

  for (let index = 0; index < entries.length; index += 1) {
    const entry = entries[index];
    const nextEntry = entries[index + 1];

    if (isMemoryExtractionEntry(entry) && nextEntry && isMemoryProposalEntry(nextEntry) && sameTurn(entry, nextEntry)) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}`,
        extraction: entry.item,
        proposal: nextEntry.item
      });
      index += 1;
      continue;
    }

    if (isMemoryProposalEntry(entry) && nextEntry && isMemoryExtractionEntry(nextEntry) && sameTurn(entry, nextEntry)) {
      rendered.push({
        kind: "memory_marker",
        id: `${entry.id}:${nextEntry.id}`,
        extraction: nextEntry.item,
        proposal: entry.item
      });
      index += 1;
      continue;
    }

    if (isMemoryExtractionEntry(entry)) {
      rendered.push({
        kind: "memory_marker",
        id: entry.id,
        extraction: entry.item
      });
      continue;
    }

    if (isMemoryProposalEntry(entry)) {
      rendered.push({
        kind: "memory_marker",
        id: entry.id,
        proposal: entry.item
      });
      continue;
    }

    rendered.push({ kind: "entry", id: entry.id, entry });
  }

  return rendered;
}

function isMemoryExtractionEntry(
  entry: TranscriptEntry
): entry is Extract<TranscriptEntry, { type: "activity" }> {
  return entry.type === "activity" && entry.item.activity_kind === "memory_extraction";
}

function isMemoryProposalEntry(entry: TranscriptEntry): entry is Extract<TranscriptEntry, { type: "card" }> {
  return entry.type === "card" && entry.item.schema === "memory_proposals";
}

function sameTurn(left: TranscriptEntry, right: TranscriptEntry) {
  return !left.turnId || !right.turnId || left.turnId === right.turnId;
}

function renderTranscriptRenderEntry(
  entry: RenderTranscriptEntry,
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void
) {
  if (entry.kind === "memory_marker") {
    return (
      <TranscriptRow lane="assistant">
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
  if (entry.kind === "typing") {
    return <TypingMessage />;
  }
  return renderTranscriptEntry(entry.entry, expandedActivities, onToggleActivity);
}

function renderTranscriptEntry(
  entry: TranscriptEntry,
  expandedActivities: Set<string>,
  onToggleActivity: (id: string) => void
) {
  if (entry.type === "user") {
    return <Message role="user" text={entry.text} />;
  }
  if (entry.type === "assistant") {
    return <Message role="assistant" text={entry.text} />;
  }
  if (entry.type === "activity") {
    return (
      <TranscriptRow lane="assistant">
        <ActivityRow item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
      </TranscriptRow>
    );
  }
  if (entry.type === "card") {
    return (
      <TranscriptRow lane="assistant">
        <StructuredCard item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
      </TranscriptRow>
    );
  }
  return (
    <TranscriptRow lane="assistant">
      <ErrorNotice message={entry.message} recoverable={entry.recoverable} />
    </TranscriptRow>
  );
}

function TranscriptRow({ lane, children }: { lane: TranscriptLane; children: React.ReactNode }) {
  const role = lane === "human" ? "user" : "assistant";

  return (
    <MessagePrimitive align={lane === "human" ? "end" : "start"} className="max-w-[760px]">
      <MessageAvatar>
        <Avatar size="sm">
          <AvatarFallback>{role === "user" ? "ME" : "N"}</AvatarFallback>
        </Avatar>
      </MessageAvatar>
      <MessageContent>{children}</MessageContent>
    </MessagePrimitive>
  );
}

function Message({ role, text }: { role: "user" | "assistant"; text: string }) {
  return (
    <TranscriptRow lane={role === "user" ? "human" : "assistant"}>
      <Bubble variant={role === "user" ? "default" : "muted"}>
        <BubbleContent className="leading-[1.7] whitespace-pre-wrap">{text}</BubbleContent>
      </Bubble>
    </TranscriptRow>
  );
}

function TypingMessage() {
  return (
    <TranscriptRow lane="assistant">
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
    <Attachment className="w-full max-w-full">
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

  return (
    <div className="grid w-full max-w-full gap-2">
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${id}-details`}
        onClick={onToggle}
        tone={failed ? "error" : "success"}
        className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
      >
        <MarkerIcon>
          <BrainIcon />
        </MarkerIcon>
        <MarkerContent>{failed ? "Memory update failed" : "Memory updated"}</MarkerContent>
      </Marker>
      {open ? (
        <MemoryDetailAttachment id={`${id}-details`} extraction={extraction} memoryCount={memories.length} failed={failed} />
      ) : null}
    </div>
  );
}

function MemoryStructuredCard({ schema, memories }: { schema: string; memories: MemoryCardData[] }) {
  const count = memories.length;
  const title = count === 1 ? "Memory saved" : `${count} memories saved`;
  const source = schema === "memory_proposals" ? "Same-call proposal" : "Explicit request";

  return (
    <Attachment className="w-full max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{source}</AttachmentDescription>
      </AttachmentContent>
    </Attachment>
  );
}

function MemoryDetailAttachment({
  id,
  extraction,
  memoryCount,
  failed
}: {
  id: string;
  extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
  memoryCount: number;
  failed: boolean;
}) {
  const title = failed ? "Memory update failed" : memoryCount === 1 ? "Memory saved" : `${memoryCount} memories saved`;
  const status = extraction ? statusLabel(extraction.status) : null;
  const description = extraction?.summary ?? (status ? `Memory extraction ${status.toLowerCase()}` : "Memory proposal");

  return (
    <Attachment id={id} state={failed ? "error" : "done"} className="w-full max-w-full">
      <AttachmentMedia className="text-[var(--pine-700)]">
        <BrainIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{description}</AttachmentDescription>
      </AttachmentContent>
    </Attachment>
  );
}

function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return <ErrorMarker message={message} label={recoverable ? "Notice" : "Error"} recoverable={recoverable} />;
}
