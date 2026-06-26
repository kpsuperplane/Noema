import React from "react";
import { refreshStatus, webSocketUrl } from "./api";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { StatusCluster } from "./components/StatusCluster";
import { Transcript } from "./components/Transcript";
import type { WebClientMessage, WebServerMessage as ServerMessage, WebStatus } from "./generated/noema";
import { handleServerMessage, pushTranscript } from "./transcript";
import type { ConversationAgentStatus, SocketState, TranscriptEntry } from "./types";

export function App() {
  const [status, setStatus] = React.useState<WebStatus | null>(null);
  const [socketState, setSocketState] = React.useState<SocketState>("connecting");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [agentStatus, setAgentStatus] = React.useState<ConversationAgentStatus>("connecting");
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
      const message: WebClientMessage = { type: "conversation_start" };

      setSocketState("ready");
      socket.send(JSON.stringify(message));
    });

    socket.addEventListener("message", (event: MessageEvent<string>) => {
      const message = JSON.parse(event.data) as ServerMessage;
      handleServerMessage(message, {
        setConversationId,
        setTranscript,
        setPending,
        setAgentStatus
      });
    });

    socket.addEventListener("close", () => {
      setSocketState("closed");
      setAgentStatus("closed");
      setPending(false);
    });

    socket.addEventListener("error", () => {
      setSocketState("closed");
      setAgentStatus("closed");
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
    const message: WebClientMessage = {
      type: "conversation_turn",
      conversation_id: conversationId,
      input,
      client_message_id: clientMessageId
    };

    setDraft("");
    setPending(true);
    pushTranscript(setTranscript, { id: clientMessageId, type: "user", text: input });
    socket.send(JSON.stringify(message));
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
        <StatusCluster status={status} socketState={socketState} agentStatus={agentStatus} />
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
