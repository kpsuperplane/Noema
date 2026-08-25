import * as React from "react";
import type { OperationVariables } from "@apollo/client";
import { useLazyQuery, useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { ChevronLeft, ChevronRight, Code2 } from "lucide-react";
import { AnimatePresence, useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { getOperationAST, print, type DocumentNode } from "graphql";
import {
  PendingHumanInterventionsDocument,
  ApproveAdapterDefinitionDocument,
  CancelAdapterDefinitionDocument,
  SetupAdapterConnectionDocument,
  StartAdapterOauthSetupDocument,
  AdapterOauthAttemptDocument,
  AdapterOauthAttemptEventsDocument,
  ImportAdapterOauthApplicationDocument,
  AttachAdapterOauthConnectionDocument,
  SaveCapabilityConnectionPolicyDocument,
  type PendingHumanInterventionsQuery,
  type SaveCapabilityConnectionPolicyMutation
} from "@/generated/graphql";
import {
  CapabilityPolicyChoices,
  type CapabilityDataSharingPolicy,
  type CapabilityUnsafeActionPolicy
} from "@/components/capabilities/CapabilityPolicyChoices";
import { AdapterDefinitionReviewDetails } from "@/components/capabilities/AdapterDefinitionReviewDetails";
import { AdapterCredentialSetupDialog, type AdapterCredentialSubmission } from "@/components/capabilities/AdapterCredentialSetupDialog";
import {
  openExternalUrlForAuth,
  reserveExternalAuthNavigation
} from "@/graphql/externalUrls";
import { McpChatSetupCard } from "@/components/mcp/McpChatSetupCard";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { RollingSwap } from "@/components/RollingText";
import { springs } from "@/motion/springs";
import { pwaRuntime } from "@/pwa/runtime";
import { HumanInterventionCard } from "./HumanInterventionCard";
import { RenderErrorBoundary } from "@/components/errors/RenderErrorBoundary";
import {
  AdapterAuthenticationCard,
  GovernedActionCard,
  McpAuthenticationCard,
  TaskGateInterventionCard
} from "./HumanInterventionDecisionCards";

export type PendingHumanIntervention = PendingHumanInterventionsQuery["pendingHumanInterventions"][number];
type PendingAdapterDefinition = Extract<PendingHumanIntervention, { __typename: "AdapterDefinition" }>;
type PendingOauthClientSetup = Extract<PendingHumanIntervention, { __typename: "AdapterOauthClientSetupIntervention" }>;

type Scope = {
  conversationId?: string | null;
  taskId?: string;
  projectId?: string;
  refreshKey?: number;
};

export type HumanInterventionPlacement = "chat" | "dock" | "queue" | "task";
type PendingHumanInterventionsResultLike = {
  data?: PendingHumanInterventionsQuery | null;
  error?: unknown;
  loading: boolean;
  observable?: { options: { query: DocumentNode } };
  refetch: () => Promise<unknown>;
  variables?: OperationVariables;
};

const dismissedAdapterSetupsKey = "noema.dismissed-adapter-setups";

export function usePendingHumanInterventions(scope: Scope = {}) {
  const result = useQuery(PendingHumanInterventionsDocument, {
    variables: {
      conversationId: scope.conversationId ?? undefined,
      taskId: scope.taskId,
      projectId: scope.projectId,
      first: 50
    },
    skip: scope.conversationId === null,
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true
  });
  const previousRefreshKey = React.useRef(scope.refreshKey);
  const refetch = result.refetch;
  React.useEffect(() => {
    if (scope.refreshKey === undefined || previousRefreshKey.current === scope.refreshKey) return;
    previousRefreshKey.current = scope.refreshKey;
    void refetch();
  }, [refetch, scope.refreshKey]);
  return result;
}

export function PendingHumanInterventions({
  conversationId,
  taskId,
  projectId,
  refreshKey,
  placement = "chat",
  emptyContent
}: Scope & { placement?: HumanInterventionPlacement; emptyContent?: React.ReactNode }) {
  const result = usePendingHumanInterventions({ conversationId, taskId, projectId, refreshKey });
  return (
    <PendingHumanInterventionsResult
      conversationId={conversationId}
      placement={placement}
      emptyContent={emptyContent}
      result={result}
    />
  );
}

export function PendingHumanInterventionsResult({
  conversationId,
  placement = "chat",
  emptyContent,
  result
}: {
  conversationId?: string | null;
  placement?: HumanInterventionPlacement;
  emptyContent?: React.ReactNode;
  result: PendingHumanInterventionsResultLike;
}) {
  const query = result.observable?.options.query;
  const querySource = React.useMemo(() => query ? print(query) : "", [query]);
  const recoveryKey = `${querySource}:${JSON.stringify(result.variables ?? {})}`;
  const reloadGeneration = React.useRef(0);
  const [recoveredData, setRecoveredData] = React.useState<{
    key: string;
    data: PendingHumanInterventionsQuery;
    generation: number;
    apolloData: PendingHumanInterventionsQuery | null | undefined;
    apolloError: unknown;
  } | null>(null);
  const currentRecoveredData = recoveredData?.key === recoveryKey
    && recoveredData.apolloData === result.data
    && recoveredData.apolloError === result.error
    ? recoveredData.data
    : null;
  const effectiveData = currentRecoveredData ?? result.data;
  const interventions = effectiveData?.pendingHumanInterventions ?? [];
  const invalidAdapterAuthentication = interventions.some((intervention) => (
    intervention.__typename === "AdapterAuthenticationIntervention"
      && (typeof intervention.serviceDisplayName !== "string"
        || !intervention.serviceDisplayName.trim())
  ));
  const stale = Boolean(result.error) && !currentRecoveredData;
  const [dismissedAdapterSetups, setDismissedAdapterSetups] = React.useState(readDismissedAdapterSetups);
  const [selectedChatInterventionKey, setSelectedChatInterventionKey] = React.useState<string | null>(null);
  const allowAdapterSetupDismissal = Boolean(conversationId) && placement === "chat";
  const availableInterventions = allowAdapterSetupDismissal
    ? interventions.filter((intervention) => (
        intervention.__typename === "AdapterOauthClientSetupIntervention"
          ? !dismissedAdapterSetups.has(`oauth:${intervention.profileDigest}`)
          : intervention.__typename !== "AdapterDefinition"
            || !intervention.reviewed
            || intervention.connectionCount > 0
            || !dismissedAdapterSetups.has(intervention.semanticDigest)
      ))
    : interventions;
  const visibleInterventions = availableInterventions.filter((intervention) => (
    intervention.__typename !== "AdapterAuthenticationIntervention"
      || (!stale && typeof intervention.serviceDisplayName === "string"
        && Boolean(intervention.serviceDisplayName.trim()))
  ));
  const selectedChatInterventionIndex = Math.max(0, visibleInterventions.findIndex(
    (intervention) => humanInterventionKey(intervention) === selectedChatInterventionKey
  ));
  const presentedInterventions = placement === "chat"
    ? visibleInterventions.slice(selectedChatInterventionIndex, selectedChatInterventionIndex + 1)
    : visibleInterventions;
  const dismissAdapterSetup = React.useCallback((semanticDigest: string) => {
    setDismissedAdapterSetups((current) => {
      const next = new Set(current).add(semanticDigest);
      writeDismissedAdapterSetups(next);
      return next;
    });
  }, []);
  const reload = React.useCallback(async () => {
    const generation = ++reloadGeneration.current;
    setRecoveredData(null);
    if (!query) {
      await result.refetch();
      return;
    }
    const response = await fetch("/graphql", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        operationName: getOperationAST(query)?.name?.value,
        query: querySource,
        variables: result.variables
      })
    });
    if (response.status === 401) {
      pwaRuntime.requireAuthentication();
    }
    if (!response.ok) throw new Error("Noema could not load response options.");
    const payload = await response.json() as {
      data?: PendingHumanInterventionsQuery;
      errors?: unknown[];
    };
    const data = payload.data;
    if (payload.errors?.length || !data || !Array.isArray(data.pendingHumanInterventions)) {
      throw new Error("Noema did not return response options.");
    }
    setRecoveredData((current) => current && current.generation > generation
      ? current
      : {
          key: recoveryKey,
          data,
          generation,
          apolloData: result.data,
          apolloError: result.error
        });
  }, [query, querySource, recoveryKey, result]);
  const retry = React.useCallback(async () => {
    try {
      await reload();
    } catch {
      // Keep the recovery surface available for another attempt.
    }
  }, [reload]);
  const listContent = visibleInterventions.length ? (
    <VStack gap={0}>
      {placement === "chat" && visibleInterventions.length > 1 ? (
        <HStack hAlign="between" vAlign="center" gap={1} {...stylex.props(styles.queueNavigation)}>
          <span aria-live="polite" {...stylex.props(styles.queuePosition)}>
            {selectedChatInterventionIndex + 1} of {visibleInterventions.length} waiting
          </span>
          <HStack gap={1}>
            <IconButton
              type="button"
              size="sm"
              variant="ghost"
              label="Previous request"
              tooltip="Previous request"
              icon={<ChevronLeft aria-hidden="true" size={15} />}
              isDisabled={selectedChatInterventionIndex === 0}
              xstyle={styles.queueAction}
              onClick={() => {
                const previous = visibleInterventions[selectedChatInterventionIndex - 1];
                if (previous) setSelectedChatInterventionKey(humanInterventionKey(previous));
              }}
            />
            <IconButton
              type="button"
              size="sm"
              variant="ghost"
              label="Next request"
              tooltip="Next request"
              icon={<ChevronRight aria-hidden="true" size={15} />}
              isDisabled={selectedChatInterventionIndex === visibleInterventions.length - 1}
              xstyle={styles.queueAction}
              onClick={() => {
                const next = visibleInterventions[selectedChatInterventionIndex + 1];
                if (next) setSelectedChatInterventionKey(humanInterventionKey(next));
              }}
            />
          </HStack>
        </HStack>
      ) : null}
      <HumanInterventionList
        interventions={presentedInterventions}
        placement={placement}
        onResolved={() => void reload().catch(() => undefined)}
        onDismissAdapterSetup={allowAdapterSetupDismissal ? dismissAdapterSetup : undefined}
        initialAnimation={false}
      />
    </VStack>
  ) : null;
  const list = listContent ? (
    <HumanInterventionMotionItem key="pending-human-interventions">
      {listContent}
    </HumanInterventionMotionItem>
  ) : null;
  const queryError = stale || invalidAdapterAuthentication ? (
    <Banner
      status="error"
      title="Response options unavailable"
      description="Try again to load the current options."
      endContent={(
        <Button
          type="button"
          size="sm"
          variant="ghost"
          label="Retry"
          clickAction={retry}
        />
      )}
      xstyle={styles.queryError}
    />
  ) : null;
  const empty = effectiveData
    && !stale
    && !invalidAdapterAuthentication
    && visibleInterventions.length === 0
    ? emptyContent
    : null;

  if (placement === "dock") {
    const interventionContent = stale && listContent ? (
      <fieldset disabled {...stylex.props(styles.staleInterventions)}>
        {listContent}
      </fieldset>
    ) : listContent;
    const content = interventionContent ?? (
      empty ? <div {...stylex.props(styles.dockStatus)}>{empty}</div> : null
    );
    return (
      <>
        <AnimatePresence initial={false}>
          {content ? (
            <HumanInterventionMotionItem key="task-secondary-surface" slide>
              <RollingSwap
                transitionKey={interventionContent ? "intervention" : "status"}
                style={{ overflow: "visible" }}
              >
                {content}
              </RollingSwap>
            </HumanInterventionMotionItem>
          ) : null}
        </AnimatePresence>
        {queryError}
      </>
    );
  }

  return (
    <>
      {stale && list ? (
        <fieldset disabled {...stylex.props(styles.staleInterventions)}>
          <AnimatePresence>{list}</AnimatePresence>
        </fieldset>
      ) : (
        <AnimatePresence>{list}</AnimatePresence>
      )}
      {queryError}
      {empty}
    </>
  );
}

