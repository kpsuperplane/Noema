import React from "react";
import {
  useApolloClient,
  useMutation,
  useQuery,
  useSubscription,
  useSuspenseQuery
} from "@apollo/client/react";
import { useLocation, useNavigate } from "@tanstack/react-router";
import {
  ChatBootDocument,
  ConfirmOnboardingModelSelectionsDocument,
  ConversationEventsDocument,
  ConversationTranscriptPageDocument,
  EnsurePrimaryConversationDocument,
  InstallLocalModelDocument,
  LocalModelEventsDocument,
  LocalModelSetupDocument,
  OnboardingModelSetupDocument,
  CancelLocalModelInstallDocument,
  CancelProviderAuthAttemptDocument,
  CreateProviderAccountDocument,
  ProviderAuthAttemptEventsDocument,
  SendMultipleChoiceSelectionDocument,
  SendConversationTurnDocument,
  StartProviderAuthAttemptDocument,
  type ConfirmOnboardingModelSelectionsInput,
  type ProviderAuthAttemptQuery,
  type StartProviderAuthAttemptMutation
} from "@/generated/graphql";
import { ChatSurface } from "@/components/ChatSurface";
import { avatarActivityForAgentStatus } from "@/components/IdentityAvatar";
import { AppShell } from "@/components/shell/AppShell";
import { SetupFrame } from "@/components/shell/SetupFrame";
import { Onboarding } from "@/components/Onboarding";
import { ModelSetup } from "@/components/onboarding/ModelSetup";
import { AppRuntimeProvider } from "./AppRuntimeContext";
import {
  pathForRoute,
  routeFromPathname,
  shouldReplaceHistoryEntryForNavigation,
  type AppRoute
} from "./routes";
import {
  appendAssistantTextDeltaEntry,
  entriesFromReplay,
  entryFromConversationEvent,
  isAgentStatusEvent,
  isAssistantTextDeltaEvent,
  isTurnCompletedEvent,
  markConversationEventReceived,
  markConversationEventScheduled,
  type ConversationEvent
} from "@/transcript/events";
import {
  appendOptimisticEntry,
  completeAssistantStreams,
  emptyTranscriptWindow,
  mergeDurableEntries,
  replaceOptimisticEntry,
  transcriptWindowEntries
} from "@/transcript/window";
import type { ConversationAgentStatus, SocketState } from "@/shared/types";
import { createClientId } from "@/shared/clientId";
import { isTauriRuntime } from "@/graphql/transportMode";
import { useBrowserGraphqlRecovery } from "./useBrowserGraphqlRecovery";

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

