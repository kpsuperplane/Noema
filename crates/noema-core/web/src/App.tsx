import React from "react";
import { useApolloClient, useMutation, useQuery, useSubscription } from "@apollo/client/react";
import {
  ConversationEventsDocument,
  LocalStatusDocument,
  OnboardingStatusDocument,
  ProviderAuthAttemptDocument,
  SendConversationTurnDocument,
  StartPrimaryConversationDocument,
  StartProviderAuthAttemptDocument,
  type ProviderAuthAttemptQuery,
  type StartProviderAuthAttemptMutation
} from "./generated/graphql";
import { AppShell } from "@/components/shell/AppShell";
import { SetupFrame } from "@/components/shell/SetupFrame";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { ErrorMarker } from "./components/ErrorMarker";
import {
  isProviderAuthAttemptPending,
  Onboarding,
  PROVIDER_AUTH_POLL_INTERVAL_MS
} from "./components/Onboarding";
import { Transcript } from "./components/Transcript";
import { MemoryGraphPage } from "./pages/MemoryGraphPage";
import { MemoryHomePage } from "./pages/MemoryHomePage";
import { useBrowserRoute } from "./routes";
import { entriesFromReplay, handleConversationEvent, pushTranscript } from "./transcript";
import type { ConversationAgentStatus, SocketState, TranscriptEntry } from "./types";

type ProviderAuthAttemptView =
  | StartProviderAuthAttemptMutation["startProviderAuthAttempt"]
  | NonNullable<ProviderAuthAttemptQuery["providerAuthAttempt"]>;

type SendMessageReadiness = {
  text: string;
  conversationId: string | null;
  socketState: SocketState;
  pending: boolean;
};

export function canSendMessage(readiness: SendMessageReadiness): readiness is SendMessageReadiness & {
  conversationId: string;
} {
  const { text, conversationId, socketState } = readiness;
  return Boolean(text.trim() && conversationId && socketState === "ready");
}