export function pendingHumanInterventionsAreFresh(
  result: PendingHumanInterventionsResultLike
) {
  return Boolean(result.data) && !result.error;
}

export function HumanInterventionList({
  interventions,
  placement = "chat",
  onResolved,
  onDismissAdapterSetup,
  animateItems = true,
  initialAnimation = true
}: {
  interventions: PendingHumanIntervention[];
  placement?: HumanInterventionPlacement;
  onResolved?: () => void;
  onDismissAdapterSetup?: (semanticDigest: string) => void;
  animateItems?: boolean;
  initialAnimation?: boolean;
}) {
  return (
    <VStack
      as="section"
      aria-label="Items waiting for you"
      data-slot="human-intervention-list"
      gap={placement !== "chat" ? 3 : 2}
      className={stylex.props(
        styles.list,
        placement !== "chat" && styles.nonChatList,
        placement === "dock" && styles.dockList,
        placement === "task" && styles.taskList
      ).className}
    >
      <AnimatePresence initial={initialAnimation}>
        {interventions.map((intervention) => {
          const key = humanInterventionKey(intervention);
          const item = (
            <RenderErrorBoundary
              errorScope={`intervention.${key}`}
              resetKey={intervention}
              fallback={({ retry }) => (
                <InterventionCardShell
                  copy={(
                    <span role="alert">
                      This request could not display. Other requests remain available.
                    </span>
                  )}
                  actions={(
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      label="Retry request"
                      onClick={retry}
                    />
                  )}
                />
              )}
            >
              <HumanInterventionListItem
                intervention={intervention}
                onResolved={onResolved}
                onDismissAdapterSetup={onDismissAdapterSetup}
              />
            </RenderErrorBoundary>
          );
          return animateItems ? (
            <HumanInterventionMotionItem key={key}>{item}</HumanInterventionMotionItem>
          ) : (
            <React.Fragment key={key}>{item}</React.Fragment>
          );
        })}
      </AnimatePresence>
    </VStack>
  );
}

