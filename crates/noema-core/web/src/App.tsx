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
import { AppHeader } from "@/components/shell/AppHeader";
import { Composer } from "./components/Composer";
import { EmptyState } from "./components/EmptyState";
import { Onboarding } from "./components/Onboarding";
import { Transcript } from "./components/Transcript";
import { entriesFromReplay, handleConversationEvent, pushTranscript } from "./transcript";
import type { ConversationAgentStatus, SocketState, TranscriptEntry } from "./types";

type ProviderAuthAttemptView =
  | StartProviderAuthAttemptMutation["startProviderAuthAttempt"]
  | NonNullable<ProviderAuthAttemptQuery["providerAuthAttempt"]>;

export function App() {
  const apolloClient = useApolloClient();
  const localStatus = useQuery(LocalStatusDocument);
  const onboardingStatus = useQuery(OnboardingStatusDocument);
  const [startProviderAuthAttempt] = useMutation(StartProviderAuthAttemptDocument);
  const [startPrimaryConversation] = useMutation(StartPrimaryConversationDocument);
  const [sendConversationTurn] = useMutation(SendConversationTurnDocument);

  const [socketState, setSocketState] = React.useState<SocketState>("closed");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [agentStatus, setAgentStatus] = React.useState<ConversationAgentStatus>("closed");
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
  const displayedOnboardingError = onboardingError ?? onboardingStatus.error?.message ?? null;

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
    if (!onboarded || conversationId || startingConversationRef.current) {
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
  }, [conversationId, onboarded, startPrimaryConversation]);

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

  async function checkProviderAuthAttempt() {
    if (!authAttempt) {
      return;
    }

    setOnboardingError(null);
    try {
      const result = await apolloClient.query({
        query: ProviderAuthAttemptDocument,
        variables: { attemptId: authAttempt.attemptId },
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
  }

  async function sendMessage(text: string) {
    const input = text.trim();
    if (!input || !conversationId || socketState !== "ready" || pending) {
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
            conversationId,
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
  const waitingForOnboardingDecision = !onboarding && !displayedOnboardingError;
  const waitingForConversationDecision = onboarding?.isUserOnboarded === true && !conversationId && transcript.length === 0;

  if (waitingForOnboardingDecision || waitingForConversationDecision) {
    return null;
  }

  if (!onboarding) {
    return (
      <main className="grid h-dvh min-h-screen grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background">
        <AppHeader status={status} socketState={socketState} agentStatus={agentStatus} />
        <section
          className="mx-auto grid min-h-[calc(100vh-68px)] w-[min(760px,100%)] content-center px-6 max-[760px]:min-h-[calc(100vh-118px)] max-[760px]:content-start max-[760px]:px-5"
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
            {displayedOnboardingError ? <p className="m-0 text-[var(--red-700)]">{displayedOnboardingError}</p> : null}
          </div>
        </section>
      </main>
    );
  }

  if (!onboarding.isUserOnboarded) {
    return (
      <main className="grid h-dvh min-h-screen grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background">
        <AppHeader status={status} socketState={socketState} agentStatus={agentStatus} />
        <Onboarding
          onboarding={onboarding}
          attempt={authAttempt}
          error={displayedOnboardingError}
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
    <main className="grid h-dvh min-h-screen grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background">
      <AppHeader status={status} socketState={socketState} agentStatus={agentStatus} />

      <section
        className="grid h-full min-h-0 w-full grid-rows-[minmax(0,1fr)_auto] overflow-hidden pb-[22px] [--chat-column-width:min(860px,calc(100%_-_48px))] max-[760px]:pb-[18px] max-[760px]:[--chat-column-width:calc(100%_-_40px)]"
        aria-label="Noema chat"
      >
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
          onSubmit={() => void sendMessage(draft)}
        />
      </section>
    </main>
  );
}
