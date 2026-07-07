import React from "react";
import {
  useApolloClient,
  useMutation,
  useSubscription,
  useSuspenseQuery
} from "@apollo/client/react";
import {
  ChatBootDocument,
  ConversationEventsDocument,
  ConversationTranscriptPageDocument,
  EnsurePrimaryConversationDocument,
  ProviderAuthAttemptDocument,
  SendConversationTurnDocument,
  StartProviderAuthAttemptDocument,
  type ChatBootQuery,
  type ProviderAuthAttemptQuery,
  type StartProviderAuthAttemptMutation
} from "@/generated/graphql";
import { ChatSurface } from "@/components/ChatSurface";
import { AppShell } from "@/components/shell/AppShell";
import { AppBootBoundary } from "@/components/shell/AppBootBoundary";
import { SetupFrame } from "@/components/shell/SetupFrame";
import {
  isProviderAuthAttemptPending,
  Onboarding,
  PROVIDER_AUTH_POLL_INTERVAL_MS
} from "@/components/Onboarding";
import { MemoryGraphPage } from "@/pages/MemoryGraphPage";
import { MemoryHomePage } from "@/pages/MemoryHomePage";
import { SettingsSurface } from "@/pages/SettingsPage";
import { useBrowserRoute } from "./routes";
import {
  appendAssistantTextDeltaEntry,
  entriesFromReplay,
  entryFromConversationEvent,
  isAgentStatusEvent,
  isAssistantTextDeltaEvent,
  isTurnCompletedEvent,
  markConversationEventReceived,
  markConversationEventScheduled,
  removeStaleStartedMemoryExtractions,
  type ConversationEvent
} from "@/transcript/events";
import {
  appendOptimisticEntry,
  emptyTranscriptWindow,
  mergeDurableEntries,
  replaceOptimisticEntry,
  transcriptWindowEntries
} from "@/transcript/window";
import type { ConversationAgentStatus, SocketState } from "@/shared/types";
import { createClientId } from "@/shared/clientId";

type ProviderAuthAttemptView =
  | StartProviderAuthAttemptMutation["startProviderAuthAttempt"]
  | NonNullable<ProviderAuthAttemptQuery["providerAuthAttempt"]>;