export function HumanInterventionMotionItem({
  children,
  exitGap,
  role,
  slide = false
}: {
  children: React.ReactNode;
  exitGap?: string;
  role?: React.AriaRole;
  slide?: boolean;
}) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  const collapsed = {
    height: 0,
    opacity: 0,
    marginBottom: exitGap ? `calc(-1 * ${exitGap})` : "calc(-1 * var(--human-intervention-motion-gap, 0px))",
    y: slide ? "var(--spacing-2)" : 0
  };
  return (
    <m.div
      aria-hidden={!isPresent}
      inert={!isPresent}
      role={role}
      layout={reduceMotion ? false : "position"}
      initial={reduceMotion ? false : collapsed}
      animate={{ height: "auto", opacity: 1, marginBottom: "0px", y: 0 }}
      exit={collapsed}
      transition={reduceMotion ? { duration: 0 } : springs.standard}
      {...stylex.props(styles.motionItem)}
    >
      {children}
    </m.div>
  );
}

function HumanInterventionListItem({
  intervention,
  onResolved,
  onDismissAdapterSetup
}: {
  intervention: PendingHumanIntervention;
  onResolved?: () => void;
  onDismissAdapterSetup?: (semanticDigest: string) => void;
}) {
  if (intervention.__typename === "TaskAttention") {
    return <TaskGateInterventionCard attention={intervention} onResolved={onResolved} />;
  }
  if (intervention.__typename === "GovernedAction") {
    return <GovernedActionCard action={intervention} onResolved={onResolved} />;
  }
  if (intervention.__typename === "McpAuthenticationIntervention") {
    return <McpAuthenticationCard request={intervention} onResolved={onResolved} />;
  }
  if (intervention.__typename === "McpSetupIntervention") {
    return <McpChatSetupCard setup={intervention} onResolved={onResolved} />;
  }
  if (intervention.__typename === "AdapterAuthenticationIntervention") {
    return <AdapterAuthenticationCard request={intervention} onResolved={onResolved} />;
  }
  if (intervention.__typename === "AdapterOauthClientSetupIntervention") {
    return (
      <OauthClientSetupCard
        setup={intervention}
        onResolved={onResolved}
        onDismiss={onDismissAdapterSetup
          ? () => onDismissAdapterSetup(`oauth:${intervention.profileDigest}`)
          : undefined}
      />
    );
  }
  return (
    <AdapterDefinitionCard
      definition={intervention}
      onResolved={onResolved}
      onDismiss={intervention.reviewed && onDismissAdapterSetup
        ? () => onDismissAdapterSetup(intervention.semanticDigest)
        : undefined}
    />
  );
}

