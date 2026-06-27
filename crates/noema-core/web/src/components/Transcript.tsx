import {
  Attachment,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle
} from "@/components/ui/attachment";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Bubble, BubbleContent } from "@/components/ui/bubble";
import { Button } from "@/components/ui/button";
import { Marker, MarkerContent } from "@/components/ui/marker";
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
import { formatPercent, readableKind, statusLabel } from "../format";
import { memoryCardsFromStructuredItem, type MemoryCardData } from "../memoryCards";
import type { TranscriptEntry, TurnTranscriptItem } from "../types";

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
            {entries.map((entry) => (
              <MessageScrollerItem
                key={entry.id}
                className={cn("flex w-full", entry.type === "user" && "justify-end")}
                messageId={entry.id}
                scrollAnchor={entry.type === "user"}
              >
                {renderTranscriptEntry(entry, expandedActivities, onToggleActivity)}
              </MessageScrollerItem>
            ))}
          </MessageScrollerContent>
        </MessageScrollerViewport>
        <MessageScrollerButton />
      </MessageScroller>
    </MessageScrollerProvider>
  );
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
    return <StructuredCard item={entry.item} />;
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

function StructuredCard({ item }: { item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }> }) {
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

function MemoryStructuredCard({ schema, memories }: { schema: string; memories: MemoryCardData[] }) {
  const count = memories.length;
  const title = count === 1 ? "Memory saved" : `${count} memories saved`;
  const source = schema === "memory_proposals" ? "Same-call proposal" : "Explicit request";

  return (
    <Attachment className="grid max-w-[760px] gap-3">
      <div className="grid grid-cols-[28px_minmax(0,1fr)] items-center gap-2.5">
        <AttachmentMedia className="size-7 font-mono text-xs font-bold text-[var(--pine-700)]">M</AttachmentMedia>
        <span>
          <AttachmentTitle>{title}</AttachmentTitle>
          <AttachmentDescription>{source}</AttachmentDescription>
        </span>
      </div>
      <AttachmentContent className="grid border-t border-[var(--border-subtle)]">
        {memories.map((memory, index) => (
          <section
            key={memory.id ?? `${memory.title}:${index}`}
            className={cn("grid gap-[7px] pt-3", index > 0 && "mt-3 border-t border-[var(--border-subtle)]")}
          >
            <div className="flex min-w-0 items-start justify-between gap-2.5 max-[760px]:flex-wrap max-[760px]:justify-start">
              <AttachmentTitle className="min-w-0 [overflow-wrap:anywhere]">{memory.title}</AttachmentTitle>
              <Badge variant="outline">{memory.status ? readableKind(memory.status) : "Saved"}</Badge>
            </div>
            <p className="m-0 text-[13px] leading-[1.55] text-muted-foreground [overflow-wrap:anywhere]">{memory.content}</p>
            <div className="flex flex-wrap gap-1.5">
              {memory.memoryType ? <Badge variant="secondary">{readableKind(memory.memoryType)}</Badge> : null}
              {memory.sensitivity ? <Badge variant="secondary">{readableKind(memory.sensitivity)}</Badge> : null}
              {typeof memory.confidence === "number" ? <Badge variant="secondary">{formatPercent(memory.confidence)}</Badge> : null}
              {memory.id ? <Badge variant="secondary">{memory.id}</Badge> : null}
            </div>
            {memory.evidenceExcerpt ? (
              <small className="font-mono text-[10px] text-muted-foreground">{memory.evidenceExcerpt}</small>
            ) : null}
          </section>
        ))}
      </AttachmentContent>
    </Attachment>
  );
}

function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return (
    <Marker
      role={recoverable ? "status" : "alert"}
      variant="border"
      className="grid max-w-[760px] gap-1 bg-[var(--red-100)] text-[var(--red-700)]"
    >
      <MarkerContent>
        <strong>{recoverable ? "Notice" : "Error"}</strong>
        <span>{message}</span>
      </MarkerContent>
    </Marker>
  );
}