export function AppRoot({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  const routerNavigate = useNavigate();
  const route = React.useMemo(() => {
    const baseRoute = routeFromPathname(location.pathname);
    if (baseRoute.kind !== "work" || !isRecord(location.search)) {
      return baseRoute;
    }

    const projectId = typeof location.search.project === "string"
      ? location.search.project.trim()
      : "";
    return projectId ? { ...baseRoute, projectId } : baseRoute;
  }, [location.pathname, location.search]);
  const apolloClient = useApolloClient();
  const desktopRuntime = isTauriRuntime();
  const boot = useSuspenseQuery(ChatBootDocument, {
    fetchPolicy: "network-only"
  });
  const [startProviderAuthAttempt] = useMutation(StartProviderAuthAttemptDocument);
  const [cancelProviderAuthAttempt] = useMutation(CancelProviderAuthAttemptDocument);
  const [createProviderAccount, createProviderAccountResult] = useMutation(
    CreateProviderAccountDocument
  );
  const localSetupResult = useQuery(LocalModelSetupDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [installLocalModel, installLocalModelResult] = useMutation(InstallLocalModelDocument);
  const [cancelLocalModelInstall, cancelLocalModelInstallResult] = useMutation(CancelLocalModelInstallDocument);
  const onboarding = boot.data.onboardingStatus;
  const refetchOnboarding = boot.refetch;
  const localSetup = localSetupResult.data?.localModelSetup ?? null;
  const onboarded = onboarding.isUserOnboarded;
  const chatRoute = route.kind === "chat";
  const [ensurePrimaryConversation] = useMutation(EnsurePrimaryConversationDocument);
  const [sendConversationTurn] = useMutation(SendConversationTurnDocument);
  const [sendMultipleChoiceSelection] = useMutation(SendMultipleChoiceSelectionDocument);
  const [socketState, setSocketState] = React.useState<SocketState>("closed");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [agentStatus, setAgentStatus] = React.useState<ConversationAgentStatus>("closed");
  const [authAttempt, setAuthAttempt] = React.useState<ProviderAuthAttemptView | null>(null);
  const [onboardingError, setOnboardingError] = React.useState<string | null>(null);
  const [chosenSetupAccountId, setChosenSetupAccountId] = React.useState<string | null>();
  const readySetupAccounts = boot.data.providerAccounts.filter(
    (account) => account.status === "AUTHENTICATED" && account.isActive
  );
  const setupProviderAccountId = chosenSetupAccountId === undefined
    ? readySetupAccounts.length === 1
      ? readySetupAccounts[0].providerAccountId
      : null
    : chosenSetupAccountId;
  const modelSetupResult = useQuery(OnboardingModelSetupDocument, {
    variables: { providerAccountId: setupProviderAccountId ?? "" },
    skip: !setupProviderAccountId,
    fetchPolicy: "network-only"
  });
  const [confirmOnboardingModels, confirmOnboardingModelsResult] = useMutation(
    ConfirmOnboardingModelSelectionsDocument
  );
  const [transcriptWindow, setTranscriptWindow] = React.useState(emptyTranscriptWindow);
  const transcript = transcriptWindowEntries(transcriptWindow);
  const [loadingLatestTranscript, setLoadingLatestTranscript] = React.useState(false);
  const [loadingOlderTranscript, setLoadingOlderTranscript] = React.useState(false);
  const [olderTranscriptPageError, setOlderTranscriptPageError] = React.useState<string | null>(null);
  const [latestTranscriptRetryTick, setLatestTranscriptRetryTick] = React.useState(0);
  const [latestTranscriptLoadedConversationId, setLatestTranscriptLoadedConversationId] =
    React.useState<string | null>(null);
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
  const reconcilingRecoveryRef = React.useRef(false);
  const localStatusRefetchRef = React.useRef(boot.refetch);

  const navigate = React.useCallback(
    (nextRoute: AppRoute) => {
      const replace = shouldReplaceHistoryEntryForNavigation(route, nextRoute);
      const resetScroll = nextRoute.kind !== "chat";
      if (nextRoute.kind === "work" && nextRoute.projectId) {
        void routerNavigate({
          to: "/work",
          search: { project: nextRoute.projectId },
          replace,
          resetScroll
        });
        return;
      }

      void routerNavigate({ to: pathForRoute(nextRoute), replace, resetScroll });
    },
    [route, routerNavigate]
  );

  const status = boot.data.localStatus;
  const agentName = status?.primaryAgentDisplayName ?? null;
  const displayedOnboardingError = onboardingError ?? boot.error?.message ?? null;
  const authAttemptId = authAttempt?.attemptId;

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

  const refreshLocalSetup = React.useCallback(async () => {
    const result = await localSetupResult.refetch();
    if (result.data?.localModelSetup.isReady) {
      await refetchOnboardingStatus();
    }
  }, [localSetupResult, refetchOnboardingStatus]);

  useSubscription(LocalModelEventsDocument, {
    onData: () => void refreshLocalSetup()
  });

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

  const markConversationReady = React.useCallback(() => {
    setSocketState("ready");
  }, []);

  const recoverBrowserConversation = React.useCallback(() => {
    reconcilingRecoveryRef.current = true;
    setSocketState("connecting");
    setPending(false);
    setAwaitingAssistantTurn(false);
    setAgentStatus("IDLE");
    latestTranscriptLoadedConversationRef.current = null;
    latestTranscriptRetryBlockedConversationRef.current = null;
    setLatestTranscriptLoadedConversationId(null);
    setLatestTranscriptRetryBlockedConversationId(null);
    setLatestTranscriptRetryTick((current) => current + 1);
    void apolloClient.refetchObservableQueries();
  }, [apolloClient]);

  const acceptPrimaryConversation = React.useCallback((nextConversationId: string) => {
    setConversationId(nextConversationId);
    setPending(false);
    setAwaitingAssistantTurn(false);
    setSocketState("ready");
    setAgentStatus("IDLE");
  }, []);

  useBrowserGraphqlRecovery({
    enabled: !desktopRuntime && conversationId !== null,
    onConnecting: markConversationConnecting,
    onReady: markConversationReady,
    onRecovered: recoverBrowserConversation
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
        const reconcileOptimisticEntries = placement === "latest" && reconcilingRecoveryRef.current;
        setTranscriptWindow((current) => {
          const merged = mergeDurableEntries(current, entriesFromReplay(page.items), {
            placement,
            beforeCursor: page.pageInfo.beforeCursor ?? null,
            hasMoreBefore: page.pageInfo.hasMoreBefore
          });
          return reconcileOptimisticEntries ? { ...merged, optimisticEntries: [] } : merged;
        });
        if (placement === "latest") {
          latestTranscriptRetryBlockedConversationRef.current = null;
          latestTranscriptErrorVisibleConversationRef.current = null;
          setLatestTranscriptLoadedConversationId(conversationId);
          setLatestTranscriptRetryBlockedConversationId(null);
          if (latestTranscriptRetryTimeoutRef.current !== null) {
            window.clearTimeout(latestTranscriptRetryTimeoutRef.current);
            latestTranscriptRetryTimeoutRef.current = null;
          }
          if (reconcileOptimisticEntries) {
            reconcilingRecoveryRef.current = false;
            setSocketState("ready");
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
    acceptPrimaryConversation,
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
    if (event.__typename === "SubscriptionReadyEvent") {
      reconcilingRecoveryRef.current = true;
      void loadConversationTranscriptPage({ cursor: null, placement: "latest" });
      markConversationEventScheduled(event);
    } else if (isTurnCompletedEvent(event)) {
      setPending(false);
      setAwaitingAssistantTurn(false);
      setAgentStatus("IDLE");
      setTranscriptWindow(completeAssistantStreams);
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
        const replacesSubmittedInput =
          entry.type === "user" ||
          (entry.type === "activity" && entry.item.activity_kind === "context_reset");
        if (event.__typename === "ConversationItemEvent" && event.clientMessageId && replacesSubmittedInput) {
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
  }, [loadConversationTranscriptPage]);

  useSubscription(ConversationEventsDocument, {
    variables: { conversationId: conversationId ?? "" },
    skip: !conversationId,
    onData: ({ data }) => {
      const event = data.data?.conversationEvents;
      if (event) {
        applyConversationEvent(event);
      }
    },
    onError: reportConversationError
  });

  async function connectProvider(providerKind: string, method: "OAUTH_PKCE" | "OAUTH_DEVICE_CODE") {
    setOnboardingError(null);
    try {
      const result = await startProviderAuthAttempt({
        variables: {
          input: {
            providerKind,
            method
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
        setChosenSetupAccountId(attempt.providerAccountId);
      }
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to start provider login");
    }
  }

  async function connectOpenRouterApiKey(secret: string) {
    setOnboardingError(null);
    try {
      const result = await createProviderAccount({
        variables: {
          input: {
            providerKind: "openrouter",
            authMethod: "SECRET_INPUT",
            secret
          }
        }
      });
      await refetchOnboardingStatus();
      const accountId = result.data?.createProviderAccount.providerAccountId;
      if (accountId) setChosenSetupAccountId(accountId);
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to connect OpenRouter");
    }
  }

  async function installRecommendedLocalModel(modelId: string, file?: string | null) {
    try {
      await installLocalModel({ variables: { input: { modelId, file } } });
      await refreshLocalSetup();
    } catch {
      // Apollo exposes the mutation error in the onboarding surface.
    }
  }

  async function cancelLocalModelDownload(installationId: string) {
    try {
      await cancelLocalModelInstall({ variables: { installationId } });
      await refreshLocalSetup();
    } catch {
      // Apollo exposes the mutation error in the onboarding surface.
    }
  }

  useSubscription(ProviderAuthAttemptEventsDocument, {
    variables: { attemptId: authAttemptId ?? "" },
    skip: !authAttemptId,
    onData: ({ data }) => {
      const next = data.data?.providerAuthAttemptEvents;
      if (!next) return;
      setAuthAttempt(next);
      if (next.status === "COMPLETED") {
        void refetchOnboardingStatus().then(() => {
          setChosenSetupAccountId(next.providerAccountId);
        });
      }
    },
    onError: (error) => setOnboardingError(error.message)
  });

  async function cancelCurrentProviderAuth() {
    if (!authAttemptId) return;
    await cancelProviderAuthAttempt({ variables: { input: { attemptId: authAttemptId } } });
    setAuthAttempt(null);
  }

  async function confirmModels(input: ConfirmOnboardingModelSelectionsInput) {
    setOnboardingError(null);
    try {
      await confirmOnboardingModels({ variables: { input } });
      await refetchOnboardingStatus();
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to save model setup");
    }
  }

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

  async function sendMultipleChoice(promptItemId: string, selectedOptionIds: string[]) {
    if (!conversationId || pending || socketState !== "ready" || selectedOptionIds.length === 0) {
      return;
    }

    const clientMessageId = createClientId();
    setPending(true);
    setAwaitingAssistantTurn(false);
    setAgentStatus("INPUT_RECEIVED");
    setSentMessageScrollRequest((current) => current + 1);

    try {
      await sendMultipleChoiceSelection({
        variables: {
          input: {
            conversationId,
            promptItemId,
            selectedOptionIds,
            clientMessageId
          }
        }
      });
    } catch (error: unknown) {
      setPending(false);
      pushTranscriptWindowError(error instanceof Error ? error.message : "Noema could not send that selection.");
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
  const runtimeAgentAvatarActivity = avatarActivityForAgentStatus(agentStatus);
  const shellAgentAvatarActivity =
    runtimeAgentAvatarActivity === "idle" && chatRoute && draft.length > 0
      ? "listening"
      : runtimeAgentAvatarActivity;
  const chatView = (
    <ChatSurface
      conversationId={conversationId}
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
      onSubmitMultipleChoiceSelection={(promptItemId, selectedOptionIds) =>
        void sendMultipleChoice(promptItemId, selectedOptionIds)
      }
    />
  );

  if (!onboarding.isUserOnboarded) {
    const modelSetup = modelSetupResult.data?.onboardingModelSetup;
    if (setupProviderAccountId && modelSetup) {
      return (
        <SetupFrame>
          <ModelSetup
            key={modelSetup.providerAccountId}
            setup={modelSetup}
            saving={confirmOnboardingModelsResult.loading}
            error={modelSetupResult.error?.message ?? displayedOnboardingError}
            onConfirm={(input) => void confirmModels(input)}
            onUseDifferentProvider={() => {
              setChosenSetupAccountId(null);
              setOnboardingError(null);
            }}
          />
        </SetupFrame>
      );
    }
    return (
      <SetupFrame>
        <Onboarding
          onboarding={onboarding}
          providerCatalog={boot.data.providerAccountCatalog}
          connectedAccounts={boot.data.providerAccounts}
          localSetup={localSetup}
          localSetupLoading={localSetupResult.loading && !localSetupResult.data}
          localSetupError={localSetupResult.error?.message ?? null}
          localSaving={installLocalModelResult.loading || cancelLocalModelInstallResult.loading}
          localSaveError={
            installLocalModelResult.error?.message ??
            cancelLocalModelInstallResult.error?.message ??
            null
          }
          providerSaving={createProviderAccountResult.loading}
          attempt={authAttempt}
          error={modelSetupResult.error?.message ?? displayedOnboardingError}
          onConnect={(providerKind, method) => void connectProvider(providerKind, method)}
          onContinue={setChosenSetupAccountId}
          onConnectOpenRouterApiKey={(secret) => void connectOpenRouterApiKey(secret)}
          onCancelProviderAuth={() => void cancelCurrentProviderAuth()}
          onInstallLocal={(modelId, file) => void installRecommendedLocalModel(modelId, file)}
          onCancelLocal={(installationId) => void cancelLocalModelDownload(installationId)}
          onRetry={() => {
            setAuthAttempt(null);
            setOnboardingError(null);
          }}
        />
      </SetupFrame>
    );
  }

  return (
    <AppRuntimeProvider
      value={{
        chatView,
        settingsSection: route.kind === "settings" ? route.section : null
      }}
    >
      <AppShell
        route={route}
        status={status}
        socketState={socketState}
        agentAvatarActivity={shellAgentAvatarActivity}
        onNavigate={navigate}
      >
        {children}
      </AppShell>
    </AppRuntimeProvider>
  );
}