export function humanInterventionKey(intervention: PendingHumanIntervention) {
  switch (intervention.__typename) {
    case "TaskAttention": return `${intervention.task.taskId}:${intervention.gate?.gateId ?? intervention.kind}`;
    case "GovernedAction": return `${intervention.actionId}:${intervention.revision}`;
    case "McpAuthenticationIntervention":
    case "AdapterAuthenticationIntervention": return `${intervention.requestId}:${intervention.revision}`;
    case "McpSetupIntervention": return intervention.itemId;
    case "AdapterOauthClientSetupIntervention": return `oauth:${intervention.profileDigest}`;
    case "AdapterDefinition": return intervention.semanticDigest;
  }
}

function OauthClientSetupCard({
  setup,
  onResolved,
  onDismiss
}: {
  setup: PendingOauthClientSetup;
  onResolved?: () => void;
  onDismiss?: () => void;
}) {
  const [importApplication, applicationImport] = useMutation(ImportAdapterOauthApplicationDocument);
  const [dialogOpen, setDialogOpen] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const openSetupUrl = async () => {
    const handled = await openExternalUrlForAuth(setup.oauthCredentialSetup.setupUrl);
    if (!handled) window.open(setup.oauthCredentialSetup.setupUrl, "_blank", "noopener,noreferrer");
  };
  const importCredentials = async (submission: AdapterCredentialSubmission) => {
    setError(null);
    try {
      if (!submission.document) throw new Error("Select an OAuth client document.");
      const documentBase64 = encodeBase64(new Uint8Array(await submission.document.arrayBuffer()));
      await importApplication({ variables: { input: {
        profileDigest: setup.profileDigest,
        projectLabel: null,
        clientDocumentBase64: documentBase64
      } } });
      setDialogOpen(false);
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The OAuth client could not be imported.");
      throw caught;
    }
  };
  return (
    <InterventionCardShell
      dismissLabel="Hide OAuth client setup from chat"
      onDismiss={onDismiss}
      copy={
        <VStack gap={3} className={stylex.props(styles.copy).className}>
          <VStack gap={1}>
            <span {...stylex.props(styles.eyebrow)}>Shared OAuth setup</span>
            <strong {...stylex.props(styles.summary, styles.adapterSummary)}>Import one OAuth client</strong>
            <span {...stylex.props(styles.context)}>
              Noema will reuse this client for {countLabel(setup.dependentDefinitions.length, "reviewed API")}.
            </span>
          </VStack>
          <VStack as="ul" gap={1} className={stylex.props(styles.operationList).className}>
            {setup.dependentDefinitions.map((definition) => (
              <li key={definition.semanticDigest} {...stylex.props(styles.operationName)}>
                {definition.displayName}
              </li>
            ))}
          </VStack>
          {setup.oauthCredentialSetup.redirectUri ? (
            <VStack gap={1}>
              <span {...stylex.props(styles.context)}><b>Authorized redirect URI</b></span>
              <code {...stylex.props(styles.redirectUriValue)}>{setup.oauthCredentialSetup.redirectUri}</code>
            </VStack>
          ) : null}
          <AdapterCredentialSetupDialog
            title="Import OAuth client"
            serviceName={setup.displayName}
            setup={setup.oauthCredentialSetup}
            scopes={[]}
            open={dialogOpen}
            submitting={applicationImport.loading}
            error={error}
            onOpenChange={(open) => {
              if (!applicationImport.loading) setDialogOpen(open);
            }}
            intro="Import this client document once. Noema will reuse it for every compatible reviewed API."
            submitLabel="Import OAuth client"
            onSubmit={importCredentials}
          />
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
        </VStack>
      }
      actions={
        <HStack gap={1} justify="end" className={stylex.props(styles.actions).className}>
          <Button
            size="sm"
            variant="ghost"
            label="Developer Tools"
            isDisabled={applicationImport.loading}
            onClick={() => void openSetupUrl()}
          />
          <Button
            size="sm"
            variant="primary"
            label="Import OAuth client"
            isLoading={applicationImport.loading}
            isDisabled={applicationImport.loading}
            onClick={() => setDialogOpen(true)}
          />
        </HStack>
      }
    />
  );
}

