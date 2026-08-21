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
  SubmitProviderInteractionDocument,
  SendMultipleChoiceSelectionDocument,
  SendConversationTurnDocument,
  StartProviderAuthAttemptDocument,
  TasksEventsDocument,
  TasksTaskReferenceSummaryFieldsFragmentDoc,
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
import type { A2UIActionSubmission, ConversationAgentStatus, SocketState } from "@/shared/types";
import { createClientId } from "@/shared/clientId";
import { isTauriRuntime } from "@/graphql/transportMode";
import { waitForBrowserGraphqlReady } from "@/graphql/browserTransport";
import { pwaRuntime } from "@/pwa/runtime";
import { readChatDraft, writeChatDraft } from "@/pwa/storage";
import { useBrowserGraphqlRecovery } from "./useBrowserGraphqlRecovery";
import { WebPushProvider } from "@/pwa/WebPushContext";
import { PERSONAL_WORKSPACE_ID } from "@/components/tasks/tasksTypes";

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
  const { text, conversationId, socketState, pending } = readiness;
  return Boolean(text.trim() && conversationId && socketState === "ready" && !pending);
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
    if (baseRoute.kind !== "tasks" || !isRecord(location.search)) {
      return baseRoute;
    }

    const projectId = typeof location.search.project === "string"
      ? location.search.project.trim()
      : "";
    return projectId ? { ...baseRoute, projectId } : baseRoute;
  }, [location.pathname, location.search]);
  const apolloClient = useApolloClient();
  const desktopRuntime = isTauriRuntime();
  const pwa = React.useSyncExternalStore(
    pwaRuntime.subscribe,
    pwaRuntime.getSnapshot,
    pwaRuntime.getSnapshot
  );
  const boot = useSuspenseQuery(ChatBootDocument, {
    fetchPolicy: pwa.installed ? "cache-first" : "network-only"
  });
  const [startProviderAuthAttempt] = useMutation(StartProviderAuthAttemptDocument);
  const [cancelProviderAuthAttempt] = useMutation(CancelProviderAuthAttemptDocument);
  const [createProviderAccount, createProviderAccountResult] = useMutation(
    CreateProviderAccountDocument
  );
  const onboarding = boot.data.onboardingStatus;
  const onboarded = onboarding.isUserOnboarded;
  const localSetupResult = useQuery(LocalModelSetupDocument, {
    fetchPolicy: "cache-and-network",
    skip: onboarded
  });
  const [installLocalModel, installLocalModelResult] = useMutation(InstallLocalModelDocument);
  const [cancelLocalModelInstall, cancelLocalModelInstallResult] = useMutation(CancelLocalModelInstallDocument);
  const refetchOnboarding = boot.refetch;
  const refetchBoot = boot.refetch;
  const localSetup = localSetupResult.data?.localModelSetup ?? null;
  const chatRoute = route.kind === "chat";
  const [ensurePrimaryConversation] = useMutation(EnsurePrimaryConversationDocument);
  const [sendConversationTurn] = useMutation(SendConversationTurnDocument);
  const [sendMultipleChoiceSelection] = useMutation(SendMultipleChoiceSelectionDocument);
  const [sendA2UIAction] = useMutation(SubmitProviderInteractionDocument);
  const [socketState, setSocketState] = React.useState<SocketState>("closed");
  const [conversationId, setConversationId] = React.useState<string | null>(null);
  const [agentStatus, setAgentStatus] = React.useState<ConversationAgentStatus>("closed");
  const [authAttempt, setAuthAttempt] = React.useState<ProviderAuthAttemptView | null>(null);
  const [onboardingError, setOnboardingError] = React.useState<string | null>(null);
  const [chosenSetupAccountId, setChosenSetupAccountId] = React.useState<string | null>(null);
  const setupProviderAccountId = chosenSetupAccountId;
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
  const sendInFlightRef = React.useRef(false);
  const [awaitingAssistantTurn, setAwaitingAssistantTurn] = React.useState(false);
  const [expandedActivities, setExpandedActivities] = React.useState<Set<string>>(new Set());
  const [sentMessageScrollRequest, setSentMessageScrollRequest] = React.useState(0);
  const [interventionsRefreshKey, setInterventionsRefreshKey] = React.useState(0);
  const conversationSubscriptionReadyRef = React.useRef(false);
  const startingConversationRef = React.useRef(false);
  const latestTranscriptLoadedConversationRef = React.useRef<string | null>(null);
  const latestTranscriptRetryBlockedConversationRef = React.useRef<string | null>(null);
  const latestTranscriptRetryTimeoutRef = React.useRef<number | null>(null);
  const latestTranscriptErrorVisibleConversationRef = React.useRef<string | null>(null);
  const draftLoadedConversationRef = React.useRef<string | null>(null);
  const reconcilingRecoveryRef = React.useRef(false);
  const localStatusRefetchRef = React.useRef(boot.refetch);

  React.useEffect(() => {
    conversationSubscriptionReadyRef.current = false;
  }, [conversationId]);

  const navigate = React.useCallback(
    (nextRoute: AppRoute) => {
      const replace = shouldReplaceHistoryEntryForNavigation(route, nextRoute);
      const resetScroll = nextRoute.kind !== "chat";
      if (nextRoute.kind === "tasks" && nextRoute.projectId) {
        return routerNavigate({
          to: "/tasks",
          search: { project: nextRoute.projectId },
          replace,
          resetScroll
        });
      }

      return routerNavigate({ to: pathForRoute(nextRoute), replace, resetScroll });
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
    skip: onboarded,
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
    const onlinePwaResume = pwa.installed && pwa.state === "online";
    if (pwa.installed && !onlinePwaResume) {
      void pwaRuntime.recover();
      return;
    }
    reconcilingRecoveryRef.current = true;
    if (!onlinePwaResume) {
      setSocketState("connecting");
      setPending(false);
      setAwaitingAssistantTurn(false);
      setAgentStatus("IDLE");
    }
    latestTranscriptLoadedConversationRef.current = null;
    latestTranscriptRetryBlockedConversationRef.current = null;
    setLatestTranscriptLoadedConversationId(null);
    setLatestTranscriptRetryBlockedConversationId(null);
    setLatestTranscriptRetryTick((current) => current + 1);
    void apolloClient.refetchObservableQueries();
  }, [apolloClient, pwa.installed, pwa.state]);

  const acceptPrimaryConversation = React.useCallback((nextConversationId: string) => {
    setConversationId(nextConversationId);
    setPending(false);
    setAwaitingAssistantTurn(false);
    setSocketState("ready");
    setAgentStatus("IDLE");
  }, []);

  React.useEffect(() => {
    const primaryConversationId = boot.data.primaryConversation?.conversationId;
    if (!conversationId && primaryConversationId) {
      window.queueMicrotask(() => acceptPrimaryConversation(primaryConversationId));
    }
  }, [acceptPrimaryConversation, boot.data.primaryConversation?.conversationId, conversationId]);

  React.useEffect(() => {
    if (!pwa.installed || !conversationId) return;
    let active = true;
    draftLoadedConversationRef.current = null;
    void readChatDraft(conversationId).then((savedDraft) => {
      if (active) {
        draftLoadedConversationRef.current = conversationId;
        setDraft(savedDraft);
      }
    });
    return () => {
      active = false;
    };
  }, [conversationId, pwa.installed]);

  React.useEffect(() => {
    if (!pwa.installed || !conversationId || draftLoadedConversationRef.current !== conversationId) return;
    const timeout = window.setTimeout(() => void writeChatDraft(conversationId, draft), 250);
    return () => window.clearTimeout(timeout);
  }, [conversationId, draft, pwa.installed]);

  React.useEffect(() => {
    if (!pwa.installed || !conversationId) return;
    return pwaRuntime.registerFlusher(() => writeChatDraft(conversationId, draft));
  }, [conversationId, draft, pwa.installed]);

  React.useEffect(() => {
    pwaRuntime.setCriticalOperation("chat-turn", pending || awaitingAssistantTurn);
    return () => pwaRuntime.setCriticalOperation("chat-turn", false);
  }, [awaitingAssistantTurn, pending]);

  React.useEffect(() => {
    const active = authAttempt?.status === "STARTING" || authAttempt?.status === "WAITING_FOR_USER";
    pwaRuntime.setCriticalOperation("provider-auth", active);
    return () => pwaRuntime.setCriticalOperation("provider-auth", false);
  }, [authAttempt?.status]);

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
    async ({
      conversationId: requestedConversationId = conversationId,
      cursor,
      placement
    }: {
      conversationId?: string | null;
      cursor: string | null;
      placement: "latest" | "before";
    }) => {
      if (!requestedConversationId) {
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
              conversationId: requestedConversationId,
              cursor,
              limit: 80
            }
          },
          fetchPolicy: pwa.installed && pwa.state === "offline" ? "cache-first" : "network-only"
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
          latestTranscriptLoadedConversationRef.current = requestedConversationId;
          latestTranscriptRetryBlockedConversationRef.current = null;
          latestTranscriptErrorVisibleConversationRef.current = null;
          setLatestTranscriptLoadedConversationId(requestedConversationId);
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
          latestTranscriptRetryBlockedConversationRef.current = requestedConversationId;
          setLatestTranscriptLoadedConversationId(null);
          setLatestTranscriptRetryBlockedConversationId(requestedConversationId);
          if (latestTranscriptErrorVisibleConversationRef.current !== requestedConversationId) {
            latestTranscriptErrorVisibleConversationRef.current = requestedConversationId;
            pushTranscriptWindowError(message);
          }
          if (latestTranscriptRetryTimeoutRef.current !== null) {
            window.clearTimeout(latestTranscriptRetryTimeoutRef.current);
          }
          latestTranscriptRetryTimeoutRef.current = window.setTimeout(() => {
            if (latestTranscriptRetryBlockedConversationRef.current === requestedConversationId) {
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
    [apolloClient, conversationId, pushTranscriptWindowError, pwa.installed, pwa.state]
  );

  React.useEffect(() => {
    if (!pwa.installed || pwa.state !== "reconciling") return;
    let cancelled = false;
    reconcilingRecoveryRef.current = true;

    void (async () => {
      try {
        await new Promise<void>((resolve) => window.queueMicrotask(resolve));
        if (cancelled) return;
        setSocketState("connecting");
        await waitForBrowserGraphqlReady();
        const refreshedBoot = await refetchBoot();
        const nextConversationId =
          refreshedBoot.data?.primaryConversation?.conversationId ?? conversationId;
        if (nextConversationId) {
          acceptPrimaryConversation(nextConversationId);
          const loaded = await loadConversationTranscriptPage({
            conversationId: nextConversationId,
            cursor: null,
            placement: "latest"
          });
          if (!loaded) {
            throw new Error("Noema could not reconcile chat history.");
          }
        }
        await apolloClient.refetchObservableQueries();
        await pwaRuntime.revalidateRecentQueries();
        if (!cancelled) {
          reconcilingRecoveryRef.current = false;
          setSocketState("ready");
          await pwaRuntime.finishReconciliation();
        }
      } catch {
        if (!cancelled) pwaRuntime.failReconciliation();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [
    acceptPrimaryConversation,
    apolloClient,
    conversationId,
    loadConversationTranscriptPage,
    pwa.installed,
    pwa.state,
    refetchBoot
  ]);

  React.useEffect(() => {
    if (
      !chatRoute ||
      !onboarded ||
      conversationId ||
      startingConversationRef.current ||
      !pwa.canMutate
    ) {
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
    pwa.canMutate,
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
    void loadConversationTranscriptPage({ cursor: null, placement: "latest" });
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
      const reconnecting = conversationSubscriptionReadyRef.current || reconcilingRecoveryRef.current;
      conversationSubscriptionReadyRef.current = true;
      if (reconnecting) {
        reconcilingRecoveryRef.current = true;
        setInterventionsRefreshKey((current) => current + 1);
        void loadConversationTranscriptPage({ cursor: null, placement: "latest" });
      }
      markConversationEventScheduled(event);
    } else if (event.__typename === "HumanInterventionsChangedEvent") {
      setInterventionsRefreshKey((current) => current + 1);
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

  useSubscription(TasksEventsDocument, {
    variables: { workspaceId: PERSONAL_WORKSPACE_ID },
    skip: !chatRoute || !conversationId,
    onData: ({ data }) => {
      const task = data.data?.tasksEvents.task;
      const id = task ? apolloClient.cache.identify(task) : undefined;
      if (!task || !id) return;
      apolloClient.cache.writeFragment({
        id,
        fragment: TasksTaskReferenceSummaryFieldsFragmentDoc,
        data: task
      });
    },
    onError: reportConversationError
  });

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
      return attempt.verificationUrl;
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to start provider login");
      return null;
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

  async function continueOnboardingWithProvider(providerAccountId: string) {
    setOnboardingError(null);
    if (setupProviderAccountId !== providerAccountId) {
      setChosenSetupAccountId(providerAccountId);
      return;
    }
    try {
      await modelSetupResult.refetch({ providerAccountId });
    } catch (error: unknown) {
      setOnboardingError(error instanceof Error ? error.message : "Failed to load model setup");
    }
  }

  async function sendMessage(text: string) {
    const input = text.trim();
    const readiness = { text, conversationId, socketState, pending };
    if (sendInFlightRef.current || !canSendMessage(readiness)) {
      return;
    }

    sendInFlightRef.current = true;
    const clientMessageId = createClientId();
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
            clientMessageId,
            clientTimeZone: Intl.DateTimeFormat().resolvedOptions().timeZone
          }
        }
      });
      setDraft("");
    } catch (error: unknown) {
      setPending(false);
      pushTranscriptWindowError(error instanceof Error ? error.message : "Noema could not send that message.");
    } finally {
      sendInFlightRef.current = false;
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

  async function submitA2UIAction(action: A2UIActionSubmission) {
    if (!conversationId || pending || socketState !== "ready") {
      return;
    }
    const clientMessageId = createClientId();
    setPending(true);
    setAwaitingAssistantTurn(false);
    setAgentStatus("INPUT_RECEIVED");
    setSentMessageScrollRequest((current) => current + 1);
    try {
      await sendA2UIAction({
        variables: {
          input: {
            conversationId,
            interactionId: action.interaction_id,
            expectedRevision: action.expected_revision,
            surfaceId: action.surface_id,
            sourceComponentId: action.source_component_id,
            actionName: action.action_name,
            context: action.context,
            dataModel: action.data_model,
            clientMessageId
          }
        }
      });
    } catch (error: unknown) {
      setPending(false);
      pushTranscriptWindowError(error instanceof Error ? error.message : "Noema could not submit that A2UI action.");
    }
  }

  const ready = socketState === "ready" && conversationId !== null && pwa.canMutate;
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
      offline={pwa.installed && pwa.state === "offline"}
      loadingInitialTranscript={loadingInitialChat}
      agentName={agentName}
      interventionsRefreshKey={interventionsRefreshKey}
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
      onSubmitA2UIAction={(action) => void submitA2UIAction(action)}
      onSubmitMultipleChoiceSelection={(promptItemId, selectedOptionIds) =>
        void sendMultipleChoice(promptItemId, selectedOptionIds)
      }
    />
  );

  if (!onboarding.isUserOnboarded) {
    const modelSetup = modelSetupResult.data?.onboardingModelSetup;
    if (setupProviderAccountId && modelSetup?.providerAccountId === setupProviderAccountId) {
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
          modelSetupAccountId={setupProviderAccountId}
          modelSetupLoading={modelSetupResult.loading}
          onConnect={connectProvider}
          onContinue={(providerAccountId) => void continueOnboardingWithProvider(providerAccountId)}
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
    <WebPushProvider
      installed={pwa.installed}
      chatVisible={chatRoute}
      connectionReady={socketState === "ready" && pwa.state === "online"}
    >
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
          recovery={pwa}
          agentAvatarActivity={shellAgentAvatarActivity}
          onNavigate={navigate}
        >
          {children}
        </AppShell>
      </AppRuntimeProvider>
    </WebPushProvider>
  );
}
