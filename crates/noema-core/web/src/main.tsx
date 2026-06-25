import React from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

type WebStatus = {
  local_service: "running" | string;
  assistant_connection: "codex" | string;
  memory_storage: "ready" | "initializing" | string;
};

type ActivityStatus = "started" | "completed" | "failed";

type TurnTranscriptItem =
  | { kind: "assistant_text"; text: string }
  | {
      kind: "activity";
      id: string;
      activity_kind: string;
      status: ActivityStatus;
      title: string;
      summary?: string;
      metadata: Record<string, unknown>;
    }
  | { kind: "a2ui_card"; id: string; schema: string; payload: unknown }
  | { kind: "error_notice"; message: string; recoverable: boolean };

type ServerMessage =
  | {
      type: "conversation_started";
      conversation_id: string;
      provider: string;
      provider_thread_id: string;
    }
  | {
      type: "turn_transcript_item";
      conversation_id: string;
      client_message_id?: string;
      item: TurnTranscriptItem;
    }
  | {
      type: "turn_completed";
      conversation_id: string;
      client_message_id?: string;
    }
  | { type: "ok"; message?: string }
  | { type: "error"; message: string };

type TranscriptEntry =
  | { id: string; type: "user"; text: string }
  | { id: string; type: "assistant"; text: string }
  | { id: string; type: "activity"; item: Extract<TurnTranscriptItem, { kind: "activity" }> }
  | { id: string; type: "card"; item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }> }
  | { id: string; type: "error"; message: string; recoverable: boolean };

const STARTERS = [
  "Say hello and tell me Noema is working.",
  "remember this: I prefer concise setup instructions",
  "What did you just remember?"
];

function App() {
  const [status, setStatus] = React.useState<WebStatus | null>(null);
  const [socketState, setSocketState] = React.useState<"connecting" | "ready" | "closed">("connecting");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [transcript, setTranscript] = React.useState<TranscriptEntry[]>([]);
  const [draft, setDraft] = React.useState("");
  const [pending, setPending] = React.useState(false);
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(new Set());
  const socketRef = React.useRef<WebSocket | null>(null);

  React.useEffect(() => {
    void refreshStatus(setStatus);
    const socket = new WebSocket(webSocketUrl());
    socketRef.current = socket;

    socket.addEventListener("open", () => {
      setSocketState("ready");
      socket.send(JSON.stringify({ type: "conversation_start" }));
    });

    socket.addEventListener("message", (event: MessageEvent<string>) => {
      const message = JSON.parse(event.data) as ServerMessage;
      handleServerMessage(message, {
        setConversationId,
        setTranscript,
        setPending
      });
    });

    socket.addEventListener("close", () => {
      setSocketState("closed");
      setPending(false);
    });

    socket.addEventListener("error", () => {
      setSocketState("closed");
      setPending(false);
      pushTranscript(setTranscript, {
        id: crypto.randomUUID(),
        type: "error",
        message: "Noema's local web connection closed. Refresh the page or restart Noema.",
        recoverable: true
      });
    });

    return () => {
      socket.close();
      socketRef.current = null;
    };
  }, []);

  function sendMessage(text: string) {
    const input = text.trim();
    const socket = socketRef.current;
    if (!input || !socket || socket.readyState !== WebSocket.OPEN || !conversationId || pending) {
      return;
    }

    const clientMessageId = crypto.randomUUID();
    setDraft("");
    setPending(true);
    pushTranscript(setTranscript, { id: clientMessageId, type: "user", text: input });
    socket.send(
      JSON.stringify({
        type: "conversation_turn",
        conversation_id: conversationId,
        input,
        client_message_id: clientMessageId
      })
    );
  }

  const ready = socketState === "ready" && conversationId !== null;

  return (
    <main className="noema-app">
      <header className="topbar">
        <div className="brand">
          <img src="/assets/noema-mark.svg" width="34" height="34" alt="" />
          <div>
            <strong>Noema</strong>
            <span>Local chat</span>
          </div>
        </div>
        <StatusCluster status={status} socketState={socketState} />
      </header>

      <section className="chat-shell" aria-label="Noema chat">
        {transcript.length === 0 ? (
          <EmptyState onPick={(starter) => setDraft(starter)} />
        ) : (
          <Transcript
            entries={transcript}
            expandedActivities={expandedActivities}
            onToggleActivity={(id) =>
              setExpandedActivities((current) => {
                const next = new Set(current);
                if (next.has(id)) {
                  next.delete(id);
                } else {
                  next.add(id);
                }
                return next;
              })
            }
          />
        )}

        <Composer
          value={draft}
          disabled={!ready || pending}
          pending={pending}
          placeholder={ready ? "Message Noema" : "Starting Noema chat..."}
          onChange={setDraft}
          onSubmit={() => sendMessage(draft)}
        />
      </section>
    </main>
  );
}