function AdapterDefinitionCard({
  definition,
  onResolved,
  onDismiss
}: {
  definition: PendingAdapterDefinition;
  onResolved?: () => void;
  onDismiss?: () => void;
}) {
  const [approveDefinition, approval] = useMutation(ApproveAdapterDefinitionDocument);
  const [cancelDefinition, cancellation] = useMutation(CancelAdapterDefinitionDocument);
  const [setupConnection, credentialSetup] = useMutation(SetupAdapterConnectionDocument);
  const [importApplication, applicationImport] = useMutation(ImportAdapterOauthApplicationDocument);
  const [attachGrant, grantAttach] = useMutation(AttachAdapterOauthConnectionDocument);
  const [startOauth, oauthStart] = useMutation(StartAdapterOauthSetupDocument);
  const [loadOauthAttempt] = useLazyQuery(AdapterOauthAttemptDocument, {
    fetchPolicy: "network-only"
  });
  const [savePolicy, policySave] = useMutation<SaveCapabilityConnectionPolicyMutation>(
    SaveCapabilityConnectionPolicyDocument
  );
  const [error, setError] = React.useState<string | null>(null);
  const [authorizing, setAuthorizing] = React.useState(false);
  const [authorizationExpiry, setAuthorizationExpiry] = React.useState<number | null>(null);
  const [oauthAttemptId, setOauthAttemptId] = React.useState<string | null>(null);
  const [oauthAttemptNeedsAttach, setOauthAttemptNeedsAttach] = React.useState(false);
  const finishingOauthAttempt = React.useRef(false);
  const [technicalDetailsOpen, setTechnicalDetailsOpen] = React.useState(false);
  const [credentialSetupOpen, setCredentialSetupOpen] = React.useState(false);
  const connection = definition.connections.find(
    (candidate) => candidate.status === "authentication_required"
  );
  const policyConnection = definition.connections.find(
    (candidate) => candidate.status === "active" && !candidate.policyConfigured
  );
  const [sharing, setSharing] = React.useState<CapabilityDataSharingPolicy>("allow_automatically");
  const [unsafeActions, setUnsafeActions] = React.useState<CapabilityUnsafeActionPolicy>("reviewer_may_approve");
  const [policyStep, setPolicyStep] = React.useState<"sharing" | "unsafe_actions">("sharing");
  const approve = async () => {
    setError(null);
    try {
      await approveDefinition({
        variables: { input: { semanticDigest: definition.semanticDigest } }
      });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The connection definition could not be approved.");
    }
  };
  const cancel = async () => {
    setError(null);
    try {
      await cancelDefinition({
        variables: { input: { semanticDigest: definition.semanticDigest } }
      });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The connection proposal could not be cancelled.");
    }
  };
  const openUrl = async (url: string) => {
    const handled = await openExternalUrlForAuth(url);
    if (!handled) window.open(url, "_blank", "noopener,noreferrer");
  };
  const importCredentials = async (submission: AdapterCredentialSubmission) => {
    setError(null);
    try {
      const documentBase64 = submission.document
        ? encodeBase64(new Uint8Array(await submission.document.arrayBuffer()))
        : null;
      if (definition.nextAction?.kind === "import_application" && definition.oauthProfileDigest && documentBase64) {
        await importApplication({ variables: { input: {
          profileDigest: definition.oauthProfileDigest,
          projectLabel: null,
          clientDocumentBase64: documentBase64
        } } });
      } else {
        await setupConnection({ variables: { input: {
          semanticDigest: definition.semanticDigest,
          replacementConnectionId: definition.nextAction?.connectionId,
          fieldValues: submission.fieldValues,
          documentBase64
        } } });
      }
      setCredentialSetupOpen(false);
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The reviewed credentials could not be added.");
      throw caught;
    }
  };
  const authorize = async () => {
    const action = definition.nextAction;
    if (!action?.applicationId || action.expectedApplicationRevision === null) return;
    const navigation = reserveExternalAuthNavigation();
    finishingOauthAttempt.current = false;
    setError(null);
    try {
      const response = await startOauth({
        variables: {
          input: {
            applicationId: action.applicationId,
            expectedApplicationRevision: action.expectedApplicationRevision,
            grantId: action.grantId,
            expectedGrantRevision: action.expectedGrantRevision,
            semanticDigest: action.semanticDigest,
            operationIds: action.operationIds
          }
        }
      });
      const attempt = response.data?.startAdapterOauthSetup;
      if (!attempt) throw new Error("Noema did not return an OAuth attempt.");
      setOauthAttemptId(attempt.attemptId);
      setOauthAttemptNeedsAttach(action.connectionId === null || action.kind === "reconnect_account");
      setAuthorizationExpiry(attempt.expiresAtEpochSeconds);
      setAuthorizing(true);
      await navigation.open(attempt.authorizationUrl);
    } catch (caught: unknown) {
      navigation.cancel();
      setAuthorizing(false);
      setOauthAttemptId(null);
      setOauthAttemptNeedsAttach(false);
      finishingOauthAttempt.current = false;
      setAuthorizationExpiry(null);
      setError(caught instanceof Error ? caught.message : "Authorization could not be started.");
    }
  };
  const finishOauthAttempt = React.useCallback(async (
    attempt: { status: string; grantId?: string | null; grantRevision?: number | null }
  ) => {
    if (!["completed", "failed", "expired", "superseded"].includes(attempt.status)) return;
    if (finishingOauthAttempt.current) return;
    finishingOauthAttempt.current = true;
    setOauthAttemptId(null);
    setAuthorizing(false);
    setAuthorizationExpiry(null);
    if (attempt.status !== "completed") {
      setOauthAttemptNeedsAttach(false);
      setError(attempt.status === "expired"
        ? "Authorization expired. You can try again."
        : "Authorization did not complete. You can try again.");
      return;
    }
    try {
      if (oauthAttemptNeedsAttach) {
        if (!attempt.grantId || attempt.grantRevision == null) {
          throw new Error("Noema did not return the authorized account revision.");
        }
        await attachGrant({ variables: { input: {
          semanticDigest: definition.semanticDigest,
          grantId: attempt.grantId,
          expectedGrantRevision: attempt.grantRevision,
          replacementConnectionId: definition.nextAction?.connectionId
        } } });
      }
      setOauthAttemptNeedsAttach(false);
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The authorized account could not be attached.");
      onResolved?.();
    }
  }, [attachGrant, definition.nextAction, definition.semanticDigest, oauthAttemptNeedsAttach, onResolved]);
  useSubscription(AdapterOauthAttemptEventsDocument, {
    variables: { attemptId: oauthAttemptId ?? "" },
    skip: oauthAttemptId === null,
    onData: ({ data }) => {
      const attempt = data.data?.adapterOauthAttemptEvents;
      if (attempt) void finishOauthAttempt(attempt);
    }
  });
  React.useEffect(() => {
    if (!oauthAttemptId) return;
    const recoverAttempt = () => {
      if (document.visibilityState !== "visible") return;
      void loadOauthAttempt({ variables: { attemptId: oauthAttemptId } })
        .then((result) => {
          if (result.data?.adapterOauthAttempt) {
            return finishOauthAttempt(result.data.adapterOauthAttempt);
          }
          return undefined;
        })
        .catch(() => undefined);
    };
    window.addEventListener("focus", recoverAttempt);
    document.addEventListener("visibilitychange", recoverAttempt);
    return () => {
      window.removeEventListener("focus", recoverAttempt);
      document.removeEventListener("visibilitychange", recoverAttempt);
    };
  }, [finishOauthAttempt, loadOauthAttempt, oauthAttemptId]);
  const attach = async () => {
    const action = definition.nextAction;
    if (!action?.grantId || action.expectedGrantRevision === null) return;
    setError(null);
    try {
      await attachGrant({ variables: { input: {
        semanticDigest: action.semanticDigest,
        grantId: action.grantId,
        expectedGrantRevision: action.expectedGrantRevision
      } } });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The account could not be attached.");
    }
  };
  const submitPolicy = async () => {
    if (!policyConnection) return;
    setError(null);
    try {
      await savePolicy({ variables: { input: {
        kind: "API",
        connectionId: policyConnection.connectionId,
        expectedConnectionRevision: String(policyConnection.connectionRevision),
        expectedPolicyRevision: policyConnection.policyRevision,
        dataSharingPolicy: sharing,
        unsafeActionPolicy: unsafeActions
      } } });
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The tool policy could not be saved.");
    }
  };
  React.useEffect(() => {
    if (!authorizing || authorizationExpiry === null || policyConnection) return;
    const timeout = window.setTimeout(() => {
      setAuthorizing(false);
      setOauthAttemptId(null);
      setOauthAttemptNeedsAttach(false);
      finishingOauthAttempt.current = false;
      setAuthorizationExpiry(null);
      setError("Authorization expired. You can try again.");
    }, Math.max(0, authorizationExpiry * 1000 - Date.now()));
    return () => window.clearTimeout(timeout);
  }, [authorizationExpiry, authorizing, policyConnection]);
  const operationCount = definition.operations.length;
  const visibleOperations = [...definition.operations]
    .sort((left, right) => operationRiskRank(left) - operationRiskRank(right))
    .slice(0, 5);
  const remainingOperationCount = operationCount - visibleOperations.length;
  const sourceIsHttps = definition.sourceReference.startsWith("https://");
  const setup = definition.credentialSetup;
  const setupUrl = setup?.setupUrl;
  const nextKind = definition.nextAction?.kind;
  const oauthSetupUnavailable = definition.reviewed && nextKind === "import_application" && !setup;
  const title = policyConnection
    ? `Enable ${definition.displayName}`
    : connection
    ? `Connect ${definition.displayName}`
    : definition.reviewed
    ? `Add credentials for ${definition.displayName}`
    : `Review ${definition.displayName}`;
  const context = policyConnection
    ? policyStep === "sharing"
      ? "Your account is connected. Choose when Noema may share relevant conversation details."
      : "Choose who may approve calls that can change, delete, or send information."
    : oauthSetupUnavailable
    ? "This connection's reviewed OAuth callback modes do not match this Noema app. Ask Noema to propose a compatible definition."
    : connection
    ? "Noema has the OAuth client details. Continue in your browser to grant the reviewed access."
    : setup
    ? `Create a ${setup.credentialType} using the reviewed provider instructions, then add it here.`
    : "This definition does not require credentials.";
  return (
    <InterventionCardShell
      dismissLabel="Hide OAuth setup from chat"
      onDismiss={policyConnection ? undefined : onDismiss}
      copy={
        <VStack gap={3} className={stylex.props(styles.copy).className}>
          {definition.reviewed ? (
            <HStack as="div" wrap="wrap" align="center" gap={2} className={stylex.props(styles.eyebrow).className}>
              <span>{policyConnection ? `Tool permissions · ${policyStep === "sharing" ? "1" : "2"} of 2` : connection ? "Authorization" : "OAuth setup"}</span>
              <span {...stylex.props(styles.accessBadge)}>{readOnlyLabel(definition.operations)}</span>
            </HStack>
          ) : null}
          <VStack gap={1}>
            <HStack as="div" wrap="wrap" align="center" gap={2}>
              <strong {...stylex.props(styles.summary, styles.adapterSummary)}>{title}</strong>
              {!definition.reviewed ? <span {...stylex.props(styles.accessBadge)}>{readOnlyLabel(definition.operations)}</span> : null}
            </HStack>
            {definition.reviewed ? <span {...stylex.props(styles.context)}>{context}</span> : null}
          </VStack>
          {!definition.reviewed && visibleOperations.length ? (
            <VStack gap={1}>
              <strong {...stylex.props(styles.detailHeading)}>What Noema can do</strong>
              <VStack as="ul" gap={1} className={stylex.props(styles.operationList).className}>
                {visibleOperations.map((operation) => (
                  <HStack as="li" key={operation.operationId} gap={2} align="start" justify="between">
                    <span {...stylex.props(styles.operationName)}>{humanizeOperationId(operation.operationId)}</span>
                    <span {...stylex.props(styles.operationRisk)}>{operationRiskLabel(operation)}</span>
                  </HStack>
                ))}
              </VStack>
              {remainingOperationCount > 0 ? (
                <span {...stylex.props(styles.moreOperations)}>
                  +{countLabel(remainingOperationCount, "more action")} in technical details
                </span>
              ) : null}
            </VStack>
          ) : null}
          {policyConnection ? (
            <CapabilityPolicyChoices
              serviceName={definition.displayName}
              step={policyStep}
              dataSharingPolicy={sharing}
              unsafeActionPolicy={unsafeActions}
              onChange={(policy) => {
                setSharing(policy.dataSharingPolicy);
                setUnsafeActions(policy.unsafeActionPolicy);
              }}
            />
          ) : null}
          {!definition.reviewed ? (
            <span {...stylex.props(styles.setupNote)}>
              This approves the setup only. You’ll connect your account next.
            </span>
          ) : null}
          {definition.reviewed && !connection && setup?.redirectUri ? (
            <VStack gap={1}>
              <span {...stylex.props(styles.context)}>
                <b>Authorized redirect URI</b><br />
                Copy this exact value into the provider's OAuth client form.
              </span>
              <code {...stylex.props(styles.redirectUriValue)}>{setup.redirectUri}</code>
            </VStack>
          ) : null}
          <Dialog
            isOpen={technicalDetailsOpen}
            onOpenChange={setTechnicalDetailsOpen}
            purpose="info"
            width={680}
            maxHeight="min(760px, calc(100dvh - var(--spacing-8)))"
            aria-label={`Technical details for ${definition.displayName}`}
          >
            <Layout
              height="auto"
              header={<DialogHeader title="Technical details" subtitle={definition.displayName} onOpenChange={setTechnicalDetailsOpen} />}
              content={
                <LayoutContent>
                  <AdapterDefinitionReviewDetails definition={definition} />
                </LayoutContent>
              }
            />
          </Dialog>
          <AdapterCredentialSetupDialog
            title={nextKind === "import_application" ? "Import OAuth client" : undefined}
            serviceName={definition.displayName}
            setup={setup}
            scopes={definition.scopes}
            open={credentialSetupOpen}
            submitting={credentialSetup.loading || applicationImport.loading}
            error={error}
            onOpenChange={(open) => {
              if (!credentialSetup.loading && !applicationImport.loading) setCredentialSetupOpen(open);
            }}
            intro={nextKind === "import_application"
              ? "Import this provider client document once. You can reuse it for more accounts and compatible APIs."
              : undefined}
            submitLabel={nextKind === "import_application" ? "Import OAuth client" : "Add connection"}
            onSubmit={importCredentials}
          />
          {error ? <span role="alert" {...stylex.props(styles.error)}>{error}</span> : null}
        </VStack>
      }
      actions={
        <HStack gap={1} justify="end" className={stylex.props(styles.actions).className}>
          <IconButton
            type="button"
            size="sm"
            variant="ghost"
            label={`Technical details for ${definition.displayName}`}
            tooltip="Technical details"
            icon={<Code2 aria-hidden="true" size={15} />}
            onClick={() => setTechnicalDetailsOpen(true)}
          />
          {definition.reviewed && !policyConnection && !oauthSetupUnavailable && setupUrl ? (
            <Button
              size="sm"
              variant="ghost"
              label="Developer Tools"
              isDisabled={approval.loading || credentialSetup.loading || oauthStart.loading || authorizing}
              onClick={() => void openUrl(setupUrl)}
            />
          ) : definition.reviewed && sourceIsHttps ? (
            <Button
              size="sm"
              variant="ghost"
              label={definition.reviewed ? "Open official source" : "Review source"}
              isDisabled={approval.loading}
              onClick={() => void openUrl(definition.sourceReference)}
            />
          ) : null}
          {policyConnection ? (
            policyStep === "sharing" ? (
              <Button
                size="sm"
                variant="primary"
                label="Continue"
                onClick={() => setPolicyStep("unsafe_actions")}
              />
            ) : (
              <>
                <Button
                  size="sm"
                  variant="ghost"
                  label="Back"
                  isDisabled={policySave.loading}
                  onClick={() => setPolicyStep("sharing")}
                />
                <Button
                  size="sm"
                  variant="primary"
                  label={`Enable ${definition.displayName}`}
                  isLoading={policySave.loading}
                  isDisabled={policySave.loading}
                  onClick={() => void submitPolicy()}
                />
              </>
            )
          ) : oauthSetupUnavailable ? null : definition.reviewed && nextKind === "attach_account" ? (
            <Button
              size="sm"
              variant="primary"
              label={`Connect ${definition.displayName}`}
              isLoading={grantAttach.loading}
              isDisabled={grantAttach.loading}
              onClick={() => void attach()}
            />
          ) : definition.reviewed && ["add_account", "add_access", "reconnect_account"].includes(nextKind ?? "") ? (
            <Button
              size="sm"
              variant="primary"
              label={nextKind === "add_access" ? "Add access" : nextKind === "reconnect_account" ? "Reconnect account" : "Add account"}
              isLoading={oauthStart.loading || authorizing}
              isDisabled={oauthStart.loading || authorizing}
              onClick={() => void authorize()}
            />
          ) : definition.reviewed && ["import_application", "set_up_credential"].includes(nextKind ?? "") ? (
            <Button
              size="sm"
              variant="primary"
              label={nextKind === "import_application" ? `Set up ${definition.displayName}` : "Add credentials"}
              isLoading={credentialSetup.loading || applicationImport.loading}
              isDisabled={credentialSetup.loading || applicationImport.loading || oauthStart.loading || !setup}
              onClick={() => setCredentialSetupOpen(true)}
            />
          ) : definition.reviewed ? null : (
            <>
              <Button
                size="sm"
                variant="ghost"
                label="Cancel"
                isLoading={cancellation.loading}
                isDisabled={approval.loading || cancellation.loading}
                onClick={() => void cancel()}
              />
              <Button
                size="sm"
                variant="primary"
                label="Approve"
                isLoading={approval.loading}
                isDisabled={approval.loading || cancellation.loading}
                onClick={() => void approve()}
              />
            </>
          )}
        </HStack>
      }
    />
  );
}