type BootPrimaryConversation = NonNullable<ChatBootQuery["primaryConversation"]>;

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
  if (!isRecord(event) || event.__typename !== "ConversationItemEvent") {
    return false;
  }

  const item = event.item;
  if (!isRecord(item) || item.__typename !== "Activity") {
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function App() {
  return (
    <AppBootBoundary>
      <AppContent />
    </AppBootBoundary>
  );
}

function AppContent() {
  const { route, navigate, goBackFromSettings } = useBrowserRoute();
  const apolloClient = useApolloClient();
  const boot = useSuspenseQuery(ChatBootDocument, {
    variables: { transcriptLimit: 80 },
    fetchPolicy: "network-only"
  });
  const [startProviderAuthAttempt] = useMutation(StartProviderAuthAttemptDocument);
  const onboarding = boot.data.onboardingStatus;
  const refetchOnboarding = boot.refetch;
  const onboarded = onboarding.isUserOnboarded;
  const chatRoute = route.kind === "chat";
  const [ensurePrimaryConversation] = useMutation(EnsurePrimaryConversationDocument);
  const [sendConversationTurn] = useMutation(SendConversationTurnDocument);

  const [socketState, setSocketState] = React.useState<SocketState>("closed");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [agentStatus, setAgentStatus] = React.useState<ConversationAgentStatus>("closed");
  const [authAttempt, setAuthAttempt] = React.useState<ProviderAuthAttemptView | null>(null);
  const [onboardingError, setOnboardingError] = React.useState<string | null>(null);
  const [transcriptWindow, setTranscriptWindow] = React.useState(() => emptyTranscriptWindow());
  const transcript = transcriptWindowEntries(transcriptWindow);
  const [loadingLatestTranscript, setLoadingLatestTranscript] = React.useState(false);
  const [loadingOlderTranscript, setLoadingOlderTranscript] = React.useState(false);
  const [olderTranscriptPageError, setOlderTranscriptPageError] = React.useState<string | null>(null);
  const [latestTranscriptRetryTick, setLatestTranscriptRetryTick] = React.useState(0);
  const [latestTranscriptLoadedConversationId, setLatestTranscriptLoadedConversationId] = React.useState<string | null>(null);
  const [latestTranscriptRetryBlockedConversationId, setLatestTranscriptRetryBlockedConversationId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState("");
  const [pending, setPending] = React.useState(false);
  const [awaitingAssistantTurn, setAwaitingAssistantTurn] = React.useState(false);
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(new Set());
  const [sentMessageScrollRequest, setSentMessageScrollRequest] = React.useState(0);
  const startingConversationRef = React.useRef(false);
  const latestTranscriptLoadedConversationRef = React.useRef<string | null>(null);
  const latestTranscriptRetryBlockedConversationRef = React.useRef<string | null>(null);
  const latestTranscriptRetryTimeoutRef = React.useRef<number | null>(null);
  const latestTranscriptErrorVisibleConversationRef = React.useRef<string | null>(null);
  const lastProcessedConversationEventRef = React.useRef<ConversationEvent | null>(null);
  const localStatusRefetchRef = React.useRef(boot.refetch);

  const status = boot.data.localStatus;
  const agentName = status?.primaryAgentDisplayName ?? null;
  const displayedOnboardingError = onboardingError ?? boot.error?.message ?? null;
  const authAttemptId = authAttempt?.attemptId;
  const authAttemptStatus = authAttempt?.status;

  const refetchOnboardingStatus = React.useCallback(
    () =>
      new Promise<void>((resolve, reject) => {
        React.startTransition(() => {
          void refetchOnboarding().then(
            () => resolve(),
            (error: unknown) => reject(error)
          );
        });
      }),
    [refetchOnboarding]
  );

  const pushTranscriptWindowError = React.useCallback((message: string) => {
    setTranscriptWindow((current) =>
      mergeDurableEntries(
        current,
        [
          {
            id: createClientId(),
            type: "error",
            message,
            recoverable: true
          }
        ],
        {
          placement: "append",
          beforeCursor: current.beforeCursor,
          hasMoreBefore: current.hasMoreBefore
        }
      )
    );
  }, []);

  const reportConversationError = React.useCallback((error: Error) => {
    setSocketState("closed");
    setAgentStatus("closed");
    setPending(false);
    setAwaitingAssistantTurn(false);
    pushTranscriptWindowError(error.message);
  }, [pushTranscriptWindowError]);

  const markConversationConnecting = React.useCallback(() => {
    setSocketState("connecting");
    setAgentStatus("connecting");
  }, []);

  const acceptPrimaryConversation = React.useCallback((nextConversationId: string) => {
    setConversationId(nextConversationId);
    setPending(false);
    setAwaitingAssistantTurn(false);
    setSocketState("ready");
    setAgentStatus("IDLE");
  }, []);

  const acceptBootPrimaryConversation = React.useCallback(
    (primary: BootPrimaryConversation) => {
      acceptPrimaryConversation(primary.conversationId);
      setTranscriptWindow((current) =>
        mergeDurableEntries(current, entriesFromReplay(primary.latestTranscriptPage.items), {
          placement: "latest",
          beforeCursor: primary.latestTranscriptPage.pageInfo.beforeCursor ?? null,
          hasMoreBefore: primary.latestTranscriptPage.pageInfo.hasMoreBefore
        })
      );
      latestTranscriptLoadedConversationRef.current = primary.conversationId;
      latestTranscriptRetryBlockedConversationRef.current = null;
      latestTranscriptErrorVisibleConversationRef.current = null;
      setLatestTranscriptLoadedConversationId(primary.conversationId);
      setLatestTranscriptRetryBlockedConversationId(null);
      if (latestTranscriptRetryTimeoutRef.current !== null) {
        window.clearTimeout(latestTranscriptRetryTimeoutRef.current);
        latestTranscriptRetryTimeoutRef.current = null;
      }
    },
    [acceptPrimaryConversation]
  );

  const conversationEvents = useSubscription(ConversationEventsDocument, {
    variables: { conversationId: conversationId ?? "" },
    skip: !conversationId,
    onError: reportConversationError
  });

  React.useEffect(() => {
    localStatusRefetchRef.current = boot.refetch;
  }, [boot.refetch]);

  const loadConversationTranscriptPage = React.useCallback(
    async ({ cursor, placement }: { cursor: string | null; placement: "latest" | "before" }) => {
      if (!conversationId) {
        return false;
      }
      if (placement === "latest") {
        setLoadingLatestTranscript(true);
      } else {
        setLoadingOlderTranscript(true);
        setOlderTranscriptPageError(null);
      }
      try {
        const result = await apolloClient.query({
          query: ConversationTranscriptPageDocument,
          variables: {
            input: {
              conversationId,
              cursor,
              limit: 80
            }
          },
          fetchPolicy: "network-only"
        });
        const page = result.data?.conversationTranscriptPage;
        if (!page) {
          throw new Error("Noema did not return chat history.");
        }
        setTranscriptWindow((current) =>
          mergeDurableEntries(current, entriesFromReplay(page.items), {
            placement,
            beforeCursor: page.pageInfo.beforeCursor ?? null,
            hasMoreBefore: page.pageInfo.hasMoreBefore
          })
        );
        if (placement === "latest") {
          latestTranscriptRetryBlockedConversationRef.current = null;
          latestTranscriptErrorVisibleConversationRef.current = null;
          setLatestTranscriptLoadedConversationId(conversationId);
          setLatestTranscriptRetryBlockedConversationId(null);
          if (latestTranscriptRetryTimeoutRef.current !== null) {
            window.clearTimeout(latestTranscriptRetryTimeoutRef.current);
            latestTranscriptRetryTimeoutRef.current = null;
          }
        } else {
          setOlderTranscriptPageError(null);
        }
        return true;
      } catch (error: unknown) {
        const message = error instanceof Error ? error.message : "Noema could not load chat history.";
        if (placement === "latest") {
          latestTranscriptLoadedConversationRef.current = null;
          latestTranscriptRetryBlockedConversationRef.current = conversationId;
          setLatestTranscriptLoadedConversationId(null);
          setLatestTranscriptRetryBlockedConversationId(conversationId);
          if (latestTranscriptErrorVisibleConversationRef.current !== conversationId) {
            latestTranscriptErrorVisibleConversationRef.current = conversationId;
            pushTranscriptWindowError(message);
          }
          if (latestTranscriptRetryTimeoutRef.current !== null) {
            window.clearTimeout(latestTranscriptRetryTimeoutRef.current);
          }
          latestTranscriptRetryTimeoutRef.current = window.setTimeout(() => {
            if (latestTranscriptRetryBlockedConversationRef.current === conversationId) {
              latestTranscriptRetryBlockedConversationRef.current = null;
              setLatestTranscriptRetryBlockedConversationId(null);
              setLatestTranscriptRetryTick((current) => current + 1);
            }
            latestTranscriptRetryTimeoutRef.current = null;
          }, 3000);
        } else {
          setOlderTranscriptPageError(message);
        }
        return false;
      } finally {
        if (placement === "latest") {
          setLoadingLatestTranscript(false);
        } else {
          setLoadingOlderTranscript(false);
        }
      }
    },
    [apolloClient, conversationId, pushTranscriptWindowError]
  );

  React.useEffect(() => {
    if (!chatRoute || !onboarded || conversationId || startingConversationRef.current) {
      return;
    }

    let cancelled = false;
    const scheduleConversationState = (update: () => void) => {
      window.queueMicrotask(() => {
        if (!cancelled) {
          update();
        }
      });
    };

    const existing = boot.data.primaryConversation;
    if (existing) {
      scheduleConversationState(() => acceptBootPrimaryConversation(existing));
      return () => {
        cancelled = true;
      };
    }

    startingConversationRef.current = true;
    scheduleConversationState(markConversationConnecting);
    void ensurePrimaryConversation()
      .then((result) => {
        if (cancelled) {
          return;
        }
        const ensured = result.data?.ensurePrimaryConversation;
        if (!ensured) {
          throw new Error("Noema did not return a conversation.");
        }
        acceptPrimaryConversation(ensured.conversationId);
      })
      .catch((error: unknown) => {
        if (cancelled) {
          return;
        }
        setSocketState("closed");
        setAgentStatus("closed");
        setPending(false);
        setAwaitingAssistantTurn(false);
        pushTranscriptWindowError(error instanceof Error ? error.message : "Noema could not start chat.");
      })
      .finally(() => {
        startingConversationRef.current = false;
      });
    return () => {
      cancelled = true;
    };
  }, [
    acceptBootPrimaryConversation,
    acceptPrimaryConversation,
    boot.data.primaryConversation,
    chatRoute,
    conversationId,
    ensurePrimaryConversation,
    markConversationConnecting,
    onboarded,
    pushTranscriptWindowError
  ]);

  React.useEffect(() => {
    return () => {
      if (latestTranscriptRetryTimeoutRef.current !== null) {
        window.clearTimeout(latestTranscriptRetryTimeoutRef.current);
        latestTranscriptRetryTimeoutRef.current = null;
      }
    };
  }, []);

  React.useEffect(() => {
    if (
      !conversationId ||
      loadingLatestTranscript ||
      latestTranscriptLoadedConversationRef.current === conversationId ||
      latestTranscriptRetryBlockedConversationRef.current === conversationId
    ) {
      return;
    }
    const targetConversationId = conversationId;
    void loadConversationTranscriptPage({ cursor: null, placement: "latest" }).then((loaded) => {
      if (loaded) {
        latestTranscriptLoadedConversationRef.current = targetConversationId;
      }
    });
  }, [
    conversationId,
    latestTranscriptRetryTick,
    loadConversationTranscriptPage,
    loadingLatestTranscript
  ]);

  const loadOlderTranscript = React.useCallback(() => {
    if (!transcriptWindow.hasMoreBefore || loadingOlderTranscript || !transcriptWindow.beforeCursor) {
      return;
    }
    void loadConversationTranscriptPage({
      cursor: transcriptWindow.beforeCursor,
      placement: "before"
    });
  }, [
    loadConversationTranscriptPage,
    loadingOlderTranscript,
    transcriptWindow.beforeCursor,
    transcriptWindow.hasMoreBefore
  ]);

  const applyConversationEvent = React.useCallback((event: ConversationEvent) => {
    markConversationEventReceived(event);
    if (isTurnCompletedEvent(event)) {
      setTranscriptWindow((current) => ({
        ...current,
        durableEntries: removeStaleStartedMemoryExtractions(current.durableEntries)
      }));
      setPending(false);
      setAwaitingAssistantTurn(false);
      setAgentStatus("IDLE");
      markConversationEventScheduled(event);
    } else if (isAgentStatusEvent(event)) {
      setAgentStatus(event.status);
      markConversationEventScheduled(event);
    } else if (isAssistantTextDeltaEvent(event)) {
      setAwaitingAssistantTurn(false);
      setTranscriptWindow((current) => ({
        ...current,
        durableEntries: appendAssistantTextDeltaEntry(current.durableEntries, {
          conversationId: event.conversationId,
          turnId: event.deltaTurnId,
          streamId: event.streamId,
          responseIndex: event.responseIndex,
          delta: event.delta
        })
      }));
      markConversationEventScheduled(event);
    } else {
      const entry = entryFromConversationEvent(event);
      if (entry) {
        if (event.__typename === "ConversationItemEvent" && event.clientMessageId && entry.type === "user") {
          setTranscriptWindow((current) => replaceOptimisticEntry(current, event.clientMessageId ?? "", entry));
        } else {
          setTranscriptWindow((current) =>
            mergeDurableEntries(current, [entry], {
              placement: "append",
              beforeCursor: current.beforeCursor,
              hasMoreBefore: current.hasMoreBefore
            })
          );
        }
        markConversationEventScheduled(event, { entryType: entry.type });
      }
    }
    if (shouldRefreshLocalStatusForConversationEvent(event)) {
      React.startTransition(() => {
        void localStatusRefetchRef.current();
      });
    }
  }, []);

  React.useEffect(() => {
    const event = conversationEvents.data?.conversationEvents;
    if (!event) {
      return;
    }
    let cancelled = false;
    window.queueMicrotask(() => {
      if (!cancelled && lastProcessedConversationEventRef.current !== event) {
        lastProcessedConversationEventRef.current = event;
        applyConversationEvent(event);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [applyConversationEvent, conversationEvents.data]);

  async function connectProvider() {
    const step = onboarding.steps.find((candidate) => candidate.id === "connect_provider_account");
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
        await refetchOnboardingStatus();
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
          await refetchOnboardingStatus();
        }
      } catch (error: unknown) {
        setOnboardingError(error instanceof Error ? error.message : "Failed to check provider login");
      }
    },
    [apolloClient, authAttempt?.attemptId, refetchOnboardingStatus]
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

    const clientMessageId = createClientId();
    setDraft("");
    setPending(true);
    setAwaitingAssistantTurn(false);
    setAgentStatus("INPUT_RECEIVED");
    setSentMessageScrollRequest((current) => current + 1);
    setTranscriptWindow((current) => appendOptimisticEntry(current, { id: clientMessageId, type: "user", text: input }));

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
      pushTranscriptWindowError(error instanceof Error ? error.message : "Noema could not send that message.");
    }
  }

  const ready = socketState === "ready" && conversationId !== null;
  const waitingForConversationDecision = chatRoute && onboarded && !conversationId && transcript.length === 0;
  const waitingForInitialTranscript =
    chatRoute &&
    onboarded &&
    conversationId !== null &&
    transcript.length === 0 &&
    latestTranscriptLoadedConversationId !== conversationId &&
    latestTranscriptRetryBlockedConversationId !== conversationId;
  const loadingInitialChat = waitingForConversationDecision || waitingForInitialTranscript;
  const chatView = (
    <ChatSurface
      transcript={transcript}
      loadingOlderTranscript={loadingOlderTranscript}
      hasMoreTranscriptBefore={transcriptWindow.hasMoreBefore}
      olderTranscriptPageError={olderTranscriptPageError}
      pending={pending}
      agentStatus={agentStatus}
      awaitingAssistantTurn={awaitingAssistantTurn}
      expandedActivities={expandedActivities}
      sentMessageScrollRequest={sentMessageScrollRequest}
      draft={draft}
      ready={ready}
      loadingInitialTranscript={loadingInitialChat}
      agentName={agentName}
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
      onLoadOlderTranscript={loadOlderTranscript}
      onSubmit={(value) => void sendMessage(value)}
    />
  );

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
