import React from "react";
import {
  fetchOnboardingStatus,
  fetchProviderAuthAttempt,
  refreshStatus,
  startProviderAuthAttempt,
  webSocketUrl
} from "./api";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { Onboarding } from "./components/Onboarding";
import { StatusCluster } from "./components/StatusCluster";
import { Transcript } from "./components/Transcript";
import type {
  OnboardingStatus,
  ProviderAuthAttemptView,
  WebClientMessage,
  WebServerMessage as ServerMessage,
  WebStatus
} from "./generated/noema";
import { handleServerMessage, pushTranscript } from "./transcript";
import type { ConversationAgentStatus, SocketState, TranscriptEntry } from "./types";

export function App() {
  const [status, setStatus] = React.useState<WebStatus | null>(null);
  const [socketState, setSocketState] = React.useState<SocketState>("closed");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [agentStatus, setAgentStatus] = React.useState<ConversationAgentStatus>("closed");
  const [onboarding, setOnboarding] = React.useState<OnboardingStatus | null>(null);
  const [authAttempt, setAuthAttempt] = React.useState<ProviderAuthAttemptView | null>(null);
  const [onboardingError, setOnboardingError] = React.useState<string | null>(null);
  const [transcript, setTranscript] = React.useState<TranscriptEntry[]>([]);
  const [draft, setDraft] = React.useState("");
  const [pending, setPending] = React.useState(false);
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(new Set());
  const socketRef = React.useRef<WebSocket | null>(null);

  React.useEffect(() => {
    void refreshStatus(setStatus);
    void fetchOnboardingStatus()
      .then(setOnboarding)
      .catch((error: unknown) => {
        setOnboardingError(error instanceof Error ? error.message : "Failed to load onboarding");
      });
  }, []);

  React.useEffect(() => {
    if (!onboarding?.is_user_onboarded || socketRef.current) {
      return;
    }

    const socket = new WebSocket(webSocketUrl());
    socketRef.current = socket;
    setSocketState("connecting");
    setAgentStatus("connecting");

    socket.addEventListener("open", () => {
      const message: WebClientMessage = { type: "primary_conversation_start" };

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
  }, [onboarding?.is_user_onboarded]);

  async function connectProvider() {
    const step = onboarding?.steps.find((candidate) => candidate.id === "connect_provider_account");
    if (!step?.provider_kind || !step.provider_account_id || !step.auth_method) {
      setOnboardingError("No provider account is available to connect.");
      return;
    }

    setOnboardingError(null);
    try {
      const attempt = await startProviderAuthAttempt({
        provider_kind: step.provider_kind,
        provider_account_id: step.provider_account_id,
        method: step.auth_method
      });
      setAuthAttempt(attempt);
      if (attempt.status === "completed") {
        setOnboarding(await fetchOnboardingStatus());
      }
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to start provider login");
    }
  }

  async function checkProviderAuthAttempt() {
    if (!authAttempt) {
      return;
    }

    setOnboardingError(null);
    try {
      const next = await fetchProviderAuthAttempt(authAttempt.attempt_id);
      setAuthAttempt(next);
      if (next.status === "completed") {
        setOnboarding(await fetchOnboardingStatus());
      }
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to check provider login");
    }
  }

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

  if (!onboarding) {
    return (
      <main className="noema-app">
        <Header status={status} socketState={socketState} agentStatus={agentStatus} />
        <section className="onboarding-shell" aria-label="Noema onboarding">
          <div className="onboarding-panel">
            <p className="eyebrow">First run</p>
            <h1>Checking setup</h1>
            <p>Noema is checking whether chat can start.</p>
            {onboardingError ? <p className="onboarding-error">{onboardingError}</p> : null}
          </div>
        </section>
      </main>
    );
  }

  if (!onboarding.is_user_onboarded) {
    return (
      <main className="noema-app">
        <Header status={status} socketState={socketState} agentStatus={agentStatus} />
        <Onboarding
          onboarding={onboarding}
          attempt={authAttempt}
          error={onboardingError}
          onConnect={() => void connectProvider()}
          onCheck={() => void checkProviderAuthAttempt()}
          onRetry={() => {
            setAuthAttempt(null);
            setOnboardingError(null);
          }}
        />
      </main>
    );
  }

  return (
    <main className="noema-app">
      <Header status={status} socketState={socketState} agentStatus={agentStatus} />

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

function Header({
  status,
  socketState,
  agentStatus
}: {
  status: WebStatus | null;
  socketState: SocketState;
  agentStatus: ConversationAgentStatus;
}) {
  return (
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
  );
}