function InterventionCardShell({
  copy,
  actions,
  dismissLabel,
  onDismiss
}: {
  copy: React.ReactNode;
  actions: React.ReactNode;
  dismissLabel?: string;
  onDismiss?: () => void;
}) {
  return (
    <HumanInterventionCard
      content={copy}
      actions={actions}
      dismissLabel={dismissLabel}
      onDismiss={onDismiss}
    />
  );
}

function readDismissedAdapterSetups() {
  if (typeof window === "undefined") return new Set<string>();
  try {
    const stored = JSON.parse(window.localStorage.getItem(dismissedAdapterSetupsKey) ?? "[]");
    return new Set<string>(Array.isArray(stored) ? stored.filter((value): value is string => typeof value === "string") : []);
  } catch {
    return new Set<string>();
  }
}

function writeDismissedAdapterSetups(digests: Set<string>) {
  try {
    window.localStorage.setItem(dismissedAdapterSetupsKey, JSON.stringify([...digests]));
  } catch {
    // Storage can be unavailable in privacy-restricted browser contexts; local state still dismisses the card.
  }
}

function readOnlyLabel(operations: PendingAdapterDefinition["operations"]) {
  if (!operations.length) return "No actions";
  return operations.every((operation) => operation.readOnly === true) ? "Read only" : "Can make changes";
}