function StatusCluster({
  status,
  socketState
}: {
  status: WebStatus | null;
  socketState: "connecting" | "ready" | "closed";
}) {
  const localService = status?.local_service === "running" ? "Ready" : "Checking";
  const memory = status?.memory_storage === "ready" ? "Memory ready" : "Memory starting";
  const socket = socketState === "ready" ? "Chat live" : socketState === "connecting" ? "Connecting" : "Disconnected";

  return (
    <div className="status-cluster" aria-label="Local status">
      <StatusPill tone={status?.local_service === "running" ? "good" : "neutral"}>{localService}</StatusPill>
      <StatusPill tone={status?.memory_storage === "ready" ? "good" : "neutral"}>{memory}</StatusPill>
      <StatusPill tone={socketState === "ready" ? "good" : socketState === "closed" ? "bad" : "neutral"}>
        {socket}
      </StatusPill>
    </div>
  );
}

function StatusPill({ tone, children }: { tone: "good" | "bad" | "neutral"; children: React.ReactNode }) {
  return <span className={`status-pill status-pill--${tone}`}>{children}</span>;
}

function EmptyState({ onPick }: { onPick: (starter: string) => void }) {
  return (
    <div className="empty-state">
      <p className="eyebrow">Today</p>
      <h1>What should we work on?</h1>
      <p>
        Start with one chat. Memory and activity appear in the transcript when Noema has something worth showing.
      </p>
      <div className="starter-grid">
        {STARTERS.map((starter) => (
          <button key={starter} type="button" onClick={() => onPick(starter)}>
            {starter}
          </button>
        ))}
      </div>
    </div>
  );
}

function Transcript({
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
  return (
    <article className="structured-card">
      <strong>{item.schema}</strong>
      <small>Structured card placeholder</small>
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

function Composer({
  value,
  disabled,
  pending,
  placeholder,
  onChange,
  onSubmit
}: {
  value: string;
  disabled: boolean;
  pending: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
}) {
  return (
    <form
      className="composer"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <textarea
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        rows={3}
        onChange={(event) => onChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            onSubmit();
          }
        }}
      />
      <button type="submit" disabled={disabled || !value.trim()}>
        {pending ? "Sending" : "Send"}
      </button>
    </form>
  );
}

function handleServerMessage(
  message: ServerMessage,
  setters: {
    setConversationId: React.Dispatch<React.SetStateAction<string | null>>;
    setTranscript: React.Dispatch<React.SetStateAction<TranscriptEntry[]>>;
    setPending: React.Dispatch<React.SetStateAction<boolean>>;
  }
) {
  if (message.type === "conversation_started") {
    setters.setConversationId(message.conversation_id);
    return;
  }
  if (message.type === "turn_completed") {
    setters.setPending(false);
    return;
  }
  if (message.type === "error") {
    setters.setPending(false);
    pushTranscript(setters.setTranscript, {
      id: crypto.randomUUID(),
      type: "error",
      message: message.message,
      recoverable: true
    });
    return;
  }
  if (message.type !== "turn_transcript_item") {
    return;
  }

  const item = message.item;
  if (item.kind === "assistant_text") {
    pushTranscript(setters.setTranscript, { id: crypto.randomUUID(), type: "assistant", text: item.text });
  } else if (item.kind === "activity") {
    pushTranscript(setters.setTranscript, { id: item.id, type: "activity", item });
  } else if (item.kind === "a2ui_card") {
    pushTranscript(setters.setTranscript, { id: item.id, type: "card", item });
  } else {
    pushTranscript(setters.setTranscript, {
      id: crypto.randomUUID(),
      type: "error",
      message: item.message,
      recoverable: item.recoverable
    });
  }
}

function pushTranscript(
  setTranscript: React.Dispatch<React.SetStateAction<TranscriptEntry[]>>,
  entry: TranscriptEntry
) {
  setTranscript((current) => [...current, entry]);
}

async function refreshStatus(setStatus: React.Dispatch<React.SetStateAction<WebStatus | null>>) {
  try {
    const response = await fetch("/api/status");
    if (response.ok) {
      setStatus((await response.json()) as WebStatus);
    }
  } catch {
    setStatus(null);
  }
}

function webSocketUrl() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/api/chat/ws`;
}

function statusLabel(status: ActivityStatus) {
  if (status === "started") {
    return "Running";
  }
  if (status === "failed") {
    return "Failed";
  }
  return "Done";
}

function readableKind(kind: string) {
  return kind.replaceAll("_", " ");
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
