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
import { readableKind, statusLabel } from "../format";
import { memoryCardsFromStructuredItem, type MemoryCardData } from "../memoryCards";
import type { TranscriptEntry, TurnTranscriptItem } from "../types";

type RenderTranscriptEntry =
  | { kind: "entry"; id: string; entry: TranscriptEntry }
  | {
      kind: "memory_marker";
      id: string;
      extraction?: Extract<TurnTranscriptItem, { kind: "activity" }>;
      proposal?: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
    };

export function Transcript({
  entries,
  expandedActivities,
  onToggleActivity
}: {
  entries: TranscriptEntry[];
  expandedActivities: Set<string>;
  onToggleActivity: (id: string) => void;
}) {
  return (
    <MessageScrollerProvider autoScroll defaultScrollPosition="last-anchor" scrollPreviousItemPeek={56}>
      <MessageScroller className="min-h-0 overflow-hidden">
        <MessageScrollerViewport aria-label="Conversation transcript">
          <MessageScrollerContent className="mx-auto flex min-h-full w-[var(--chat-column-width)] flex-col gap-3 px-0.5 py-6">
            {groupMemoryMarkers(entries).map((entry) => (
              <MessageScrollerItem
                key={entry.id}
                className={cn("flex w-full", entry.kind === "entry" && entry.entry.type === "user" && "justify-end")}
                messageId={entry.id}
                scrollAnchor={entry.kind === "entry" && entry.entry.type === "user"}
              >
                {renderTranscriptRenderEntry(entry, expandedActivities, onToggleActivity)}
              </MessageScrollerItem>
            ))}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
    </MessageScrollerProvider>
  );
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
      <MemoryMarker
        id={entry.id}
        extraction={entry.extraction}
        proposal={entry.proposal}
        open={expandedActivities.has(entry.id)}
        onToggle={() => onToggleActivity(entry.id)}
      />
    );
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
      <ActivityRow item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
    );
  }
  if (entry.type === "card") {
    return (
      <StructuredCard item={entry.item} open={expandedActivities.has(entry.id)} onToggle={() => onToggleActivity(entry.id)} />
    );
  }
  return <ErrorNotice message={entry.message} recoverable={entry.recoverable} />;
}

function Message({ role, text }: { role: "user" | "assistant"; text: string }) {
  return (
    <MessagePrimitive align={role === "user" ? "end" : "start"} className="max-w-[760px]">
      <MessageAvatar>
        <Avatar size="sm">
          <AvatarFallback>{role === "user" ? "ME" : "N"}</AvatarFallback>
        </Avatar>
      </MessageAvatar>
      <MessageContent>
        <Bubble variant={role === "user" ? "default" : "muted"}>
          <BubbleContent className="leading-[1.7] whitespace-pre-wrap">{text}</BubbleContent>
        </Bubble>
      </MessageContent>
    </MessagePrimitive>
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
    <Attachment className="max-w-[760px]">
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
    <Attachment className="max-w-[760px]">
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
    <div className="ml-10 grid w-[calc(100%-2.5rem)] max-w-[720px] gap-2">
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${id}-details`}
        onClick={onToggle}
        className={cn(
          "w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none",
          failed && "text-[var(--red-700)]"
        )}
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
    <Attachment className="max-w-[760px]">
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
    <Attachment id={id} state={failed ? "error" : "done"} className="max-w-[760px]">
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