type AdapterOperation = PendingAdapterDefinition["operations"][number];

function operationRiskRank(operation: AdapterOperation) {
  if (operation.destructive === true) return 0;
  if (operation.readOnly !== true) return 1;
  return 2;
}

function operationRiskLabel(operation: AdapterOperation) {
  if (operation.destructive === true) return "Can delete";
  if (operation.readOnly === true) return "View only";
  return "Can make changes";
}

function humanizeOperationId(operationId: string) {
  const value = operationId.replace(/[_\-.:\s]+/g, " ").trim().toLowerCase();
  return value.replace(/^./, (character) => character.toUpperCase());
}

function countLabel(count: number, singular: string) {
  return `${count} ${singular}${count === 1 ? "" : "s"}`;
}

function encodeBase64(bytes: Uint8Array) {
  let value = "";
  for (let index = 0; index < bytes.length; index += 1) {
    value += String.fromCharCode(bytes[index] ?? 0);
  }
  return btoa(value);
}

const styles = stylex.create({
  staleInterventions: {
    minWidth: 0,
    margin: "var(--spacing-0)",
    padding: "var(--spacing-0)",
    borderWidth: 0,
    opacity: 0.72
  },
  queryError: {
    marginBlockStart: "var(--spacing-2)",
    marginInline: "var(--spacing-2)"
  },
  motionItem: {
    minWidth: 0,
    overflow: "clip",
    overflowClipMargin: "var(--human-intervention-card-overlap, var(--spacing-3))"
  },
  list: {
    width: "100%",
    padding: "var(--spacing-2)",
    "--human-intervention-card-radius": "calc(var(--radius) * 1.8)",
    "--human-intervention-motion-gap": "var(--spacing-2)"
  },
  nonChatList: {
    "--human-intervention-motion-gap": "var(--spacing-3)"
  },
  dockList: {
    paddingBlockStart: "var(--spacing-2)",
    paddingInline: "var(--spacing-0)",
    paddingBlockEnd: "var(--spacing-0)",
    marginBlockEnd: "calc(-1 * var(--human-intervention-card-overlap, var(--spacing-6)))"
  },
  dockStatus: {
    marginBlockEnd: "calc(-1 * var(--human-intervention-card-overlap, var(--spacing-6)))"
  },
  taskList: {
    padding: "var(--spacing-0)",
    marginBlockEnd: "calc(-1 * var(--human-intervention-card-overlap, var(--spacing-6)))",
    "--human-intervention-card-shadow": "var(--task-card-shadow, var(--shadow-low))"
  },
  queueNavigation: {
    paddingInline: "var(--spacing-3)",
    paddingBlockStart: "var(--spacing-1)",
    marginBlockEnd: "calc(-1 * var(--spacing-1))"
  },
  queuePosition: {
    color: "var(--color-on-accent)",
    fontSize: 12,
    lineHeight: 1.35
  },
  queueAction: {
    color: {
      default: "var(--color-on-accent)",
      ":disabled": "var(--color-on-accent)"
    },
    opacity: {
      default: 1,
      ":disabled": 0.45
    }
  },
  copy: {
    minWidth: 0
  },
  eyebrow: {
    color: "var(--noema-text-muted)",
    fontSize: 12,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: "0.045em",
    whiteSpace: "nowrap"
  },
  accessBadge: {
    borderRadius: "var(--radius-full)",
    backgroundColor: "var(--noema-surface-sunken)",
    paddingBlock: "var(--spacing-0-5)",
    paddingInline: "var(--spacing-1)",
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    letterSpacing: 0,
    textTransform: "none"
  },
  summary: {
    color: "var(--noema-text-primary)",
    fontSize: 12,
    lineHeight: 1.35
  },
  adapterSummary: {
    fontSize: 14
  },
  context: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    lineHeight: 1.4
  },
  setupNote: {
    color: "var(--noema-text-muted)",
    fontSize: 12,
    lineHeight: 1.4
  },
  operationList: {
    width: "100%",
    margin: 0,
    padding: 0,
    listStyle: "none",
    color: "var(--noema-text-primary)",
    fontSize: 12,
    lineHeight: 1.35,
    overflowWrap: "anywhere"
  },
  operationRisk: {
    flexShrink: 0,
    color: "var(--noema-text-muted)",
    fontSize: 12,
    lineHeight: 1.35
  },
  operationName: {
    minWidth: 0,
    overflowWrap: "anywhere"
  },
  moreOperations: {
    color: "var(--noema-text-muted)",
    fontSize: 12,
    lineHeight: 1.35
  },
  redirectUriValue: {
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--noema-text-primary)",
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: 12,
    lineHeight: 1.4,
    overflowWrap: "anywhere",
    userSelect: "all",
    cursor: "text"
  },
  detailHeading: {
    color: "var(--noema-text-primary)",
    fontSize: 12
  },
  actions: {
    flexShrink: 0,
  },
  error: {
    color: "var(--noema-text-danger)",
    fontSize: 12
  }
});