export function App() {
  const { route, navigate } = useBrowserRoute();
  const apolloClient = useApolloClient();
  const localStatus = useQuery(LocalStatusDocument);
  const onboardingStatus = useQuery(OnboardingStatusDocument);
  const [startProviderAuthAttempt] = useMutation(StartProviderAuthAttemptDocument);
  const [startPrimaryConversation] = useMutation(StartPrimaryConversationDocument);
  const [sendConversationTurn] = useMutation(SendConversationTurnDocument);

  const [socketState, setSocketState] = React.useState<SocketState>("closed");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [, setAgentStatus] = React.useState<ConversationAgentStatus>("closed");
  const [authAttempt, setAuthAttempt] = React.useState<ProviderAuthAttemptView | null>(null);
  const [onboardingError, setOnboardingError] = React.useState<string | null>(null);
  const [transcript, setTranscript] = React.useState<TranscriptEntry[]>([]);
  const [draft, setDraft] = React.useState("");
  const [pending, setPending] = React.useState(false);
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(new Set());
  const startingConversationRef = React.useRef(false);

  const onboarding = onboardingStatus.data?.onboardingStatus ?? null;
  const status = localStatus.data?.localStatus ?? null;
  const onboarded = onboarding?.isUserOnboarded ?? false;
  const chatRoute = route.kind === "chat";
  const displayedOnboardingError = onboardingError ?? onboardingStatus.error?.message ?? null;
  const authAttemptId = authAttempt?.attemptId;
  const authAttemptStatus = authAttempt?.status;

  const reportConversationError = React.useCallback((error: Error) => {
    setSocketState("closed");
    setAgentStatus("closed");
    setPending(false);
    pushTranscript(setTranscript, {
      id: crypto.randomUUID(),
      type: "error",
      message: error.message,
      recoverable: true
    });
  }, []);

  const conversationEvents = useSubscription(ConversationEventsDocument, {
    variables: { conversationId: conversationId ?? "" },
    skip: !conversationId,
    onError: reportConversationError
  });

  React.useEffect(() => {
    if (!chatRoute || !onboarded || conversationId || startingConversationRef.current) {
      return;
    }

    startingConversationRef.current = true;
    setSocketState("connecting");
    setAgentStatus("connecting");
    void startPrimaryConversation()
      .then((result) => {
        const started = result.data?.startPrimaryConversation;
        if (!started) {
          throw new Error("Noema did not return a conversation.");
        }
        setConversationId(started.conversationId);
        setTranscript(entriesFromReplay(started.replay));
        setPending(false);
        setSocketState("ready");
        setAgentStatus("IDLE");
      })
      .catch((error: unknown) => {
        setSocketState("closed");
        setAgentStatus("closed");
        setPending(false);
        pushTranscript(setTranscript, {
          id: crypto.randomUUID(),
          type: "error",
          message: error instanceof Error ? error.message : "Noema could not start chat.",
          recoverable: true
        });
      })
      .finally(() => {
        startingConversationRef.current = false;
      });
  }, [chatRoute, conversationId, onboarded, startPrimaryConversation]);

  React.useEffect(() => {
    const event = conversationEvents.data?.conversationEvents;
    if (!event) {
      return;
    }
    handleConversationEvent(event, {
      setTranscript,
      setPending,
      setAgentStatus
    });
  }, [conversationEvents.data]);

  async function connectProvider() {
    const step = onboarding?.steps.find((candidate) => candidate.id === "connect_provider_account");
    if (!step?.providerKind || !step.providerAccountId || !step.authMethod) {
      setOnboardingError("No provider account is available to connect.");
      return;
    }

    setOnboardingError(null);
    try {
      const result = await startProviderAuthAttempt({
        variables: {
          input: {
            providerKind: step.providerKind,
            providerAccountId: step.providerAccountId,
            method: step.authMethod
          }
        }
      });
      const attempt = result.data?.startProviderAuthAttempt;
      if (!attempt) {
        throw new Error("Noema did not return a provider login attempt.");
      }
      setAuthAttempt(attempt);
      if (attempt.status === "COMPLETED") {
        await onboardingStatus.refetch();
      }
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to start provider login");
    }
  }

  const checkProviderAuthAttempt = React.useCallback(
    async (attemptId = authAttempt?.attemptId) => {
      if (!attemptId) {
        return;
      }

      setOnboardingError(null);
      try {
        const result = await apolloClient.query({
          query: ProviderAuthAttemptDocument,
          variables: { attemptId },
          fetchPolicy: "network-only"
        });
        const next = result.data?.providerAuthAttempt;
        if (!next) {
          throw new Error("Provider login attempt was not found.");
        }
        setAuthAttempt(next);
        if (next.status === "COMPLETED") {
          await onboardingStatus.refetch();
        }
      } catch (error: unknown) {
        setOnboardingError(error instanceof Error ? error.message : "Failed to check provider login");
      }
    },
    [apolloClient, authAttempt?.attemptId, onboardingStatus]
  );

  React.useEffect(() => {
    if (!authAttemptId || !authAttemptStatus || !isProviderAuthAttemptPending(authAttemptStatus)) {
      return;
    }

    let cancelled = false;
    let timeoutId: number | null = null;

    const poll = () => {
      timeoutId = window.setTimeout(() => {
        if (cancelled) {
          return;
        }
        void checkProviderAuthAttempt(authAttemptId).then(() => {
          if (!cancelled) {
            poll();
          }
        });
      }, PROVIDER_AUTH_POLL_INTERVAL_MS);
    };

    poll();
    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [authAttemptId, authAttemptStatus, checkProviderAuthAttempt]);

  async function sendMessage(text: string) {
    const input = text.trim();
    const readiness = { text, conversationId, socketState, pending };
    if (!canSendMessage(readiness)) {
      return;
    }

    const clientMessageId = crypto.randomUUID();
    setDraft("");
    setPending(true);
    setAgentStatus("INPUT_RECEIVED");
    pushTranscript(setTranscript, { id: clientMessageId, type: "user", text: input });

    try {
      await sendConversationTurn({
        variables: {
          input: {
            conversationId: readiness.conversationId,
            input,
            clientMessageId
          }
        }
      });
    } catch (error: unknown) {
      setPending(false);
      pushTranscript(setTranscript, {
        id: crypto.randomUUID(),
        type: "error",
        message: error instanceof Error ? error.message : "Noema could not send that message.",
        recoverable: true
      });
    }
  }

  const ready = socketState === "ready" && conversationId !== null;
  const waitingForConversationDecision =
    chatRoute && onboarding?.isUserOnboarded === true && !conversationId && transcript.length === 0;
  const chatView = (
    <section
      className="grid h-full min-h-0 w-full grid-rows-[minmax(0,1fr)_auto] overflow-hidden pb-[22px] [--chat-column-width:min(860px,calc(100%_-_48px))] max-[760px]:pb-[18px] max-[760px]:[--chat-column-width:calc(100%_-_40px)]"
      aria-label="Noema chat"
    >
      {transcript.length === 0 ? (
        <EmptyState onPick={(starter) => setDraft(starter)} />
      ) : (
        <Transcript
          entries={transcript}
          pending={pending}
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
        ready={ready}
        pending={pending}
        placeholder={ready ? "Message Noema" : "Starting Noema chat..."}
        onChange={setDraft}
        onSubmit={() => void sendMessage(draft)}
      />
    </section>
  );

  if (waitingForConversationDecision) {
    return null;
  }

  if (!onboarding) {
    return (
      <SetupFrame>
        <section
          className="mx-auto grid min-h-full w-[min(760px,100%)] content-center px-6 max-[760px]:content-start max-[760px]:px-5"
          aria-label="Noema onboarding"
        >
          <div className="grid min-w-0 gap-3.5 py-[18px]">
            <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">First run</p>
            <h1 className="m-0 font-heading text-[34px] leading-[1.1] tracking-normal text-foreground [overflow-wrap:anywhere] max-[760px]:text-3xl">
              Checking setup
            </h1>
            <p className="m-0 max-w-[560px] text-muted-foreground [overflow-wrap:anywhere]">
              Noema is checking whether chat can start.
            </p>
            {displayedOnboardingError ? <ErrorMarker message={displayedOnboardingError} /> : null}
          </div>
        </section>
      </SetupFrame>
    );
  }

  if (!onboarding.isUserOnboarded) {
    return (
      <SetupFrame>
        <Onboarding
          onboarding={onboarding}
          attempt={authAttempt}
          error={displayedOnboardingError}
          onConnect={() => void connectProvider()}
          onRetry={() => {
            setAuthAttempt(null);
            setOnboardingError(null);
          }}
        />
      </SetupFrame>
    );
  }

  if (route.kind === "memory_home") {
    return (
      <AppShell route={route} status={status} socketState={socketState} onNavigate={navigate}>
        <MemoryHomePage onOpenGraph={() => navigate({ kind: "memory_graph" })} />
      </AppShell>
    );
  }

  if (route.kind === "memory_graph") {
    return (
      <AppShell route={route} status={status} socketState={socketState} onNavigate={navigate}>
        <MemoryGraphPage />
      </AppShell>
    );
  }

  return (
    <AppShell route={route} status={status} socketState={socketState} onNavigate={navigate}>
      {chatView}
    </AppShell>
  );
}
