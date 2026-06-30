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
import { ChatSurface } from "./components/ChatSurface";
import { AppShell } from "@/components/shell/AppShell";
import { SetupFrame } from "@/components/shell/SetupFrame";
import { ErrorMarker } from "./components/ErrorMarker";
import {
  isProviderAuthAttemptPending,
  Onboarding,
  PROVIDER_AUTH_POLL_INTERVAL_MS
} from "./components/Onboarding";
import { MemoryGraphPage } from "./pages/MemoryGraphPage";
import { MemoryHomePage } from "./pages/MemoryHomePage";
import { SettingsSurface } from "./pages/SettingsPage";
import { useBrowserRoute, type AppRoute } from "./routes";
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

export function shouldRefreshLocalStatusForConversationEvent(event: unknown) {
  if (!isRecord(event) || event.__typename !== "GraphqlConversationItemEvent") {
    return false;
  }

  const item = event.item;
  if (!isRecord(item) || item.__typename !== "GraphqlActivity") {
    return false;
  }

  if (item.activityKind !== "tool_result") {
    return false;
  }

  const metadata = item.metadata;
  const action = isRecord(metadata) ? metadata.action : null;
  return (
    isRecord(action) &&
    action.name === "update_own_name" &&
    action.success === true
  );
}

export function shouldRouteThroughAppShell({
  route,
  onboarded
}: {
  route: AppRoute;
  onboarded: boolean;
}) {
  return onboarded && Boolean(route);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function App() {
  const { route, navigate, goBackFromSettings } = useBrowserRoute();
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
  const agentName = status?.primaryAgentDisplayName ?? null;
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
    if (shouldRefreshLocalStatusForConversationEvent(event)) {
      void localStatus.refetch();
    }
  }, [conversationEvents.data, localStatus]);

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
    <ChatSurface
      transcript={transcript}
      pending={pending}
      expandedActivities={expandedActivities}
      draft={draft}
      ready={ready}
      agentName={agentName}
      onPickStarter={(starter) => setDraft(starter)}
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
      onDraftChange={setDraft}
      onSubmit={() => void sendMessage(draft)}
    />
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
      <AppShell
        route={route}
        status={status}
        socketState={socketState}
        onNavigate={navigate}
        goBackFromSettings={goBackFromSettings}
      >
        <MemoryHomePage onOpenGraph={() => navigate({ kind: "memory_graph" })} />
      </AppShell>
    );
  }

  if (route.kind === "memory_graph") {
    return (
      <AppShell
        route={route}
        status={status}
        socketState={socketState}
        onNavigate={navigate}
        goBackFromSettings={goBackFromSettings}
      >
        <MemoryGraphPage />
      </AppShell>
    );
  }

  if (route.kind === "settings") {
    return (
      <AppShell
        route={route}
        status={status}
        socketState={socketState}
        onNavigate={navigate}
        goBackFromSettings={goBackFromSettings}
      >
        <SettingsSurface section={route.section} />
      </AppShell>
    );
  }

  return (
    <AppShell
      route={route}
      status={status}
      socketState={socketState}
      onNavigate={navigate}
      goBackFromSettings={goBackFromSettings}
    >
      {chatView}
    </AppShell>
  );
}
