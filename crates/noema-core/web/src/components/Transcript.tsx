import type { TurnTranscriptItem } from "../generated/noema";
import { formatPercent, readableKind, statusLabel } from "../format";
import { memoryCardsFromStructuredItem, type MemoryCardData } from "../memoryCards";
import type { TranscriptEntry } from "../types";

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
    <div className="transcript" aria-live="polite">
      {entries.map((entry) => {
        if (entry.type === "user") {
          return <Message key={entry.id} role="user" name="You" text={entry.text} />;
        }
        if (entry.type === "assistant") {
          return <Message key={entry.id} role="assistant" name="Noema" text={entry.text} />;
        }
        if (entry.type === "activity") {
          return (
            <ActivityRow
              key={entry.id}
              item={entry.item}
              open={expandedActivities.has(entry.id)}
              onToggle={() => onToggleActivity(entry.id)}
            />
          );
        }
        if (entry.type === "card") {
          return <StructuredCard key={entry.id} item={entry.item} />;
        }
        return <ErrorNotice key={entry.id} message={entry.message} recoverable={entry.recoverable} />;
      })}
    </div>
  );
}
function Message({ role, name, text }: { role: "user" | "assistant"; name: string; text: string }) {
  return (
    <article className={`message message--${role}`}>
      <div className="message__meta">{name}</div>
      <div className="message__body">{text}</div>
    </article>
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
    <article className={`activity activity--${item.status}`}>
      <button type="button" className="activity__summary" onClick={onToggle} aria-expanded={open}>
        <span className="activity__glyph">{isMemorySave ? "M" : "A"}</span>
        <span>
          <strong>{title}</strong>
          {item.summary ? <small>{item.summary}</small> : null}
        </span>
        <em>{status}</em>
      </button>
      {open ? (
        <div className="activity__detail">
          <dl>
            <div>
              <dt>What happened</dt>
              <dd>{item.summary || title}</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>{isMemorySave ? "You used an explicit remember request in this chat." : readableKind(item.activity_kind)}</dd>
            </div>
            <div>
              <dt>Next step</dt>
              <dd>{isMemorySave ? "Continue chatting. Deeper memory settings come in the next slice." : "No action needed."}</dd>
            </div>
          </dl>
        </div>
      ) : null}
    </article>
  );
}

function StructuredCard({ item }: { item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }> }) {
  const memories = memoryCardsFromStructuredItem(item);
  if (memories) {
    return <MemoryStructuredCard schema={item.schema} memories={memories} />;
  }

  return (
    <article className="structured-card">
      <strong>{item.schema}</strong>
      <small>Structured card placeholder</small>
    </article>
  );
}

function MemoryStructuredCard({ schema, memories }: { schema: string; memories: MemoryCardData[] }) {
  const count = memories.length;
  const title = count === 1 ? "Memory saved" : `${count} memories saved`;
  const source = schema === "memory_proposals" ? "Same-call proposal" : "Explicit request";

  return (
    <article className="structured-card structured-card--memory">
      <div className="structured-card__header">
        <span className="structured-card__glyph">M</span>
        <span>
          <strong>{title}</strong>
          <small>{source}</small>
        </span>
      </div>
      <div className="memory-card-list">
        {memories.map((memory, index) => (
          <section key={memory.id ?? `${memory.title}:${index}`} className="memory-card-row">
            <div className="memory-card-row__title">
              <strong>{memory.title}</strong>
              <span>{memory.status ? readableKind(memory.status) : "Saved"}</span>
            </div>
            <p>{memory.content}</p>
            <div className="memory-card-row__meta">
              {memory.memoryType ? <span>{readableKind(memory.memoryType)}</span> : null}
              {memory.sensitivity ? <span>{readableKind(memory.sensitivity)}</span> : null}
              {typeof memory.confidence === "number" ? <span>{formatPercent(memory.confidence)}</span> : null}
              {memory.id ? <span>{memory.id}</span> : null}
            </div>
            {memory.evidenceExcerpt ? <small>{memory.evidenceExcerpt}</small> : null}
          </section>
        ))}
      </div>
    </article>
  );
}

function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return (
    <article className="error-notice">
      <strong>{recoverable ? "Notice" : "Error"}</strong>
      <span>{message}</span>
    </article>
  );
}
